//! 日期、时间、星期的几种常用写法，以及 v 模式下日期串 / 时间串的解析。

use jiff::Zoned;
use jiff::civil::{Date, Weekday};

/// 星期几的中文尾字（一、二…日）。
fn weekday_label(weekday: Weekday) -> &'static str {
    match weekday {
        Weekday::Monday => "一",
        Weekday::Tuesday => "二",
        Weekday::Wednesday => "三",
        Weekday::Thursday => "四",
        Weekday::Friday => "五",
        Weekday::Saturday => "六",
        Weekday::Sunday => "日",
    }
}

/// 年月日做成三种写法：`2026年10月1日`、`2026-10-01`、`2026/10/01`。
pub fn date_forms_from(year: i16, month: i8, day: i8) -> Vec<String> {
    vec![
        format!("{year}年{month}月{day}日"),
        format!("{year}-{month:02}-{day:02}"),
        format!("{year}/{month:02}/{day:02}"),
    ]
}

/// `rq`：今天的三种写法。
pub fn date_forms(now: &Zoned) -> Vec<String> {
    date_forms_from(now.year(), now.month(), now.day())
}

/// 某个日期是星期几，写成「星期X」；日期不合法返回 `None`。
pub fn weekday_for(year: i16, month: i8, day: i8) -> Option<String> {
    let date = Date::new(year, month, day).ok()?;
    Some(format!("星期{}", weekday_label(date.weekday())))
}

/// 时间写法：`12:30`，有秒再给 `12:30:45`，最后 `12点30分`。
pub fn time_forms_from(hour: i8, minute: i8, second: Option<i8>) -> Vec<String> {
    let mut forms = vec![format!("{hour:02}:{minute:02}")];
    if let Some(second) = second {
        forms.push(format!("{hour:02}:{minute:02}:{second:02}"));
    }
    forms.push(format!("{hour}点{minute:02}分"));
    forms
}

/// `sj`：现在的时间写法（带秒）。
pub fn time_forms(now: &Zoned) -> Vec<String> {
    time_forms_from(now.hour(), now.minute(), Some(now.second()))
}

/// `xq`：`星期四`、`周四`。
pub fn weekday_forms(now: &Zoned) -> Vec<String> {
    let name = weekday_label(now.weekday());
    vec![format!("星期{name}"), format!("周{name}")]
}

/// v 模式日期串：只认完整「年-月-日」三段、年四位，分隔符 `-` `/` `.` 都行
/// （`2026-10-1`、`2026/10/1`、`2026.10.1`）。两段不判日期，留给算式（`10-1` 是减法、`10/1` 是除法），
/// 点号两段也留给小数。日期要真实存在（靠 [`Date::new`] 校验），否则返回 `None`。
pub fn parse_date(body: &str) -> Option<(i16, i8, i8)> {
    for sep in ['-', '/', '.'] {
        let parts: Vec<&str> = body.split(sep).collect();
        let [y, m, d] = parts.as_slice() else {
            continue;
        };
        if y.len() != 4 {
            continue;
        }
        let (year, month, day) = match (y.parse(), m.parse(), d.parse()) {
            (Ok(year), Ok(month), Ok(day)) => (year, month, day),
            _ => continue,
        };
        if (1..=9999).contains(&year) && Date::new(year, month, day).is_ok() {
            return Some((year, month, day));
        }
    }
    None
}

/// v 模式时间串：`12:30` 或 `12:30:45`；时、分、秒要在范围内。
pub fn parse_time(body: &str) -> Option<(i8, i8, Option<i8>)> {
    let parts: Vec<&str> = body.split(':').collect();
    let (hour, minute, second) = match parts.as_slice() {
        [h, m] => (h.parse().ok()?, m.parse().ok()?, None),
        [h, m, s] => (h.parse().ok()?, m.parse().ok()?, Some(s.parse().ok()?)),
        _ => return None,
    };
    let valid = (0..=23).contains(&hour)
        && (0..=59).contains(&minute)
        && second.is_none_or(|s| (0..=59).contains(&s));
    valid.then_some((hour, minute, second))
}
