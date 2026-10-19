use qingjian_dictionary::SyllablePattern;

/// 一个位置上某种写法的来源：决定它算「系统发音模糊」还是「随机手滑」，
/// 原样词保护（见 `engine::query::converting`）只对随机手滑那条路径退回。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EdgeKind {
    /// 用户敲的原样，代价 0。
    Original,

    /// 用户在设置里主动开启的模糊音（平翘舌 / 前后鼻音 / 边鼻音等）：本人的发音习惯，不退回。
    Fuzzy,

    /// 成系统的发音漏键：前后鼻音尾 g、平翘舌声母 h，没开模糊音时也容错，不退回。
    Phonetic,

    /// 随机手滑：相邻键误碰、换位、多键 / 少键（非系统发音）。原样能拼出词时退回，防止过度纠错。
    Physical,
}

impl EdgeKind {
    /// 随机手滑边：原样词保护只认这一类。
    pub fn is_physical(self) -> bool {
        matches!(self, EdgeKind::Physical)
    }
}

/// 一个位置的「(写法, 代价, 来源) 与是否完整音节」，写法同一位置完整性相同。
type PositionForms = (Vec<(String, f64, EdgeKind)>, bool);

/// 一串模式扩展后的结果：每个位置若干写法，第一种是用户敲的（代价 0），其余是模糊音（扣 ln 2）
/// 或敲错变体（按类别与个人敲错表定代价）。拥有字符串，借出 [`SyllablePattern`]。
#[derive(Debug, Clone, Default)]
pub struct Expanded {
    /// 每个位置的 (写法, 代价, 来源) 与「是否完整音节」（同一位置的写法完整性相同）。
    positions: Vec<PositionForms>,

    /// 有没有任何一个位置带了敲的原样以外的写法：没有就不必逐条命中算代价。
    alternatives: bool,
}

impl Expanded {
    pub(super) fn new(positions: impl IntoIterator<Item = PositionForms>) -> Self {
        let positions: Vec<_> = positions.into_iter().collect();
        let alternatives = positions.iter().any(|(forms, _)| forms.len() > 1);
        Self {
            positions,
            alternatives,
        }
    }

    /// 给 `index` 位置加一种写法；已有同样写法时只留代价低的那个。不完整的位置（简拼、前缀）不加。
    pub fn push_alternative(&mut self, index: usize, text: &str, cost: f64, kind: EdgeKind) {
        let Some((forms, complete)) = self.positions.get_mut(index) else {
            return;
        };
        if !*complete {
            return;
        }
        match forms.iter_mut().find(|(t, _, _)| t == text) {
            Some(existing) => {
                if cost < existing.1 {
                    existing.1 = cost;
                    existing.2 = kind;
                }
            }
            None => forms.push((text.to_owned(), cost, kind)),
        }
        self.alternatives = true;
    }

    /// 词库多写法查询要的形状。
    pub fn positions(&self) -> Vec<Vec<SyllablePattern<'_>>> {
        self.positions
            .iter()
            .map(|(forms, complete)| {
                forms
                    .iter()
                    .map(|(text, _, _)| SyllablePattern {
                        text,
                        complete: *complete,
                    })
                    .collect()
            })
            .collect()
    }

    /// 有没有敲的原样以外的写法。
    pub fn has_alternatives(&self) -> bool {
        self.alternatives
    }

    /// `index` 位置命中音节 `syllable` 的代价：敲的原样 0，模糊音 / 敲错变体按写法记的代价；哪种写法都对不上（不该发生）算 0。
    pub fn cost(&self, index: usize, syllable: &str) -> f64 {
        let Some((forms, complete)) = self.positions.get(index) else {
            return 0.0;
        };
        forms
            .iter()
            .find(|(text, _, _)| {
                SyllablePattern {
                    text,
                    complete: *complete,
                }
                .accepts(syllable)
            })
            .map_or(0.0, |(_, cost, _)| *cost)
    }

    /// 与 [`Self::cost`] 同一写法，但只算「随机手滑」边（[`EdgeKind::Physical`]）的代价；
    /// 原样 / 模糊音 / 系统发音纠错都算 0。原样词保护据此判断首选路径是否真的有手滑。
    pub fn strict_cost(&self, index: usize, syllable: &str) -> f64 {
        let Some((forms, complete)) = self.positions.get(index) else {
            return 0.0;
        };
        forms
            .iter()
            .find(|(text, _, _)| {
                SyllablePattern {
                    text,
                    complete: *complete,
                }
                .accepts(syllable)
            })
            .map_or(
                0.0,
                |(_, cost, kind)| {
                    if kind.is_physical() { *cost } else { 0.0 }
                },
            )
    }

    /// 一条命中的总代价：各位置代价之和。没有任何替代写法时直接 0。
    pub fn penalty<'a>(&self, syllables: impl IntoIterator<Item = &'a str>) -> f64 {
        if !self.alternatives {
            return 0.0;
        }
        syllables
            .into_iter()
            .enumerate()
            .map(|(index, syllable)| self.cost(index, syllable))
            .sum()
    }

    /// 一条命中里「随机手滑」边的代价之和；大于 0 说明这条路径含物理误触（系统发音 / 模糊音不算）。
    pub fn strict_penalty<'a>(&self, syllables: impl IntoIterator<Item = &'a str>) -> f64 {
        if !self.alternatives {
            return 0.0;
        }
        syllables
            .into_iter()
            .enumerate()
            .map(|(index, syllable)| self.strict_cost(index, syllable))
            .sum()
    }

    pub fn len(&self) -> usize {
        self.positions.len()
    }

    pub fn is_empty(&self) -> bool {
        self.positions.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn costs_follow_the_matched_form() {
        let mut expanded = Expanded::new([
            (vec![("mei".to_owned(), 0.0, EdgeKind::Original)], true),
            (
                vec![
                    ("gan".to_owned(), 0.0, EdgeKind::Original),
                    ("gang".to_owned(), 0.7, EdgeKind::Fuzzy),
                ],
                true,
            ),
            (vec![("x".to_owned(), 0.0, EdgeKind::Original)], false),
        ]);
        expanded.push_alternative(1, "guan", 4.5, EdgeKind::Physical);
        expanded.push_alternative(1, "gang", 4.5, EdgeKind::Physical); // 已有更便宜的模糊音写法，保留 0.7
        expanded.push_alternative(2, "xi", 4.0, EdgeKind::Physical); // 不完整的位置不加
        assert!(expanded.has_alternatives());
        assert_eq!(expanded.penalty(["mei", "gan", "xi"]), 0.0);
        assert_eq!(expanded.penalty(["mei", "guan", "xia"]), 4.5);
        assert_eq!(expanded.penalty(["mei", "gang", "xi"]), 0.7);
        // 随机手滑边：guan 计 strict，模糊音 gang 不计
        assert_eq!(expanded.strict_penalty(["mei", "guan", "xia"]), 4.5);
        assert_eq!(expanded.strict_penalty(["mei", "gang", "xi"]), 0.0);
        assert_eq!(expanded.positions()[1].len(), 3);
        let plain = Expanded::new([(vec![("ni".to_owned(), 0.0, EdgeKind::Original)], true)]);
        assert!(!plain.has_alternatives());
        assert_eq!(plain.penalty(["mi"]), 0.0);
    }

    #[test]
    fn phonetic_edges_are_not_strict() {
        let mut expanded = Expanded::new([
            (vec![("shen".to_owned(), 0.0, EdgeKind::Original)], true),
            (vec![("huo".to_owned(), 0.0, EdgeKind::Original)], true),
        ]);
        expanded.push_alternative(0, "sheng", 3.5, EdgeKind::Phonetic);
        // 系统发音纠错有总代价，但不算随机手滑
        assert_eq!(expanded.penalty(["sheng", "huo"]), 3.5);
        assert_eq!(expanded.strict_penalty(["sheng", "huo"]), 0.0);
    }
}
