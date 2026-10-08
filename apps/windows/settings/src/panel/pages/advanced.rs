//! 「高级」页：打开配置文件 / 数据目录 / 日志目录、详细日志、学习开关、输入日志。

use qingjian_platform::LogLevel;
use windows_reactor::*;

use super::general;
use crate::panel::controls::{field, note, page};
use crate::panel::{Message, Settings};

pub(crate) fn view(settings: &Settings, context: &mut ViewContext<Settings>) -> View {
    let g = &settings.config.general;
    let english_off = !settings.config.apps.english_candidates_off.is_empty();
    let rows = [
        note("设置改完会自动生效（Server 每秒看一次配置文件）。只有换学习语言要重启 Server。"),
        field(
            "配置文件",
            "",
            Button::new()
                .on_click(context.message(Message::OpenConfigFile))
                .content("在记事本中打开"),
        ),
        field(
            "数据目录",
            "",
            Button::new()
                .on_click(context.message(Message::OpenDataDir))
                .content("打开数据目录"),
        ),
        field(
            "日志目录",
            "输入法、引擎与设置程序的日志都在这一个目录，按天分文件，保留 7 天。",
            StackPanel::new()
                .orientation(Orientation::Horizontal)
                .spacing(8.0)
                .children((
                    Button::new()
                        .on_click(context.message(Message::OpenLogDir))
                        .content("打开日志目录"),
                    Button::new()
                        .on_click(context.message(Message::ExportLogs))
                        .content("打包日志到桌面"),
                )),
        ),
        field(
            "详细日志",
            "排查问题时临时打开，会记下敲的拼音与上屏文字。",
            ToggleSwitch::new()
                .is_on(g.log_level == LogLevel::Debug)
                .on_toggled(context.callback(Message::VerboseLog)),
        ),
        field(
            "学习输入习惯",
            "按你的选择调整候选顺序、记新词与敲错纠正。关掉后不再学，已学的仍参与排序；学习数据在数据目录里。",
            ToggleSwitch::new()
                .is_on(g.learning)
                .on_toggled(context.callback(Message::Learning)),
        ),
        field(
            "记录输入日志",
            "每次上屏记一行，只写本机、不上传，用于离线评测与个人模型。",
            ToggleSwitch::new()
                .is_on(g.input_log)
                .on_toggled(context.callback(Message::InputLog)),
        ),
        field(
            "清空输入日志",
            "",
            Button::new()
                .on_click(context.message(Message::ClearInputLog))
                .content("清空输入日志"),
        ),

        // —— 一般不用改的输入选项：从「通用」页挪过来，给需要的人留着 ——
        note("一般不用改的输入选项"),
        field(
            "双拼在输入框显示原始按键",
            "勾上后双拼模式下输入框（光标处）显示敲击的英文字母，回车可直接上屏；候选窗口顶部的拼音行照旧显示解码全拼。",
            ToggleSwitch::new()
                .is_on(g.shuangpin_raw_preedit)
                .is_enabled(g.scheme().is_shuangpin())
                .on_toggled(context.callback(Message::ShuangpinRawPreedit)),
        ),
        field(
            "中文模式下的 Shift + 字母",
            "「交给应用」是临时打英文（与以前一致）：拼音先上屏，这个键归应用；\
             「进组句」把它收进拼音缓冲区，匹配时按小写算，所以 Cpan 与 cpan 一样能出「C盘」。",
            general::shift_letter_combo(g.shift_letter, context.callback(Message::ShiftLetter)),
        ),
        field(
            "中文候选排在英文词前面",
            "开着时整段输入是英文词时（hello、key）英文词排第二，空格上屏的仍是中文；关着（缺省）拼音不成立的输入英文词排第一。",
            ToggleSwitch::new()
                .is_on(g.chinese_first)
                .on_toggled(context.callback(Message::ChineseFirst)),
        ),
        field(
            "英文模式标点转全角",
            "中英各记一份，缺省英文半角。",
            ToggleSwitch::new()
                .is_on(g.english_full_width_punctuation)
                .on_toggled(context.callback(Message::EnglishFullWidthPunctuation)),
        ),
        field(
            "终端和代码编辑器里不给英文候选",
            "终端、Windows Terminal、VS Code、Cursor、JetBrains 等，那里的候选窗口会挡住应用自己的补全；名单可在配置文件里改。",
            ToggleSwitch::new()
                .is_on(english_off)
                .is_enabled(g.english_candidates)
                .on_toggled(context.callback(Message::EnglishOffInApps)),
        ),
    ];
    page("高级", StackPanel::new().spacing(16.0).children(rows))
}
