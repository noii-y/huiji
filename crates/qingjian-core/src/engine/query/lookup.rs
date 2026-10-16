//! 词级查找：每个位置展开成多种写法，再按这些写法查主词库与用户词。

use super::*;

impl Engine {
    /// 每个位置的写法：敲的原样、模糊音，再加音节级敲错变体（`correction::typo`）当带代价的边，
    /// 代价按类别定、按个人敲错表打折。太短的输入（不到 [`correction::MIN_LETTERS`]）、双拼、非末尾带简拼的切分不加敲错变体：
    /// 短串一处编辑几乎总能凑出别的词，双拼敲错一键换掉的是整个声母 / 韵母。不完整的位置（简拼、前缀）本来就按前缀查，不加。
    pub(in crate::engine) fn expand_positions(
        &self,
        patterns: &[qingjian_dictionary::SyllablePattern<'_>],
        typos: bool,
    ) -> Expanded {
        let mut expanded = self.fuzzy.expand(patterns);
        if !typos {
            return expanded;
        }
        let letters: usize = patterns.iter().map(|p| p.text.len()).sum();
        // 非末尾有简拼 / 残缺音节的切分（`kai f a`）本来就不是用户敲的原话，不在它上面再猜敲错
        let inner_abbreviated = patterns
            .iter()
            .take(patterns.len().saturating_sub(1))
            .any(|p| !p.complete);
        if self.shuangpin.is_some() || letters < correction::MIN_LETTERS || inner_abbreviated {
            return expanded;
        }
        for (index, pattern) in patterns.iter().enumerate() {
            if !pattern.complete {
                continue;
            }
            for (text, kind) in typo::variants(pattern.text) {
                let accepted = self.learner.typo_count(pattern.text, text);
                // 系统发音漏键（前后鼻音尾 g、平翘舌声母 h）：保留声母、只动一个固定音素，
                // 成系统而非随机手滑，给低代价且标成 Phonetic（不被原样词保护一刀切退回）；
                // 其余编辑（相邻键误碰、换位、多漏键）是随机手滑，按物理误触处理。
                let (cost, edge) = if typo::is_velar_nasal_edit(pattern.text, &text) {
                    (self.typo_costs.velar_nasal_cost(accepted), EdgeKind::Phonetic)
                } else if typo::is_retroflex_edit(pattern.text, &text) {
                    (self.typo_costs.retroflex_cost(accepted), EdgeKind::Phonetic)
                } else {
                    (self.typo_costs.typo_cost(*kind, accepted), EdgeKind::Physical)
                };
                expanded.push_alternative(index, text, cost, edge);
            }
        }
        expanded
    }

    /// 本地整句转换把最优切分转成的汉字，给云端当参考（问字模式里就是问题的汉字形式）；转不出或有占位音节为空。
    pub(in crate::engine) fn local_guess(&self, segmentations: &[Segmentation]) -> String {
        // 先让同音节数的切分公平竞争（henan 该给河南而不是很暗）；
        // 用户有敲错时各切分都带纠错代价，竞争会落空，再回退首选切分的原样/纠错结果。
        if let Some(cross) = self.best_segmentation_sentence(segmentations, true) {
            return cross.text;
        }
        segmentations
            .first()
            .and_then(|best| self.convert_sentence(&best.patterns(), true))
            .filter(|conversion| !conversion.has_placeholder())
            .map(|conversion| conversion.text)
            .unwrap_or_default()
    }

    /// 主词库与用户词一起查（每个位置多种写法）。用户词是用户自己选过的（云联想接受的词等），排序上靠 weight 自然靠前。
    pub(in crate::engine) fn lookup_all(
        &self,
        positions: &[Vec<qingjian_dictionary::SyllablePattern<'_>>],
    ) -> Vec<Match<'_>> {
        let mut hits = self.dictionary.lookup_pattern_alt(positions);
        for dictionary in self.all_dictionaries().into_iter().skip(1) {
            hits.extend(dictionary.lookup_pattern_alt(positions));
        }
        hits
    }

    /// 只要音节数正好等于位置数的词，主词库与用户词一起查。
    pub(in crate::engine) fn lookup_exact_all(
        &self,
        positions: &[Vec<qingjian_dictionary::SyllablePattern<'_>>],
    ) -> Vec<Match<'_>> {
        let mut hits = self.dictionary.lookup_exact_alt(positions);
        for dictionary in self.all_dictionaries().into_iter().skip(1) {
            hits.extend(dictionary.lookup_exact_alt(positions));
        }
        hits
    }
}
