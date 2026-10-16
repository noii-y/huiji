//! 后台线程：收请求、防抖、查缓存、发网络请求、回结果。

use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::time::{Duration, Instant};

use qingjian_core::{Prediction, PredictionKind, PredictionRequest};

use crate::prompt::Reply;

use crate::cache::PredictionCache;
use crate::chat_client::ChatClient;
use crate::disk_cache::DiskCache;
use crate::error::PredictError;
use crate::suspension::Suspension;

/// 内存缓存条数：只管当前会话里退格重打这类即时反复。
const CACHE_CAPACITY: usize = 64;

pub struct Worker {
    /// 请求入口。主线程 drop 掉发送端后线程自然退出。
    requests: Receiver<PredictionRequest>,

    /// 结果出口。
    responses: Sender<Prediction>,

    /// 网络客户端。
    client: ChatClient,

    /// 防抖窗口。
    debounce: Duration,

    /// 内存结果缓存（键含完整上下文，精确匹配当前会话）。
    cache: PredictionCache,

    /// 磁盘结果缓存（键只看拼音，跨重启、断网也能命中）。云没启用时为 `None`。
    disk: Option<DiskCache>,

    /// 账户失败（密钥无效 / 欠费 / 被拒）后的暂停：暂停期间不发新的网络请求，本地候选照常。
    suspension: Suspension,
}

/// 缓存里存的是解析后的回复。
type Cached = Reply;

impl Worker {
    pub fn new(
        requests: Receiver<PredictionRequest>,
        responses: Sender<Prediction>,
        client: ChatClient,
        debounce: Duration,
        disk: Option<DiskCache>,
    ) -> Self {
        Self {
            requests,
            responses,
            client,
            debounce,
            cache: PredictionCache::with_capacity(CACHE_CAPACITY),
            disk,
            suspension: Suspension::default(),
        }
    }

    /// 阻塞运行直到发送端全部关闭。
    pub fn run(mut self) -> Result<(), PredictError> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        while let Ok(first) = self.requests.recv() {
            let Some(request) = self.debounce(first) else {
                return Ok(());
            };
            let sequence = request.sequence;
            // 拿不到任何回复（网络失败）时不回，端侧译文与本地候选照常显示。
            if let Some(reply) = self.handle(&runtime, request) {
                self.reply(sequence, reply);
            }
        }
        Ok(())
    }

    /// 处理一次请求：内存缓存 → 磁盘缓存 → 发网络。组句请求没拿到云整句时，
    /// 若本地已组出完整中文长句，再补一次云端中译英（复用同一套缓存），让联网时
    /// 顶部译文统一用云；补不到（断网、超时）则留给端侧 opus-mt 兜底。
    fn handle(
        &mut self,
        runtime: &tokio::runtime::Runtime,
        request: PredictionRequest,
    ) -> Option<Reply> {
        let key = PredictionCache::key(&request);
        if let Some(reply) = self.cache.get(&key) {
            tracing::debug!(sequence = request.sequence, kind = ?request.kind, "联想命中内存缓存");
            return Some(reply.clone());
        }
        let disk_key = DiskCache::key(&request);
        if let Some(reply) = self.disk.as_mut().and_then(|disk| disk.get(&disk_key)) {
            tracing::info!(sequence = request.sequence, kind = ?request.kind, "联想命中磁盘缓存");
            let reply = reply.clone();
            // 顺手回填内存，同一次输入里继续退格重打就走内存、不再碰磁盘。
            self.cache.insert(key, reply.clone());
            return Some(reply);
        }
        // 账户失败暂停期：不发新网络请求。内存 / 磁盘缓存上面已经查过，没命中就直接
        // 交回本地候选与端侧译文（云只是增强，停了不影响打字）。
        if self.suspension.active(Instant::now()) {
            return None;
        }
        let start = Instant::now();
        let mut reply = match runtime.block_on(self.client.complete(&request)) {
            Ok(reply) => reply,
            Err(error) => {
                // 密钥无效 / 欠费 / 被拒：暂停一段时间，期间不再发，避免白烧请求、刷日志；
                // record 内部已记一条带原因的日志，普通失败才在这里补一条。
                if !self.suspension.record(&error, Instant::now(), "联想") {
                    tracing::warn!(sequence = request.sequence, %error, "联想失败");
                }
                return None;
            }
        };
        tracing::info!(
            sequence = request.sequence,
            elapsed_ms = start.elapsed().as_millis(),
            words = reply.words.len(),
            sentence = reply.sentence.is_some(),
            "联想完成"
        );
        // 云这次没给整句（缩写、没把握），但本地已组出完整中文长句：补一次云端中译英。
        // 译文跟随本地首位中文；补不到（断网、超时）保持 None，顶部由端侧 opus-mt 兜底。
        if reply.sentence.is_none() && reply.translation.is_none() {
            if let Some(fill) = translation_fill_request(&request) {
                tracing::info!(sequence = request.sequence, "云未给整句，补一次云端中译英");
                if let Some(fill_reply) = self.handle(runtime, fill) {
                    reply.translation = fill_reply.sentence;
                }
            }
        }
        // 空回复不进缓存：模型偶尔什么都不给（问字尤其），缓存住就等于这个问题以后永远没答案，
        // 用户重打一遍也只会命中缓存（2026-09-07 `?mumumu` 就是这么卡住的）。
        // 补的中译英随翻译请求单独缓存，这里只管主请求。
        if !reply.is_empty() {
            self.cache.insert(key, reply.clone());
            if let Some(disk) = self.disk.as_mut() {
                disk.insert(disk_key, reply.clone());
                disk.flush();
            }
        }
        Some(reply)
    }

    /// 防抖：在窗口内持续收到新请求就一直等，只保留最后一个。发送端关闭返回 `None`。
    fn debounce(&self, mut latest: PredictionRequest) -> Option<PredictionRequest> {
        loop {
            match self.requests.recv_timeout(self.debounce) {
                Ok(newer) => latest = newer,
                Err(RecvTimeoutError::Timeout) => return Some(latest),
                Err(RecvTimeoutError::Disconnected) => return None,
            }
        }
    }

    fn reply(&self, sequence: u64, reply: Cached) {
        // 接收端没了说明 Predictor 已经被 drop，线程随后也会退出
        let _ = self.responses.send(Prediction {
            sequence,
            words: reply.words,
            sentence: reply.sentence,
            translation: reply.translation,
        });
    }
}

/// 组句请求没拿到云整句时，把本地组出的中文整句单独构造一个中译英请求。
/// 不是组句、没要整句（短于 4 音节，本地首选已够可靠）或本地没组出中文时返回 None。
/// 译文走的是选中文字翻译同一套提示词与缓存，同一句重复打只花一次 token。
fn translation_fill_request(request: &PredictionRequest) -> Option<PredictionRequest> {
    if request.kind != PredictionKind::Compose || !request.want_sentence {
        return None;
    }
    let source = request.guess.trim();
    if source.is_empty() {
        return None;
    }
    let mut fill = request.clone();
    fill.kind = PredictionKind::Translate;
    fill.text = source.to_owned();
    fill.guess = String::new();
    fill.want_sentence = false;
    if fill.target_language.trim().is_empty() {
        fill.target_language = "en".to_owned();
    }
    Some(fill)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn compose_request(guess: &str, want_sentence: bool) -> PredictionRequest {
        PredictionRequest {
            sequence: 1,
            kind: PredictionKind::Compose,
            before: String::new(),
            after: String::new(),
            pinyin: "wo'jin'tian'xiang'chi'p'g".into(),
            letters: "wojintianxiangchipg".into(),
            syllables: 8,
            candidates: vec!["我今天想吃苹果".into()],
            guess: guess.into(),
            max_items: 4,
            want_sentence,
            text: String::new(),
            target_language: "en".into(),
        }
    }

    #[test]
    fn fills_translation_for_long_local_sentence_without_cloud_sentence() {
        let fill = translation_fill_request(&compose_request("我今天想吃苹果", true)).unwrap();
        assert_eq!(fill.kind, PredictionKind::Translate);
        assert_eq!(fill.text, "我今天想吃苹果");
        assert_eq!(fill.target_language, "en");
        // 翻译走 text，guess 清空；它不再是组句请求
        assert!(fill.guess.is_empty());
        assert!(!fill.want_sentence);
    }

    #[test]
    fn no_fill_for_short_input_because_sentence_not_wanted() {
        // 短于 4 音节 want_sentence=false，本地首选已可靠，不补
        assert!(translation_fill_request(&compose_request("你好", false)).is_none());
    }

    #[test]
    fn no_fill_when_local_guess_is_empty() {
        assert!(translation_fill_request(&compose_request("   ", true)).is_none());
    }

    #[test]
    fn no_fill_for_non_compose_requests() {
        // 补全自身是翻译请求，不能再触发补全，否则递归不终止
        let mut translate = compose_request("我今天想吃苹果", true);
        translate.kind = PredictionKind::Translate;
        translate.text = "我今天想吃苹果".to_owned();
        assert!(translation_fill_request(&translate).is_none());

        let mut question = compose_request("问题", true);
        question.kind = PredictionKind::Question;
        assert!(translation_fill_request(&question).is_none());
    }

    #[test]
    fn fill_defaults_empty_target_language_to_english() {
        let mut req = compose_request("我今天想吃苹果", true);
        req.target_language = String::new();
        let fill = translation_fill_request(&req).unwrap();
        assert_eq!(fill.target_language, "en");
    }
}
