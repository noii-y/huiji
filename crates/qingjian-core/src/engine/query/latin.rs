//! 中文模式下，整串其实是一个拉丁词（英文词 / 专有名词）时给的候选，见 [`Engine::latin_match`]。

use super::*;

/// 拉丁候选放在候选列表的哪个位置。
pub(crate) enum Latin {
    /// 整串切不成拼音：拉丁词排第一，空格直接上（`linux` → Linux）。
    First(Candidate),
    /// 整串也切得成拼音、但它是专有名词：排在首选中文之后（`beijing` → Beijing）。
    AfterFirst(Candidate),
    /// 不给拉丁候选。
    None,
}

/// 构造一个拉丁（英文）候选：上屏即取其文本、清空缓冲区，不占拼音音节。
fn english_candidate(text: &str) -> Candidate {
    Candidate {
        text: text.to_owned(),
        kind: CandidateKind::English,
        syllables: Vec::new(),
        reading: None,
        translation: None,
        aux_code: None,
    }
}

impl Engine {
    /// 中文模式下整串 `keys`（全拼、全小写）对应的拉丁候选。
    ///
    /// 判定顺序：整串在英文词表里、又切不成拼音 → 排第一（普通词和专有名词都给）；
    /// 整串在词表里、也切得成拼音，但是专有名词 → 排首选中文之后；其余情况不给。
    /// 切不成拼音、词表也没有的生僻串不在此处理——回车照旧原样上屏，避免在纯声母缩写
    /// （`zt` 等待云端、`ggll` 走形码）时凭空塞一个候选。
    pub(super) fn latin_match(&self, keys: &str) -> Latin {
        if keys.is_empty() || !keys.bytes().all(|b| b.is_ascii_lowercase()) {
            return Latin::None;
        }
        // 不含任何元音的纯辅音串是中文声母简拼（sj、zt、ggll），不是拉丁词。
        // 拉丁词几乎都带元音；大写缩写专名（IBM、CPU）在大写分支处理，走不到这里。
        if !keys
            .bytes()
            .any(|b| matches!(b, b'a' | b'e' | b'i' | b'o' | b'u'))
        {
            return Latin::None;
        }
        let fully = parser::is_fully_segmentable(keys);
        if let Some(word) = self.english.as_ref().and_then(|words| words.get(keys)) {
            let proper = word.chars().any(|c| c.is_ascii_uppercase());
            if !fully {
                return Latin::First(english_candidate(word));
            }
            if proper {
                return Latin::AfterFirst(english_candidate(word));
            }
        }
        Latin::None
    }
}
