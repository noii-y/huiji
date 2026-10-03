//! 游戏模式：前台是全屏应用（独占 / 无边框全屏的游戏、全屏演示）时，输入法整体让位——
//! 不吃键、不连 Server、不弹自绘候选窗。
//!
//! 全屏独占（尤其带 ACE 这类反作弊的游戏）下，在游戏 UI 线程同步连命名管道、再往上盖分层候选窗，
//! 会卡住游戏的主循环和画面，表现为游戏卡死、但还能 Alt+Tab。让按键全直通游戏，这两件事都不会发生。
//! 带标题栏的最大化窗口不算全屏，照常输入。

use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::Graphics::Gdi::{
    GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromWindow,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GWL_STYLE, GetForegroundWindow, GetShellWindow, GetWindowLongW, GetWindowRect, IsZoomed,
    WS_CAPTION,
};

// 测试用覆盖：单测里没有真实前台窗口的控制权，用它固定全屏判定结果。
#[cfg(test)]
std::thread_local! {
    static TEST_OVERRIDE: core::cell::Cell<Option<bool>> = const { core::cell::Cell::new(None) };
}

/// 前台是否正处在全屏状态。每键实时判定，覆盖 F11、Alt+Enter 等任何切换方式，不缓存状态。
pub(crate) fn active() -> bool {
    #[cfg(test)]
    if let Some(forced) = TEST_OVERRIDE.get() {
        return forced;
    }
    detect()
}

// 测试里固定全屏判定（`None` 恢复读真实前台）。
#[cfg(test)]
pub(crate) fn force(value: Option<bool>) {
    TEST_OVERRIDE.set(value);
}

fn detect() -> bool {
    let hwnd = unsafe { GetForegroundWindow() };
    if hwnd.is_invalid() || hwnd == unsafe { GetShellWindow() } {
        return false;
    }
    // 带标题栏的最大化不算：任务栏自动隐藏时它也盖满整屏，但用户要的是照常显示候选窗。
    let zoomed_with_caption = unsafe { IsZoomed(hwnd) }.as_bool()
        && unsafe { GetWindowLongW(hwnd, GWL_STYLE) } as u32 & WS_CAPTION.0 == WS_CAPTION.0;
    if zoomed_with_caption {
        return false;
    }
    covers_monitor(hwnd)
}

/// 窗口矩形是否盖满它所在的整个显示器。
fn covers_monitor(hwnd: HWND) -> bool {
    let mut window = RECT::default();
    let mut info = MONITORINFO {
        cbSize: size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    unsafe {
        if GetWindowRect(hwnd, &mut window).is_err() {
            return false;
        }
        let monitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
        if !GetMonitorInfoW(monitor, &mut info).as_bool() {
            return false;
        }
    }
    let screen = info.rcMonitor;
    window.left <= screen.left
        && window.top <= screen.top
        && window.right >= screen.right
        && window.bottom >= screen.bottom
}
