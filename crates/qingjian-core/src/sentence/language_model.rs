/// 整句转换的打分来源：词级语言模型。实现放兄弟 crate（`qingjian-lm`），Core 只认这个 trait。
pub trait LanguageModel: Send {
    /// `log P(word | previous)`；`previous` 为 `None` 表示句首。模型不认识 `word` 时返回 `None`，
    /// 由 Core 用词库词频兜底。
    fn log_prob(&self, previous: Option<&str>, word: &str) -> Option<f64>;

    /// `log P(word)` 的纯一元先验，不看上下文。声母缩写（`sj`、`xx`）排序用它：
    /// 缩写是高歧义通道，后验主要由一元先验决定；静态语料的句首二元语域偏书面，不适合给缩写排序。
    /// 默认返回 `None`，由调用方退回词库词频兜底。
    fn unigram_log_prob(&self, word: &str) -> Option<f64> {
        let _ = word;
        None
    }
}

/// 没接语言模型：一律兜底，整句转换退化为一元词频。
#[derive(Debug, Default, Clone, Copy)]
pub struct NoLanguageModel;

impl LanguageModel for NoLanguageModel {
    fn log_prob(&self, _previous: Option<&str>, _word: &str) -> Option<f64> {
        None
    }
}
