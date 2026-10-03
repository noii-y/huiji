//! 一套配色（灰迹「石墨」主题）：深色石墨底配暖琥珀，浅色暖灰纸配低饱和赭石。

use crate::color::Color;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Palette {
    /// 候选词。
    pub text: Color,

    /// 译文。
    pub gloss: Color,

    /// 词性，比译文更浅。
    pub pos: Color,

    /// 生词译文：比普通译文醒目，看熟了就回到译文色。
    pub fresh: Color,

    /// 序号。
    pub index: Color,

    /// 云联想的云朵与文字。
    pub cloud: Color,

    /// 窗口背景。
    pub background: Color,

    /// 当前候选的高亮底色。
    pub highlight: Color,

    /// 首选左侧的强调竖条。
    pub accent: Color,
}

impl Palette {
    pub const fn light() -> Self {
        Self {
            text: Color::rgb(0x23, 0x22, 0x24),
            gloss: Color::rgb(0x9a, 0x6e, 0x3c),
            pos: Color::rgb(0x9c, 0x98, 0x92),
            fresh: Color::rgb(0xb0, 0x7e, 0x44),
            index: Color::rgb(0xa6, 0xa2, 0x9c),
            cloud: Color::rgb(0x9a, 0x6e, 0x3c),
            background: Color::rgb(0xf6, 0xf5, 0xf4),
            highlight: Color::rgb(0xe9, 0xe7, 0xe4),
            accent: Color::rgb(0xb0, 0x82, 0x4e),
        }
    }

    pub const fn dark() -> Self {
        Self {
            text: Color::rgb(0xe8, 0xe8, 0xea),
            gloss: Color::rgb(0xcd, 0x9e, 0x68),
            pos: Color::rgb(0x8a, 0x8a, 0x90),
            fresh: Color::rgb(0xd2, 0xa2, 0x6a),
            index: Color::rgb(0x7a, 0x7a, 0x80),
            cloud: Color::rgb(0xcd, 0x9e, 0x68),
            background: Color::rgb(0x22, 0x22, 0x25),
            highlight: Color::rgb(0x30, 0x30, 0x34),
            accent: Color::rgb(0xcd, 0x9e, 0x68),
        }
    }
}
