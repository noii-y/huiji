//! 中文模式下「拼音 + 句内标点 + 大写原样段」混合串的解析。
//!
//! 灰迹中文模式默认全力匹配中文。组句中打逗号、句号等标点不再把整串打成英文直输段，
//! 标点留在句子内部；连续大写字母（如 PG、ID）是原样片段，不翻译，把句子切成前后几段，
//! 各段拼音分别匹配中文后再拼到一起。分段转换的引擎方法与查询接入在本文件的后半部分。

use super::*;

/// 混合串里的一段。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::engine) enum MixPart {
    /// 一段拼音（小写字母与音节分隔符 `'`）。
    Pinyin(String),
    /// 句内标点（半角逗号、句号等），留在中文句子内部。
    Punctuation(char),
    /// 连续大写字母组成的原样片段（PG、ID），不翻译。
    Verbatim(String),
}

/// 留在中文句子内部的标点（缓冲区里是半角形式）。
fn is_sentence_punctuation(c: char) -> bool {
    // 只收无歧义的句读：';' 是辅码触发键（双拼里还是韵母键），':' 留给英文 / 算式，都不在此。
    matches!(c, ',' | '.' | '!' | '?')
}

/// 中文模式这段输入是否走混合分段：含句内标点或大写字母、且不含真正的 raw 符号
/// （`-`、数字、`@` 等仍走英文直输段）。
pub(in crate::engine) fn is_mixed_scope(text: &str) -> bool {
    if text.is_empty() {
        return false;
    }
    let mut has_boundary = false;
    for c in text.chars() {
        if c.is_ascii_lowercase() || c == '\'' {
            continue;
        }
        if c.is_ascii_uppercase() || is_sentence_punctuation(c) {
            has_boundary = true;
        } else {
            return false;
        }
    }
    has_boundary
}

/// 把一段混合输入解析成片段序列。
pub(in crate::engine) fn parse_mixed(text: &str) -> Vec<MixPart> {
    let mut parts = Vec::new();
    let mut pinyin = String::new();
    let mut verbatim = String::new();

    for c in text.chars() {
        if c.is_ascii_uppercase() {
            if !pinyin.is_empty() {
                parts.push(MixPart::Pinyin(std::mem::take(&mut pinyin)));
            }
            verbatim.push(c);
        } else if is_sentence_punctuation(c) {
            if !pinyin.is_empty() {
                parts.push(MixPart::Pinyin(std::mem::take(&mut pinyin)));
            }
            if !verbatim.is_empty() {
                parts.push(MixPart::Verbatim(std::mem::take(&mut verbatim)));
            }
            parts.push(MixPart::Punctuation(c));
        } else {
            if !verbatim.is_empty() {
                parts.push(MixPart::Verbatim(std::mem::take(&mut verbatim)));
            }
            pinyin.push(c);
        }
    }
    if !pinyin.is_empty() {
        parts.push(MixPart::Pinyin(pinyin));
    }
    if !verbatim.is_empty() {
        parts.push(MixPart::Verbatim(verbatim));
    }
    parts
}

impl Engine {
    /// 一个拼音片段的整句转换；切不出或读不通（含占位音节）时返回 `None`，调用方用原文兜底。
    fn convert_pinyin_part(&self, pinyin: &str) -> Option<Conversion> {
        let segmentation = parser::segment(pinyin).ok()?.into_iter().next()?;
        let conversion = self.convert_sentence(&segmentation.patterns(), true)?;
        (!conversion.has_placeholder()).then_some(conversion)
    }

    /// 中文模式混合串的首选整句：拼音段转中文，标点转全角留在句内，大写片段原样保留。
    pub(in crate::engine) fn mixed_sentence_candidate(&self, scope: &str) -> Candidate {
        let mut text = String::new();
        let mut syllables: Vec<String> = Vec::new();
        for part in parse_mixed(scope) {
            match part {
                MixPart::Pinyin(pinyin) => match self.convert_pinyin_part(&pinyin) {
                    Some(conversion) => {
                        text.push_str(&conversion.text);
                        syllables.extend(conversion.syllables);
                    }
                    // 读不通的拼音段用原文兜底，保证敲的内容一个都不丢
                    None => {
                        text.push_str(&pinyin);
                        syllables.push(pinyin);
                    }
                },
                MixPart::Punctuation(c) => {
                    text.push(full_width_punctuation(c));
                    // 音节对齐用半角原字符（缓冲区里是半角）
                    syllables.push(c.to_string());
                }
                MixPart::Verbatim(verbatim) => {
                    text.push_str(&verbatim);
                    syllables.push(verbatim);
                }
            }
        }
        Candidate {
            text,
            kind: CandidateKind::Sentence,
            syllables,
            reading: None,
            translation: None,
            aux_code: None,
        }
    }

    /// 中文模式混合串（含句内标点 / 大写原样段）的查询：首选是拼好的整句。
    pub(in crate::engine) fn query_mixed_segments(
        &self,
        keys: &str,
        rest: String,
        start: Instant,
    ) -> Query {
        let candidate = self.mixed_sentence_candidate(keys);
        Query {
            segmentations: Vec::new(),
            candidates: CandidateList {
                items: vec![candidate],
            },
            tail: String::new(),
            text: self.composition.text().to_owned(),
            cursor: self.composition.cursor(),
            rest,
            decoded_keys: false,
            shuangpin_raw_preedit: false,
            // 拼音行显示敲的原始串（含标点、大写），混合查询没有拼音切分
            typed_display: Some(keys.to_owned()),
            correction: None,
            aux: None,
            timings: Timings {
                parse: Duration::ZERO,
                lookup: Duration::ZERO,
                rank: start.elapsed(),
            },
        }
    }
}

/// 句内半角标点转全角：中文句子用全角标点。
fn full_width_punctuation(c: char) -> char {
    match c {
        ',' => '，',
        '.' => '。',
        '!' => '！',
        '?' => '？',
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_mixed_scope() {
        assert!(is_mixed_scope("woaiPGdedongxi"));
        assert!(is_mixed_scope("woaichipingguo,wohaie"));
        assert!(!is_mixed_scope("nihao"));
        assert!(!is_mixed_scope(""));
        // 真正的 raw 符号不走混合，仍交给英文直输段
        assert!(!is_mixed_scope("no-way"));
        assert!(!is_mixed_scope("a1b"));
    }

    #[test]
    fn splits_on_verbatim_capitals() {
        let parts = parse_mixed("woaiPGdedongxi");
        assert_eq!(
            parts,
            vec![
                MixPart::Pinyin("woai".to_owned()),
                MixPart::Verbatim("PG".to_owned()),
                MixPart::Pinyin("dedongxi".to_owned()),
            ]
        );
    }

    #[test]
    fn splits_on_punctuation() {
        let parts = parse_mixed("woaichipingguo,wohaie");
        assert_eq!(
            parts,
            vec![
                MixPart::Pinyin("woaichipingguo".to_owned()),
                MixPart::Punctuation(','),
                MixPart::Pinyin("wohaie".to_owned()),
            ]
        );
    }

    #[test]
    fn handles_several_boundaries() {
        // wo , ai PG hao !
        let parts = parse_mixed("wo,aiPGhao!");
        assert_eq!(
            parts,
            vec![
                MixPart::Pinyin("wo".to_owned()),
                MixPart::Punctuation(','),
                MixPart::Pinyin("ai".to_owned()),
                MixPart::Verbatim("PG".to_owned()),
                MixPart::Pinyin("hao".to_owned()),
                MixPart::Punctuation('!'),
            ]
        );
    }

    #[test]
    fn plain_pinyin_is_one_part() {
        assert_eq!(
            parse_mixed("nihao"),
            vec![MixPart::Pinyin("nihao".to_owned())]
        );
    }

    fn mixed_engine() -> Engine {
        let dict = Dictionary::parse(
            "我\two\t9000\n爱\tai\t9000\n吃\tchi\t8000\n苹果\tping guo\t9000\n的\tde\t9000\n东西\tdong xi\t8000\n还\thai\t6000\n饿\te\t7000\n",
        )
        .unwrap();
        Engine::new(dict)
    }

    #[test]
    fn joins_verbatim_capitals_into_the_sentence() {
        let engine = mixed_engine();
        let candidate = engine.mixed_sentence_candidate("woaiPGdedongxi");
        assert_eq!(candidate.text, "我爱PG的东西");
        assert_eq!(candidate.kind, CandidateKind::Sentence);
        // 音节对齐要覆盖整段输入，上屏时才不会在 PG 处留下残字符
        assert_eq!(
            engine
                .align("woaiPGdedongxi", &candidate.syllables)
                .consumed,
            "woaiPGdedongxi".len()
        );
    }

    #[test]
    fn keeps_punctuation_inside_the_sentence() {
        let engine = mixed_engine();
        let candidate = engine.mixed_sentence_candidate("woaichipingguo,wohaie");
        assert_eq!(candidate.text, "我爱吃苹果，我还饿");
        assert_eq!(
            engine
                .align("woaichipingguo,wohaie", &candidate.syllables)
                .consumed,
            "woaichipingguo,wohaie".len()
        );
    }
}
