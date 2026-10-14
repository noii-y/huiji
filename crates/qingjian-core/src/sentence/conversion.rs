use super::SentenceWord;

/// 一次整句转换的结果。
#[derive(Debug, Clone, PartialEq)]
pub struct Conversion {
    /// 整句文本。
    pub text: String,

    /// 覆盖的全拼音节，与 `text` 逐字对应。
    pub syllables: Vec<String>,

    /// 路径上的词，按顺序。只有一个说明整段本来就是一个词，不必当整句显示。
    pub words: Vec<SentenceWord>,

    /// 路径得分（log 概率之和），越大越好。
    pub score: f64,

    /// `score` 里静态语言模型那部分（每步只问静态模型、不认识就兜底的 log 概率之和），不含个人 n-gram 插值、
    /// 用户加分与代价。神经重打分用它：神经分替换的是静态模型的判断，个人的那些原样保留。
    pub static_score: f64,

    /// 路径上模糊音 / 敲错变体的代价之和（已含在 `score` 里）：大于 0 说明这条路径不是按敲的原样读的。
    pub penalty: f64,

    /// 路径上「随机手滑」边（相邻键误碰 / 换位 / 多漏键）的代价之和；系统发音纠错（前后鼻音、平翘舌）
    /// 与用户主动开的模糊音不计入。原样词保护只在这一项大于 0 时退回——发音模糊允许和原样词竞争。
    pub strict_penalty: f64,
}

impl Conversion {
    pub fn word_count(&self) -> usize {
        self.words.len()
    }

    /// 路径里有音节不是按敲的原样读的（模糊音或敲错变体）。
    pub fn altered(&self) -> bool {
        self.penalty > 0.0
    }

    /// 路径里含随机手滑（不是系统发音 / 模糊音）：原样词保护只对这种路径生效。
    pub fn strict_altered(&self) -> bool {
        self.strict_penalty > 0.0
    }

    /// 路径里有占位音节（没转成字的拼音）。
    pub fn has_placeholder(&self) -> bool {
        self.words.iter().any(|w| w.placeholder)
    }
}
