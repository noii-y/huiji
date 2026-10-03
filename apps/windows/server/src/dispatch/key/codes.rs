//! 按键分派用的虚拟键码与字符解析。

use qingjian_platform::protocol::KeyEvent;

pub(crate) const BACK: u32 = 0x08;
pub(crate) const TAB: u32 = 0x09;
pub(crate) const RETURN: u32 = 0x0D;
pub(crate) const ESCAPE: u32 = 0x1B;
pub(crate) const PRIOR: u32 = 0x21;
pub(crate) const NEXT: u32 = 0x22;
pub(crate) const END: u32 = 0x23;
pub(crate) const HOME: u32 = 0x24;
pub(crate) const LEFT: u32 = 0x25;
pub(crate) const UP: u32 = 0x26;
pub(crate) const RIGHT: u32 = 0x27;
pub(crate) const DOWN: u32 = 0x28;
/// 主键盘句点键 `.`（VK_OEM_PERIOD）：靠它识别 Ctrl+. 这个快捷键。
pub(crate) const OEM_PERIOD: u32 = 0xBE;
/// 字母 F 键：靠它识别 Ctrl+Shift+F 简繁切换。
pub(crate) const KEY_F: u32 = 0x46;
/// 空格键（VK_SPACE）：靠它识别 Shift+空格 全半角切换。
pub(crate) const SPACE: u32 = 0x20;

/// 小键盘减号 / 加号 VK：组句中固定为上一页 / 下一页，与配置的翻页档无关。
pub(crate) const KEYPAD_SUBTRACT: u32 = 0x6D;
pub(crate) const KEYPAD_ADD: u32 = 0x6B;

/// 翻页键对 `(上一页, 下一页)`：返回 -1 / +1。
pub(crate) fn page_key(event: &KeyEvent, page_keys: (char, char)) -> Option<isize> {
    // 小键盘 +/- 任何配置档下都翻页：小键盘是专门的数字 / 翻页区，字符 '+' 也不匹配主键盘 '='
    match event.virtual_key {
        KEYPAD_SUBTRACT => return Some(-1),
        KEYPAD_ADD => return Some(1),
        _ => {}
    }
    let c = event.character?;
    if c == page_keys.0 {
        Some(-1)
    } else if c == page_keys.1 {
        Some(1)
    } else {
        None
    }
}

/// 敲出来是数字 1–9 的键（选候选用）：按 `character` 认，Shift 出的 `!@#` 不算。
pub(crate) fn digit(event: &KeyEvent) -> Option<usize> {
    match event.character {
        Some(c) => ('1'..='9').contains(&c).then(|| c as usize - '0' as usize),
        None => digit_key(event.virtual_key),
    }
}

/// 主键盘区数字键 1–9 的键码，不管修饰键（修饰键 + 数字的快捷键按键位认）。
pub(crate) fn digit_key(virtual_key: u32) -> Option<usize> {
    (0x31..=0x39)
        .contains(&virtual_key)
        .then(|| (virtual_key - 0x30) as usize)
}

/// 小键盘区的键（数字与 `* + - . /`）：敲出来的标点一律半角。
pub(crate) fn is_keypad(virtual_key: u32) -> bool {
    (0x60..=0x6F).contains(&virtual_key)
}

#[cfg(test)]
mod tests {
    use super::*;
    use qingjian_platform::protocol::{KeyEvent, KeyModifiers};

    fn key(virtual_key: u32, character: Option<char>) -> KeyEvent {
        KeyEvent::new(virtual_key, character, KeyModifiers::default())
    }

    #[test]
    fn keypad_plus_minus_pages_regardless_of_config() {
        // 默认 -= 档
        assert_eq!(
            page_key(&key(KEYPAD_SUBTRACT, Some('-')), ('-', '=')),
            Some(-1)
        );
        assert_eq!(page_key(&key(KEYPAD_ADD, Some('+')), ('-', '=')), Some(1));
        // 换成 [] 档，小键盘 +/- 照样成对翻页
        assert_eq!(
            page_key(&key(KEYPAD_SUBTRACT, Some('-')), ('[', ']')),
            Some(-1)
        );
        assert_eq!(page_key(&key(KEYPAD_ADD, Some('+')), ('[', ']')), Some(1));
    }

    #[test]
    fn main_keys_follow_the_configured_pair() {
        let pair = ('-', '=');
        assert_eq!(page_key(&key(0xBD, Some('-')), pair), Some(-1)); // 主键盘 -
        assert_eq!(page_key(&key(0xBB, Some('=')), pair), Some(1)); // 主键盘 =
        assert_eq!(page_key(&key(0x41, Some('a')), pair), None);
    }
}
