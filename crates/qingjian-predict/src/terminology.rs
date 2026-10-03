//! 译文后处理：把端侧模型对网络流行语的「稳定字面硬译」改回地道说法。
//!
//! 端侧 Marian 只有约 81MB，对「摸鱼、内卷」这类词会逐字硬译成正常英语里
//! 几乎不出现的短语（feel/touch the fish、inner volume）。源端替换和占位符
//! 都被探针（tools/term_probe.py）证伪：模型会把注入的英文拆碎变形。
//! 于是改成在译文上做保守替换，只匹配正常英语不会出现、模型又稳定输出的
//! 模式，并把时态人称一并还原，避免替换出新的语法错误。

use regex::{Captures, Regex};
use std::sync::OnceLock;

/// 摸鱼硬译里的动词形态 -> slack 的对应形态。
fn fish_form(caps: &Captures) -> String {
    let verb = caps
        .get(1)
        .map(|m| m.as_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let form = match verb.as_str() {
        "feels" | "touches" => "slacks",
        "feeling" | "touching" => "slacking",
        "felt" | "touched" => "slacked",
        _ => "slack",
    };
    format!("{form} off")
}

fn fish_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r"(?i)\b(feel|feels|feeling|felt|touch|touches|touching|touched)\s+(?:the\s+)?fish\b",
        )
        .expect("摸鱼后处理正则应合法")
    })
}

fn juan_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)\binner\s+volume\b").expect("内卷后处理正则应合法"))
}

/// 对模型译文做术语后处理；没有命中规则时原样返回。
pub fn apply(sentence: &str) -> String {
    let mut out = fish_re()
        .replace_all(sentence, |caps: &Captures| fish_form(caps))
        .into_owned();
    out = juan_re().replace_all(&out, "rat race").into_owned();
    out
}

#[cfg(test)]
mod tests {
    use super::apply;

    #[test]
    fn 摸鱼原形() {
        assert_eq!(
            apply("Feel the fish for a while, and always feel the fish."),
            "slack off for a while, and always slack off."
        );
    }

    #[test]
    fn 摸鱼第三人称() {
        assert_eq!(apply("He always touches fish."), "He always slacks off.");
    }

    #[test]
    fn 摸鱼过去式() {
        assert_eq!(apply("Touched the fish all day."), "slacked off all day.");
    }

    #[test]
    fn 摸鱼分词() {
        assert_eq!(
            apply("He is feeling the fish again."),
            "He is slacking off again."
        );
    }

    #[test]
    fn 内卷改rat_race() {
        assert_eq!(
            apply("Young people are all in the inner volume."),
            "Young people are all in the rat race."
        );
    }

    #[test]
    fn 通顺译文不动() {
        let sentence = "Are you free tonight?";
        assert_eq!(apply(sentence), sentence);
    }

    #[test]
    fn 真钓鱼不误伤() {
        // go fishing / fishing by the river 是真钓鱼，不在替换规则内。
        let sentence = "They are fishing by the river.";
        assert_eq!(apply(sentence), sentence);
    }
}
