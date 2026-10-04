//! 附加候选：日期时间等快捷项、中英混输的英文词与补全、emoji。

use super::*;

impl Engine {
    /// 精确匹配自定义输入码时，数字键应选择候选。
    pub(super) fn has_custom_phrase(&self) -> bool {
        !self.english_mode
            && self
                .custom_phrases
                .iter()
                .any(|p| p.enabled && p.code == self.composition.scope())
    }

    /// 所有普通候选完成排序后按输入码固定位置。
    pub(super) fn insert_custom_phrases(&self, items: &mut Vec<Candidate>) {
        if self.english_mode {
            return;
        }
        let mut phrases: Vec<_> = self
            .custom_phrases
            .iter()
            .filter(|p| p.enabled && p.code == self.composition.scope())
            .collect();
        phrases.sort_by_key(|p| p.position);
        for phrase in phrases {
            items.insert(
                (phrase.position - 1).min(items.len()),
                Candidate {
                    text: phrase.text.clone(),
                    kind: CandidateKind::Custom(phrase.position),
                    syllables: Vec::new(),
                    reading: None,
                    translation: None,
                    aux_code: None,
                },
            );
        }
    }

    /// 日期 / 时间 / 星期这类快捷候选插在本地首选之后：`rq` 首选仍是词库里的词，快捷写法紧随其后。
    pub(super) fn insert_shortcuts(&self, items: &mut Vec<Candidate>, scope: &str) {
        let expression_char =
            if self.zhuyin && crate::zhuyin::layout::map_key(self.modes().expression).is_some() {
                '\0'
            } else {
                self.modes().expression
            };
        let shortcuts = shortcut::candidates(scope, expression_char, &jiff::Zoned::now());
        if shortcuts.is_empty() {
            return;
        }
        let position = items.len().min(1);
        items.splice(position..position, shortcuts);
    }

    /// 给英文候选用的词表，个人的在前、随包的在后；一张都没有就是空。
    /// 整段作用域本身就是个英文词（`database`、`agent`）：用户多半在打那个词。
    pub(in crate::engine) fn scope_is_english_word(&self) -> bool {
        let scope = self.composition.scope();
        !scope.is_empty()
            && self
                .english_lists()
                .iter()
                .any(|words| words.get(scope).is_some())
    }

    pub(super) fn english_lists(&self) -> Vec<&WordList> {
        self.learner
            .user_english()
            .into_iter()
            .chain(self.english.as_ref())
            .collect()
    }

    /// emoji 候选：前几个中文候选里有配 emoji 的，emoji 紧跟在那个词后面，右侧标注它对应的词。
    /// 词后面紧挨着的英文词候选（中文优先时 `key` → 可以、key）不被 emoji 挤开，emoji 排在它之后。
    pub(super) fn insert_emoji(&self, items: &mut Vec<Candidate>) {
        let Some(table) = &self.emoji else { return };
        let mut inserted = 0;
        let mut index = 0;
        let mut scanned = 0;
        while index < items.len() && scanned < EMOJI_SCAN && inserted < EMOJI_TOTAL {
            let item = &items[index];
            index += 1;
            if !matches!(
                item.kind,
                CandidateKind::Chinese | CandidateKind::Sentence | CandidateKind::English
            ) {
                continue;
            }
            scanned += 1;
            // 英文词按小写查英文表（smile → 😀）
            let key = if item.kind == CandidateKind::English {
                item.text.to_lowercase()
            } else {
                item.text.clone()
            };
            let emojis = table.lookup(&key);
            if emojis.is_empty() {
                continue;
            }
            let word = item.text.clone();
            let syllables = item.syllables.clone();
            if item.kind != CandidateKind::English {
                while items
                    .get(index)
                    .is_some_and(|next| next.kind == CandidateKind::English)
                {
                    index += 1;
                }
            }
            for emoji in emojis.iter().take(EMOJI_PER_WORD) {
                if inserted >= EMOJI_TOTAL {
                    break;
                }
                items.insert(
                    index,
                    Candidate {
                        text: emoji.clone(),
                        kind: CandidateKind::Emoji,
                        syllables: syllables.clone(),
                        reading: Some(word.clone()),
                        translation: None,
                        aux_code: None,
                    },
                );
                index += 1;
                inserted += 1;
            }
        }
    }
}
