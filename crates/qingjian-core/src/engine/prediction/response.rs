use super::cloud_word::CloudWord;

/// 联想结果。只是候选之外的补充展示，**不重排本地候选**：云端词补进第一页末尾几格。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Prediction {
    /// 对应的请求序号。
    pub sequence: u64,

    /// 这段拼音可能对应的词（已按拼音校验）。
    pub words: Vec<CloudWord>,

    /// 中文整句纠错或补全（云端）：替换整段拼音，按 Tab 上屏。
    pub sentence: Option<String>,

    /// 组句中文的外文译文（本地翻译模型）：只展示、不上屏，Tab 不接受。
    pub translation: Option<String>,
}

impl Prediction {
    pub fn is_empty(&self) -> bool {
        self.words.is_empty() && self.sentence.is_none() && self.translation.is_none()
    }
}
