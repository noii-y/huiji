//! 一帧要画的全部内容：顶部拼音行、候选行、高亮、页脚、右侧整句补全。只是展示形态，不含排序或查词。

mod preedit;
mod row;
mod tone;

pub use preedit::{Preedit, PreeditSegment, PreeditStyle};
pub use row::Row;
pub use tone::Tone;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Frame {
    /// 顶部拼音行；配置成只在行内显示时为 `None`。
    pub preedit: Option<Preedit>,

    /// 候选行。
    pub rows: Vec<Row>,

    /// 高亮行；`None` 不高亮。
    pub highlighted: Option<usize>,

    /// 横排展开成矩阵时每行几格，`rows` 按行优先排开、空位是空行；0 为没展开。
    pub columns: usize,

    /// 矩阵各列要留几个候选字宽（壳按整份候选估的，滚动、移动高亮时不变，窗口才不跳）；空着就按视口里的内容实测。
    pub column_ems: Vec<f32>,

    /// 右下角页码。
    pub footer: Option<String>,

    /// 拼音行右侧的外文译文（端侧翻译，离线）：只展示、不上屏。
    pub translation: Option<String>,

    /// 拼音行右侧的一句临时状态（删了什么词）：有它时不画译文。
    pub status: Option<String>,

    /// 拼音行下方的中文整句纠错（云联想）：带云朵，按 Tab 上屏。
    pub sentence: Option<String>,
}

impl Frame {
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
            && self.preedit.is_none()
            && self.trailing().is_none()
            && self.sentence.is_none()
    }

    /// 顶部要不要画一行（拼音或右侧文字任一存在）。
    pub fn has_top_line(&self) -> bool {
        self.preedit.is_some() || self.trailing().is_some()
    }

    /// 拼音行右侧画什么：状态优先，其次外文译文；都不带云朵（端侧产出、离线）。
    pub fn trailing(&self) -> Option<&str> {
        self.status.as_deref().or(self.translation.as_deref())
    }
}
