//! 「通用」页：学习语言、每页候选数、输入方案、英文模式候选。

use qingjian_platform::{MAX_PAGE_SIZE, Scheme, ShiftLetter, SwitchKey};
use windows_reactor::*;

use crate::panel::controls::{feedback, field, index_of, page};
use crate::panel::{Message, Settings};

/// 学习语言：界面名 + 配置写法。
pub(crate) const LANGUAGES: [(&str, &str); 4] = [
    ("英语", "en"),
    ("日语", "ja"),
    ("西班牙语", "es"),
    ("不显示译文", "off"),
];

/// 输入方案：界面名 + 配置写法，直接照 [`Scheme::COMMON`] 建，不另抄一份。
/// 数组长度取自 `COMMON`，以后加方案时这里数组对不上就编不过。
pub(crate) const SCHEMES: [(&str, &str); Scheme::COMMON.len()] = [
    (Scheme::COMMON[0].label(), Scheme::COMMON[0].key()),
    (Scheme::COMMON[1].label(), Scheme::COMMON[1].key()),
    (Scheme::COMMON[2].label(), Scheme::COMMON[2].key()),
    (Scheme::COMMON[3].label(), Scheme::COMMON[3].key()),
    (Scheme::COMMON[4].label(), Scheme::COMMON[4].key()),
    (Scheme::COMMON[5].label(), Scheme::COMMON[5].key()),
    (Scheme::COMMON[6].label(), Scheme::COMMON[6].key()),
    (Scheme::COMMON[7].label(), Scheme::COMMON[7].key()),
    (Scheme::COMMON[8].label(), Scheme::COMMON[8].key()),
];

pub(crate) fn string_combo(
    options: &'static [(&str, &str)],
    current: &str,
    callback: Callback<Option<usize>>,
) -> ComboBox {
    ComboBox::new()
        .items_source(options.iter().map(|(label, _)| *label))
        .selected_index(index_of(options, current))
        .on_selection_changed(callback)
}

/// Shift+字母的下拉：选项直接由 [`ShiftLetter::ALL`] 生成，免得再抄一份表（顺序要和它一致）。
/// 通用页与高级页都用它，所以放开可见性。
pub(crate) fn shift_letter_combo(current: ShiftLetter, callback: Callback<Option<usize>>) -> ComboBox {
    ComboBox::new()
        .items_source(ShiftLetter::ALL.iter().map(|mode| mode.label()))
        .selected_index(ShiftLetter::ALL.iter().position(|mode| *mode == current))
        .on_selection_changed(callback)
}

pub(crate) fn view(settings: &Settings, context: &mut ViewContext<Settings>) -> View {
    let g = &settings.config.general;
    let rows = [
        field(
            "学习语言",
            "候选词右侧显示哪种语言的译词，只列出装了释义表的语言；「不显示译文」同时关掉生词标记与释义兜底。",
            string_combo(
                &LANGUAGES,
                &g.learning_language,
                context.callback(Message::LearningLanguage),
            ),
        ),
        field(
            "中英切换键",
            "勾上的键都能在中英之间切换，可以多选，改完立刻生效；中英模式所有应用共用一份。打字时容易误触 Shift 的话改勾「单击 Ctrl」；一个都不勾时只剩任务栏 / 悬浮状态条上的「中」「英」按钮。\
             系统自带的 Ctrl + Space 也能切中英，与微软拼音一致，不用勾（装了别的输入法时 Windows 可能改用它切换输入法）。",
            switch_key_boxes(settings, context),
        ),
        field(
            "启用内置英文模式",
            "关掉后灰迹固定中文模式：切换键与任务栏、悬浮状态条上的「中」「英」按钮都不再切到英文，需要英文时用系统快捷键（Win + Space）切到别的输入法。",
            ToggleSwitch::new()
                .is_on(g.english_mode)
                .on_toggled(context.callback(Message::EnglishMode)),
        ),
        field(
            "每页候选数",
            "",
            NumberBox::new()
                .minimum(1.0)
                .maximum(MAX_PAGE_SIZE as f64)
                .value(g.page_size as f64)
                .on_value_changed(context.callback(Message::PageSize)),
        ),
        field(
            "拼音方案",
            "全拼、七套双拼、大千注音。\
             双拼下 v、u、i 是音节键，表达式与问字模式改用 Shift+V、Shift+U 进（微软、搜狗方案的 ; 键是 ing）；\
             注音下 v、u、i 也是按键，只能用 ? 开头进。",
            string_combo(
                &SCHEMES,
                g.scheme().key(),
                context.callback(Message::Scheme),
            ),
        ),
        field(
            "繁体输出",
            "打字时将候选词转换为繁体中文。",
            ToggleSwitch::new()
                .is_on(g.traditional)
                .on_toggled(context.callback(Message::Traditional)),
        ),
        field(
            "中文模式标点转全角",
            "没在打拼音时敲 , . ? ! 等出「，。？！」，数字后面的点保持半角；悬浮状态条的「，。」格也能切，切的是当前模式那份。",
            ToggleSwitch::new()
                .is_on(g.full_width_punctuation)
                .on_toggled(context.callback(Message::FullWidthPunctuation)),
        ),
        field(
            "英文模式（Caps Lock）也给候选",
            "Tab 或方向键选词；空格、回车、标点仍原样上屏敲的字母，不选词时与直接打字一样。",
            ToggleSwitch::new()
                .is_on(g.english_candidates)
                .on_toggled(context.callback(Message::EnglishCandidates)),
        ),
        feedback(&settings.notice),
    ];
    page("通用", StackPanel::new().spacing(16.0).children(rows))
}

/// 中英切换键：每个键一个勾选框，横排。
fn switch_key_boxes(settings: &Settings, context: &mut ViewContext<Settings>) -> View {
    let keys = settings.config.shortcut.switch_mode;
    let boxes = SwitchKey::ALL.map(|key| {
        CheckBox::new()
            .is_checked(keys.contains(key))
            .on_is_checked_changed(context.callback(move |on| Message::SwitchKey(key, on)))
            .content(key.label())
    });
    StackPanel::new()
        .orientation(Orientation::Horizontal)
        .spacing(12.0)
        .children(boxes)
}
