//! 云端联想结果的磁盘持久化缓存。
//!
//! 内存缓存（[`crate::cache::PredictionCache`]）进程一退就没了，而云整句纠错里有大量句子是
//! 每天反复打的：固定的工作话术、常用问句。这些结果落到磁盘后，下次（包括重启、断网、云端
//! 超时）同样的拼音直接出结果，不再发一次请求、也不再花 token。
//!
//! 和内存缓存的区别在键：内存键把光标前后文本、本地候选都算进去，求的是「这次输入」完全一致；
//! 磁盘键只保留决定整句的稳定字段（敲的字母、切分、要不要整句、目标语言），丢掉随应用变化的
//! 上下文。这样同一句话在记事本里打过、在聊天框里再打也能命中。整句纠错主要看拼音，上下文只是
//! 提示，忽略它命中率高得多，代价是极端情况下候选略有差异——这在可接受范围。
//!
//! 只在后台 worker 线程里访问，不用加锁。写盘走「临时文件 + 重命名」，避免写一半崩掉损坏缓存。

use std::collections::{HashMap, VecDeque};
use std::fs;
use std::path::PathBuf;

use qingjian_core::{PredictionKind, PredictionRequest};
use serde::{Deserialize, Serialize};

use crate::prompt::Reply;

/// 磁盘缓存文件格式版本。解析结构变了就加一，旧文件加载时直接作废。
const FORMAT_VERSION: u32 = 1;

/// 磁盘上保留多少条。每条通常一两百字，两千条大约一兆上下，启动加载和整文件重写都不慢。
pub const DEFAULT_DISK_CAPACITY: usize = 2000;

/// 磁盘缓存条目，顺序即 LRU 顺序（最旧在前、最新在后）。
#[derive(Serialize, Deserialize)]
struct StoredCache {
    version: u32,
    model: String,
    #[serde(default)]
    entries: Vec<StoredEntry>,
}

#[derive(Serialize, Deserialize)]
struct StoredEntry {
    key: String,
    reply: Reply,
}

/// 跨会话的云端结果缓存。
pub struct DiskCache {
    /// 缓存文件完整路径。
    path: PathBuf,

    /// 产生这份缓存的模型名；模型换了（flash 换 pro）旧结果作废。
    model: String,

    /// 容量。
    capacity: usize,

    /// 键 → 结果。
    entries: HashMap<String, Reply>,

    /// LRU 顺序，队首最旧、队尾最新。
    order: VecDeque<String>,

    /// 有未写入磁盘的改动（新增 / 淘汰）。
    dirty: bool,
}

impl DiskCache {
    /// 加载已有缓存。文件不存在、解析失败、版本或模型对不上都返回一个空缓存，不影响启动。
    pub fn open(path: PathBuf, model: String, capacity: usize) -> Self {
        let mut cache = Self {
            path,
            model,
            capacity,
            entries: HashMap::new(),
            order: VecDeque::new(),
            dirty: false,
        };
        cache.load();
        cache
    }

    /// 决定磁盘缓存的稳定键：只留决定整句 / 译文的字段，丢掉光标上下文和本地候选。
    pub fn key(request: &PredictionRequest) -> String {
        match request.kind {
            // 翻译整段原文，确定性最强，键就是原文 + 目标语言。
            PredictionKind::Translate => {
                format!("T\u{1}{}\u{1}{}", request.text, request.target_language)
            }
            // 问字：只看敲的问题和要几条。
            PredictionKind::Question => {
                format!("Q\u{1}{}\u{1}{}", request.letters, request.max_items)
            }
            // 组句：字母、切分、要不要整句、条数、目标语言。
            PredictionKind::Compose => format!(
                "C\u{1}{}\u{1}{}\u{1}{}\u{1}{}\u{1}{}",
                request.letters,
                request.pinyin,
                request.want_sentence,
                request.max_items,
                request.target_language
            ),
        }
    }

    /// 命中并把该条移到最近使用；不标脏，顺序漂移等下次插入时一并落盘。
    pub fn get(&mut self, key: &str) -> Option<&Reply> {
        if self.entries.contains_key(key) {
            if let Some(pos) = self.order.iter().position(|k| k.as_str() == key) {
                let k = self.order.remove(pos)?;
                self.order.push_back(k);
            }
            self.entries.get(key)
        } else {
            None
        }
    }

    /// 插入或更新一条，超出容量淘汰最旧的，并标脏等待落盘。
    pub fn insert(&mut self, key: String, reply: Reply) {
        if self.entries.insert(key.clone(), reply).is_none() {
            self.order.push_back(key);
        } else {
            // 已存在：刷新到队尾。
            if let Some(pos) = self.order.iter().position(|k| k.as_str() == key.as_str()) {
                let k = self.order.remove(pos);
                if let Some(k) = k {
                    self.order.push_back(k);
                }
            }
        }
        while self.order.len() > self.capacity {
            if let Some(oldest) = self.order.pop_front() {
                self.entries.remove(&oldest);
            }
        }
        self.dirty = true;
    }

    /// 有改动才写盘；返回是否写了。
    pub fn flush(&mut self) -> bool {
        if !self.dirty {
            return false;
        }
        let entries = self
            .order
            .iter()
            .filter_map(|key| {
                self.entries.get(key).map(|reply| StoredEntry {
                    key: key.clone(),
                    reply: reply.clone(),
                })
            })
            .collect();
        let stored = StoredCache {
            version: FORMAT_VERSION,
            model: self.model.clone(),
            entries,
        };
        let wrote = match serde_json::to_vec(&stored) {
            Ok(bytes) => self.write_atomic(&bytes),
            Err(error) => {
                tracing::warn!(%error, "序列化云缓存失败");
                false
            }
        };
        if wrote {
            self.dirty = false;
        }
        wrote
    }

    /// 已缓存条数，供日志和测试。
    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    fn load(&mut self) {
        let bytes = match fs::read(&self.path) {
            Ok(bytes) => bytes,
            // 还没有缓存文件是常态。
            Err(_) => return,
        };
        let stored: StoredCache = match serde_json::from_slice(&bytes) {
            Ok(stored) => stored,
            Err(error) => {
                tracing::warn!(%error, "云缓存解析失败，按空缓存重建");
                return;
            }
        };
        if stored.version != FORMAT_VERSION || stored.model != self.model {
            tracing::info!(
                file_version = stored.version,
                file_model = %stored.model,
                current_model = %self.model,
                "云缓存模型或格式已变，旧缓存作废"
            );
            return;
        }
        for entry in stored.entries {
            if self
                .entries
                .insert(entry.key.clone(), entry.reply)
                .is_none()
            {
                self.order.push_back(entry.key);
            }
        }
        while self.order.len() > self.capacity {
            if let Some(oldest) = self.order.pop_front() {
                self.entries.remove(&oldest);
            }
        }
        tracing::info!(entries = self.entries.len(), "已加载云端结果磁盘缓存");
    }

    /// 写临时文件再重命名，避免写一半崩溃留下半个 JSON。
    fn write_atomic(&self, bytes: &[u8]) -> bool {
        let Some(dir) = self.path.parent() else {
            return false;
        };
        if fs::create_dir_all(dir).is_err() {
            return false;
        }
        let tmp = self.path.with_extension("cache.tmp");
        if fs::write(&tmp, bytes).is_err() {
            return false;
        }
        // std::fs::rename 在 Windows 上用 MoveFileEx(REPLACE_EXISTING)，可覆盖旧文件。
        if let Err(error) = fs::rename(&tmp, &self.path) {
            tracing::warn!(%error, "云缓存落盘失败");
            let _ = fs::remove_file(&tmp);
            false
        } else {
            true
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reply(s: &str) -> Reply {
        Reply {
            words: Vec::new(),
            sentence: Some(s.to_owned()),
            translation: None,
        }
    }

    fn compose(letters: &str, before: &str) -> PredictionRequest {
        PredictionRequest {
            sequence: 1,
            kind: PredictionKind::Compose,
            before: before.to_owned(),
            after: String::new(),
            pinyin: letters
                .chars()
                .map(|c| c.to_string())
                .collect::<Vec<_>>()
                .join("'"),
            letters: letters.to_owned(),
            syllables: letters.len(),
            candidates: vec!["本地".to_owned()],
            guess: String::new(),
            max_items: 4,
            want_sentence: true,
            text: String::new(),
            target_language: "en".to_owned(),
        }
    }

    #[test]
    fn key_ignores_context_and_candidates() {
        // 同一段拼音，光标前文本不同，磁盘键应相同，这样跨应用才命中。
        let a = compose("nihao", "在记事本");
        let b = compose("nihao", "在浏览器里");
        assert_eq!(DiskCache::key(&a), DiskCache::key(&b));
    }

    #[test]
    fn key_distinguishes_meaningful_fields() {
        let base = compose("nihao", "");
        let mut other_lang = compose("nihao", "");
        other_lang.target_language = "ja".to_owned();
        assert_ne!(DiskCache::key(&base), DiskCache::key(&other_lang));

        let mut no_sentence = compose("nihao", "");
        no_sentence.want_sentence = false;
        assert_ne!(DiskCache::key(&base), DiskCache::key(&no_sentence));

        let mut translate = compose("nihao", "");
        translate.kind = PredictionKind::Translate;
        translate.text = "你好".to_owned();
        translate.target_language = "en".to_owned();
        assert_ne!(DiskCache::key(&base), DiskCache::key(&translate));
    }

    #[test]
    fn insert_get_and_evict() {
        let mut cache = DiskCache::open(PathBuf::from("unused.json"), "m".into(), 2);
        cache.insert("a".into(), reply("一"));
        cache.insert("b".into(), reply("二"));
        // 访问 a 让它变新，淘汰时 b 最旧。
        assert!(cache.get("a").is_some());
        cache.insert("c".into(), reply("三"));
        assert_eq!(cache.get("a").unwrap().sentence.as_deref(), Some("一"));
        assert!(cache.get("b").is_none());
        assert_eq!(cache.get("c").unwrap().sentence.as_deref(), Some("三"));
    }

    #[test]
    fn survives_round_trip_and_rejects_other_model() {
        let dir = std::env::temp_dir().join(format!("hj-disk-cache-{}", std::process::id()));
        let _ = fs::create_dir_all(&dir);
        let path = dir.join("cloud-cache.json");
        let _ = fs::remove_file(&path);

        {
            let mut cache = DiskCache::open(path.clone(), "deepseek-flash".into(), 10);
            cache.insert(DiskCache::key(&compose("nihao", "")), reply("你好"));
            assert!(cache.flush());
        }
        // 同模型重新打开：命中。
        {
            let mut cache = DiskCache::open(path.clone(), "deepseek-flash".into(), 10);
            assert_eq!(cache.len(), 1);
            assert_eq!(
                cache
                    .get(&DiskCache::key(&compose("nihao", "任意上下文")))
                    .unwrap()
                    .sentence
                    .as_deref(),
                Some("你好")
            );
        }
        // 换模型：旧缓存作废。
        {
            let cache = DiskCache::open(path.clone(), "deepseek-v4-pro".into(), 10);
            assert_eq!(cache.len(), 0);
        }
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn flush_only_when_dirty() {
        let dir = std::env::temp_dir().join(format!("hj-disk-cache-flush-{}", std::process::id()));
        let _ = fs::create_dir_all(&dir);
        let path = dir.join("cloud-cache.json");
        let _ = fs::remove_file(&path);
        let mut cache = DiskCache::open(path.clone(), "m".into(), 10);
        assert!(!cache.flush()); // 空且没改动
        cache.insert("a".into(), reply("一"));
        assert!(cache.flush());
        assert!(!cache.flush()); // 已经落盘
        let _ = fs::remove_file(&path);
    }
}
