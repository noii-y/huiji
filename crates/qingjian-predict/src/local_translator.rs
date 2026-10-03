//! 端侧整句翻译：[`Predictor`] 的本地实现，ct2rs 跑 CTranslate2，全程离线、数据不出本机。
//!
//! 和 [`crate::CloudPredictor`] 一样用后台线程加通道：`submit` 只丢请求、`poll` 只取结果，
//! 推理再慢也不卡在输入线程上。目前模型是中译英 opus-mt；其他语种等模型就位再扩。

use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

use ct2rs::tokenizers::sentencepiece::Tokenizer as SpmTokenizer;
use ct2rs::{ComputeType, Config as Ct2Config, TranslationOptions, Translator};
use qingjian_core::{Prediction, PredictionKind, PredictionPolicy, PredictionRequest, Predictor};

/// 端侧翻译器初始化或运行出错。
#[derive(Debug, thiserror::Error)]
pub enum LocalTranslateError {
    /// 模型加载或 CT2 初始化失败。
    #[error("端侧翻译模型加载失败：{0}")]
    Model(String),

    /// 后台线程起不来。
    #[error("无法启动端侧翻译线程：{0}")]
    Thread(#[from] std::io::Error),
}

/// 发给后台线程的任务：序号加待译原文。
type Job = (u64, String);

/// 端侧翻译器，实现 [`Predictor`]。
pub struct LocalTranslator {
    /// 观察窗口与开关。端侧不补云端词，槽位给 0，整句译文照出。
    policy: PredictionPolicy,

    /// 往后台线程发任务。
    jobs: Sender<Job>,

    /// 从后台线程收结果。
    results: Receiver<Prediction>,
}

impl LocalTranslator {
    /// 加载模型并起后台线程。`model_dir` 需含 `model.bin`、`source.spm`、`target.spm`。
    ///
    /// 注意：SentencePiece 在 Windows 上按系统代码页打开文件，`model_dir` 含非 ASCII 字符时
    /// 可能加载失败，安装目录应保持英文。
    pub fn new(model_dir: impl Into<PathBuf>) -> Result<Self, LocalTranslateError> {
        let model_dir = model_dir.into();
        let translator = build_translator(&model_dir)?;

        let (jobs, job_rx) = mpsc::channel::<Job>();
        let (result_tx, results) = mpsc::channel::<Prediction>();
        thread::Builder::new()
            .name("huiji-local-nmt".to_owned())
            .spawn(move || run(job_rx, result_tx, translator))?;

        Ok(Self {
            policy: PredictionPolicy {
                slots: 0,
                ..PredictionPolicy::default()
            },
            jobs,
            results,
        })
    }
}

/// 加载 SentencePiece 与 CT2 翻译器，int8 量化。
fn build_translator(model_dir: &Path) -> Result<Translator<SpmTokenizer>, LocalTranslateError> {
    let tokenizer = SpmTokenizer::new(model_dir)
        .map_err(|error| LocalTranslateError::Model(error.to_string()))?;
    let config = Ct2Config {
        compute_type: ComputeType::INT8,
        // 线程数留 0，让 CT2 按物理核自行决定
        ..Default::default()
    };
    Translator::with_tokenizer(model_dir, tokenizer, &config)
        .map_err(|error| LocalTranslateError::Model(error.to_string()))
}

/// 后台线程主循环：收一句、译一句、把结果发回去。
fn run(job_rx: Receiver<Job>, result_tx: Sender<Prediction>, translator: Translator<SpmTokenizer>) {
    // Python 探针验证过：默认生成参数会让 opus-mt 重复退化，这两个参数把循环压掉。
    let options = TranslationOptions {
        beam_size: 2,
        repetition_penalty: 1.2,
        no_repeat_ngram_size: 3,
        ..Default::default()
    };

    while let Ok((sequence, text)) = job_rx.recv() {
        let prediction = match translator.translate_batch(&[text.as_str()], &options, None) {
            Ok(batch) if !batch.is_empty() => Prediction {
                sequence,
                words: Vec::new(),
                sentence: Some(batch[0].0.clone()),
            },
            // 空结果或出错也回一条，让壳别一直等
            Ok(_) => Prediction {
                sequence,
                ..Default::default()
            },
            Err(error) => {
                tracing::error!(%error, "端侧翻译失败");
                Prediction {
                    sequence,
                    ..Default::default()
                }
            }
        };
        if result_tx.send(prediction).is_err() {
            break; // 壳已释放，收工
        }
    }
}

impl Predictor for LocalTranslator {
    fn policy(&self) -> PredictionPolicy {
        self.policy
    }

    fn submit(&mut self, request: PredictionRequest) {
        // 翻译选中文字时原文在 `text`；打整句拼音时用 Core 本地组出的整句 `guess`。
        // 问字模式不翻译。
        let source = match request.kind {
            PredictionKind::Translate => request.text,
            PredictionKind::Compose => request.guess,
            PredictionKind::Question => return,
        };
        let source = source.trim();
        if source.is_empty() {
            return;
        }
        if self
            .jobs
            .send((request.sequence, source.to_owned()))
            .is_err()
        {
            tracing::warn!("端侧翻译线程已退出，请求被丢弃");
        }
    }

    fn poll(&mut self) -> Option<Prediction> {
        self.results.try_recv().ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 组一个不加载模型的翻译器，外部留下任务接收端，便于观察 submit。
    fn harness() -> (LocalTranslator, Receiver<Job>) {
        let (jobs, job_rx) = mpsc::channel::<Job>();
        let (_result_tx, results) = mpsc::channel::<Prediction>();
        let translator = LocalTranslator {
            policy: PredictionPolicy::default(),
            jobs,
            results,
        };
        (translator, job_rx)
    }

    fn request(sequence: u64, kind: PredictionKind, text: &str, guess: &str) -> PredictionRequest {
        PredictionRequest {
            sequence,
            kind,
            before: String::new(),
            after: String::new(),
            pinyin: String::new(),
            letters: String::new(),
            syllables: 0,
            candidates: Vec::new(),
            guess: guess.to_owned(),
            max_items: 1,
            want_sentence: true,
            text: text.to_owned(),
            target_language: "en".to_owned(),
        }
    }

    #[test]
    fn 正常翻译请求进入队列() {
        let (mut translator, job_rx) = harness();
        translator.submit(request(7, PredictionKind::Translate, "你好", ""));

        let (sequence, text) = job_rx.try_recv().expect("任务应进入队列");
        assert_eq!(sequence, 7);
        assert_eq!(text, "你好");
    }

    #[test]
    fn 整句拼音用本地guess进入队列() {
        let (mut translator, job_rx) = harness();
        translator.submit(request(4, PredictionKind::Compose, "", "你今天晚上有空吗"));

        let (sequence, text) = job_rx.try_recv().expect("任务应进入队列");
        assert_eq!(sequence, 4);
        assert_eq!(text, "你今天晚上有空吗");
    }

    #[test]
    fn 整句guess为空被忽略() {
        let (mut translator, job_rx) = harness();
        translator.submit(request(1, PredictionKind::Compose, "", ""));
        assert!(job_rx.try_recv().is_err());
    }

    #[test]
    fn 问字请求被忽略() {
        let (mut translator, job_rx) = harness();
        translator.submit(request(5, PredictionKind::Question, "", ""));
        assert!(job_rx.try_recv().is_err());
    }

    #[test]
    fn 空白翻译请求被忽略() {
        let (mut translator, job_rx) = harness();
        translator.submit(request(2, PredictionKind::Translate, "   ", ""));
        assert!(job_rx.try_recv().is_err());
    }

    #[test]
    fn 原文两端空白被裁掉() {
        let (mut translator, job_rx) = harness();
        translator.submit(request(
            3,
            PredictionKind::Translate,
            "  今天天气不错  ",
            "",
        ));
        let (_, text) = job_rx.try_recv().expect("任务应进入队列");
        assert_eq!(text, "今天天气不错");
    }
}
