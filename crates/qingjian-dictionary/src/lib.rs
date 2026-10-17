//! 词库：按拼音音节序列查词。
//!
//! 纯数据层，不依赖任何兄弟 crate。文件格式为 TSV：
//!
//! ```text
//! 词\t音节（空格分隔）\t词频
//! 开发\tkai fa\t9000
//! ```
//!
//! `#` 开头为注释行，空行忽略。
//!
//! 内存布局面向「几十万到上百万条常驻」：词文本与拼音键各放一个连续 arena，
//! 词目只存偏移与词频，键排序后二分定位、顺序扫描前缀范围。
//!
//! 辅码码表（[`AuxCodeTable`]）是另一套数据：词 → 码的映射，按词文本排序 + 哈希索引定位（见 `aux_code_table` 模块）。
//! 另有两种旁支：形码码表 [`CodeTable`]（键是五笔编码，按前缀查）与英文词表 [`WordList`]。

mod aux_code_table;
mod code_table;
mod dictionary;
mod error;
pub mod import;
mod matching;
mod pattern;
mod word_list;

/// 剥掉文本开头的 UTF-8 BOM（U+FEFF）。
///
/// `std::fs::read_to_string` 不解码 BOM，带 BOM 的文件首行会被当成 `U+FEFF+词` 入库。
/// BOM 只可能出现在整个文件的第一个字符，所以在各解析入口剥一次即可；
/// 不要逐行剥——正文里（罕见但合法）出现的同字符会被误伤。幂等，重复调用安全。
pub(crate) fn strip_utf8_bom(text: &str) -> &str {
    text.strip_prefix('﻿').unwrap_or(text)
}

pub use aux_code_table::{
    AuxCodeLookup, AuxCodeTable, AuxCodeTableImport, AuxCodeTableImportReport, AuxCodeTableInfo,
    aux_code_table_info, import_aux_code_table,
};
pub use code_table::CodeTable;
pub use dictionary::Dictionary;
pub use error::DictionaryError;
pub use matching::Match;
pub use pattern::{SyllablePattern, canonical_syllable};
pub use word_list::WordList;
