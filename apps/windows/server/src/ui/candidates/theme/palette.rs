//! 浅色 / 深色配色（灰迹「石墨」主题）。

use windows::Win32::Foundation::COLORREF;

use super::rgb;

/// 浅色 / 深色各一套。
pub(super) struct Palette {
    pub(super) text_color: COLORREF,
    pub(super) gloss_color: COLORREF,
    pub(super) pos_color: COLORREF,
    pub(super) fresh_color: COLORREF,
    pub(super) index_color: COLORREF,
    pub(super) cloud_color: COLORREF,
    pub(super) background: COLORREF,
    pub(super) highlight: COLORREF,
    /// 首选左侧的强调竖条。
    pub(super) accent: COLORREF,
}

impl Palette {
    /// 浅色：暖灰纸面，强调色用低饱和赭石。
    pub(super) fn light() -> Self {
        Self {
            text_color: rgb(0x23, 0x22, 0x24),
            gloss_color: rgb(0x9a, 0x6e, 0x3c),
            pos_color: rgb(0x9c, 0x98, 0x92),
            fresh_color: rgb(0xb0, 0x7e, 0x44),
            index_color: rgb(0xa6, 0xa2, 0x9c),
            cloud_color: rgb(0x9a, 0x6e, 0x3c),
            background: rgb(0xf6, 0xf5, 0xf4),
            highlight: rgb(0xe9, 0xe7, 0xe4),
            accent: rgb(0xb0, 0x82, 0x4e),
        }
    }

    /// 深色：石墨底，强调色用暖琥珀。
    pub(super) fn dark() -> Self {
        Self {
            text_color: rgb(0xe8, 0xe8, 0xea),
            gloss_color: rgb(0xcd, 0x9e, 0x68),
            pos_color: rgb(0x8a, 0x8a, 0x90),
            fresh_color: rgb(0xd2, 0xa2, 0x6a),
            index_color: rgb(0x7a, 0x7a, 0x80),
            cloud_color: rgb(0xcd, 0x9e, 0x68),
            background: rgb(0x22, 0x22, 0x25),
            highlight: rgb(0x30, 0x30, 0x34),
            accent: rgb(0xcd, 0x9e, 0x68),
        }
    }
}
