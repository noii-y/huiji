//! 候选窗口：不抢焦点、置顶的分层窗口，跟随光标，画拼音行与候选列表，四周柔和阴影。
//! 缺省交给青简渲染器出位图再贴（[`super::painter`]），配置 `renderer = "system"` 时走 GDI：绘制在 [`view`]，
//! 配色 / 字体在 [`theme`]。绘制内容在 [`RenderData`]，一行的展示形态在 [`row`]。设计语言对齐 macOS 端。

mod render_data;
pub(crate) mod row;
pub(crate) mod theme;
pub(crate) mod view;

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use windows::Win32::Foundation::{E_INVALIDARG, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{GetDC, ReleaseDC};
use windows::Win32::UI::HiDpi::{GetDpiForSystem, GetDpiForWindow};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, IDC_ARROW, IsWindow, LoadCursorW, SW_HIDE,
    SW_SHOWNA, ShowWindow, WNDCLASSEXW, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
    WS_EX_TOPMOST, WS_POPUP,
};
use windows::core::{Error, PCWSTR, Result, w};

use qingjian_platform::ThemeMode;
use qingjian_platform::protocol::Frame;

pub(crate) use self::render_data::RenderData;
use self::theme::Theme;
use super::layered::{self, Layered};
use super::monitor;
use super::painter::SharedPainter;
use super::window_class::WindowClass;

const CLASS_NAME: PCWSTR = w!("QingjianCandidateWindow");
static CLASS: WindowClass = WindowClass::new();

/// 光标行与候选窗之间的间隙（逻辑像素）。
const CARET_GAP: i32 = 2;

/// 按外观模式解析深浅；`System` 读系统主题。
pub(super) fn resolve_dark(mode: ThemeMode) -> bool {
    match mode {
        ThemeMode::Light => false,
        ThemeMode::Dark => true,
        ThemeMode::System => system_prefers_dark(),
    }
}

/// `HKCU\...\Themes\Personalize\AppsUseLightTheme` 为 0 是深色；读不到当浅色。
fn system_prefers_dark() -> bool {
    windows_registry::CURRENT_USER
        .open(r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize")
        .and_then(|key| key.get_u32("AppsUseLightTheme"))
        .is_ok_and(|value| value == 0)
}

/// 候选窗口。内容经 `UpdateLayeredWindow` 一次贴上，窗口过程只走默认处理。
pub(crate) struct CandidateWindow {
    /// 分层窗口句柄；呈现失败或句柄失效时重建（见 `ensure_window`），故用 Cell。
    hwnd: Cell<HWND>,

    /// 绘制内容。
    data: RefCell<RenderData>,

    /// 上次用的 DPI，变了重建字体。
    dpi: Cell<u32>,

    /// 上次解析出的深浅，变了重建配色。
    dark: Cell<bool>,

    /// 上次记进日志的缩放值（窗口 DPI、光标所在显示器 DPI）：变了才再记一条（#146）。
    logged_dpi: Cell<Option<(u32, Option<u32>)>>,

    /// 青简渲染器；`None` 走 GDI。
    painter: SharedPainter,
}

impl CandidateWindow {
    /// 建一个隐藏的候选窗口。
    pub(crate) fn new(painter: SharedPainter) -> Result<Self> {
        let dpi = unsafe { GetDpiForSystem() }.max(96);
        let dark = resolve_dark(ThemeMode::default());
        let data = RefCell::new(RenderData::empty(Rc::new(Theme::new(dpi, dark))));
        let hwnd = Self::create_hwnd()?;
        Ok(Self {
            hwnd: Cell::new(hwnd),
            data,
            dpi: Cell::new(dpi),
            dark: Cell::new(dark),
            logged_dpi: Cell::new(None),
            painter,
        })
    }

    /// 注册窗口类并建一个隐藏的分层窗口。NOACTIVATE：显示时不抢应用焦点。
    fn create_hwnd() -> Result<HWND> {
        CLASS.ensure(|| WNDCLASSEXW {
            lpfnWndProc: Some(wndproc),
            hInstance: super::module_handle(),
            hCursor: unsafe { LoadCursorW(None, IDC_ARROW) }.unwrap_or_default(),
            lpszClassName: CLASS_NAME,
            ..Default::default()
        })?;
        unsafe {
            CreateWindowExW(
                WS_EX_LAYERED | WS_EX_TOOLWINDOW | WS_EX_TOPMOST | WS_EX_NOACTIVATE,
                CLASS_NAME,
                w!("灰迹候选"),
                WS_POPUP,
                0,
                0,
                0,
                0,
                None,
                None,
                Some(super::module_handle()),
                None,
            )
        }
    }

    /// 句柄仍有效直接用；失效（窗口被销毁）则重建。
    fn ensure_window(&self) -> Result<()> {
        if unsafe { IsWindow(Some(self.hwnd.get())) }.as_bool() {
            return Ok(());
        }
        tracing::warn!("候选窗口句柄失效，重建窗口");
        self.hwnd.set(Self::create_hwnd()?);
        Ok(())
    }

    /// 不管句柄是否有效，销毁旧窗口并重建。用于句柄还在、但分层呈现状态损坏的情况。
    fn force_rebuild(&self) -> Result<()> {
        let old = self.hwnd.get();
        if unsafe { IsWindow(Some(old)) }.as_bool() {
            let _ = unsafe { DestroyWindow(old) };
        }
        self.hwnd.set(Self::create_hwnd()?);
        Ok(())
    }

    /// 刷新内容（不定位、不显示）。
    pub(crate) fn set_content(&self, frame: &Frame) {
        self.data.borrow_mut().set(frame);
    }

    /// 按光标矩形定位并显示：贴光标下方（放不下放上方），四周留出阴影。
    /// 呈现失败时重建窗口重试一次，避免窗口状态损坏后一直不弹。
    pub(crate) fn show(&self, anchor: RECT) {
        let _ = self.ensure_window();
        let mut outcome = self.present_frame(anchor);
        if let Err(error) = &outcome {
            tracing::warn!(%error, "候选窗口呈现失败，重建窗口后重试");
            if self.force_rebuild().is_ok() {
                outcome = self.present_frame(anchor);
            }
        }
        match outcome {
            Ok(true) => {
                let _ = unsafe { ShowWindow(self.hwnd.get(), SW_SHOWNA) };
            }
            Ok(false) => {} // 内容为空，present_frame 里已 hide
            Err(error) => {
                tracing::error!(%error, "候选窗口重建后仍呈现失败");
                self.hide();
            }
        }
    }

    /// 渲染并把内容贴上分层窗口。
    /// 返回 Ok(true) 已贴上可显示；Ok(false) 内容为空、已隐藏；Err 呈现失败。
    fn present_frame(&self, anchor: RECT) -> Result<bool> {
        self.sync_theme(anchor);
        let rendered = {
            let data = self.data.borrow();
            self.painter.borrow_mut().as_mut().and_then(|painter| {
                painter.render_frame(
                    &data.render_frame(),
                    data.layout,
                    self.dark.get(),
                    self.dpi.get(),
                )
            })
        };
        match rendered {
            Some(rendered) => {
                let content = (
                    rendered.content_width as i32,
                    rendered.content_height as i32,
                );
                if content.0 <= 0 || content.1 <= 0 {
                    self.hide();
                    return Ok(false);
                }
                let (content_x, content_y) = place(anchor, content);
                layered::present(
                    self.hwnd.get(),
                    &rendered.pixmap,
                    (
                        content_x - rendered.content_x as i32,
                        content_y - rendered.content_y as i32,
                    ),
                )?;
                Ok(true)
            }
            None => {
                self.show_gdi(anchor)?;
                Ok(true)
            }
        }
    }

    /// GDI 画法：量尺寸、定位、合成。
    fn show_gdi(&self, anchor: RECT) -> Result<()> {
        let margin = layered::shadow_margin(self.dpi.get());
        let content = self.preferred_size();
        if content.0 <= 0 || content.1 <= 0 {
            return Err(Error::from(E_INVALIDARG));
        }
        let (content_x, content_y) = place(anchor, content);
        let data = self.data.borrow();
        layered::composite(
            self.hwnd.get(),
            &Layered {
                content,
                margin,
                win_pos: (content_x - margin, content_y - margin),
                win_size: (content.0 + margin * 2, content.1 + margin * 2),
                background: data.theme.background,
                corner_radius: data.theme.corner_radius,
                paint: &|hdc, client| view::paint(hdc, &data, client),
            },
        )
    }

    pub(crate) fn hide(&self) {
        let _ = unsafe { ShowWindow(self.hwnd.get(), SW_HIDE) };
    }

    /// DPI 或深浅变了就重建主题；每次 `show` 前调。
    ///
    /// DPI 取光标所在显示器的：窗口藏着时改了缩放（或睡眠唤醒后多显示器重排），
    /// `GetDpiForWindow` 会停在旧值，候选字就大小不对（#146）。
    fn sync_theme(&self, anchor: RECT) {
        let caret = POINT {
            x: anchor.left,
            y: anchor.top,
        };
        let monitor_dpi = monitor::dpi_near(caret);
        let window_dpi = unsafe { GetDpiForWindow(self.hwnd.get()) };
        let dpi = match (monitor_dpi, window_dpi) {
            (Some(dpi), _) => dpi,
            (None, 0) => self.dpi.get(),
            (None, dpi) => dpi,
        };
        self.log_dpi(caret, window_dpi, monitor_dpi, dpi);
        let dark = resolve_dark(self.data.borrow().theme_mode);
        if dpi != self.dpi.get() || dark != self.dark.get() {
            self.data.borrow_mut().theme = Rc::new(Theme::new(dpi, dark));
            self.dpi.set(dpi);
            self.dark.set(dark);
        }
    }

    /// 缩放值变了就记一条，多显示器 / 睡眠唤醒的问题从日志里能看出取到的是哪个值（#146）。
    fn log_dpi(&self, caret: POINT, window_dpi: u32, monitor_dpi: Option<u32>, used: u32) {
        if self.logged_dpi.replace(Some((window_dpi, monitor_dpi)))
            == Some((window_dpi, monitor_dpi))
        {
            return;
        }
        tracing::info!(
            window_dpi,
            ?monitor_dpi,
            used,
            system_dpi = unsafe { GetDpiForSystem() },
            caret_x = caret.x,
            caret_y = caret.y,
            "候选窗口缩放值"
        );
    }

    /// 内容需要的大小（不含阴影留白）。
    fn preferred_size(&self) -> (i32, i32) {
        let hdc = unsafe { GetDC(Some(self.hwnd.get())) };
        let size = view::preferred_size(hdc, &self.data.borrow());
        unsafe { ReleaseDC(Some(self.hwnd.get()), hdc) };
        (size.cx, size.cy)
    }
}

impl Drop for CandidateWindow {
    fn drop(&mut self) {
        let _ = unsafe { DestroyWindow(self.hwnd.get()) };
    }
}

/// 内容左上角：贴光标下方，放不下放上方，再放不下贴屏幕内；都夹在所在显示器工作区里。
fn place(anchor: RECT, content: (i32, i32)) -> (i32, i32) {
    let work = monitor::work_area_near(POINT {
        x: anchor.left,
        y: anchor.top,
    });
    let x = anchor
        .left
        .clamp(work.left, (work.right - content.0).max(work.left));
    let below = anchor.bottom + CARET_GAP;
    let above = anchor.top - CARET_GAP - content.1;
    let y = if below + content.1 <= work.bottom {
        below
    } else if above >= work.top {
        above
    } else {
        (work.bottom - content.1).max(work.top)
    };
    (x, y)
}

/// 分层窗口无需 `WM_PAINT`，全交默认处理。
unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use windows::Win32::UI::WindowsAndMessaging::IsWindow;

    use super::{CandidateWindow, DestroyWindow};

    #[test]
    fn rebuilds_window_after_destroyed() {
        // 建窗口走 user32，无需先初始化 COM。
        let painter = Rc::new(RefCell::new(None));
        let window = CandidateWindow::new(painter).expect("建候选窗");
        let hwnd = window.hwnd.get();
        assert!(unsafe { IsWindow(Some(hwnd)) }.as_bool());

        // 模拟窗口句柄失效（被销毁）。
        unsafe { DestroyWindow(hwnd) }.expect("销毁窗口");
        assert!(!unsafe { IsWindow(Some(hwnd)) }.as_bool());

        // ensure_window 应重建出新窗口。
        window.ensure_window().expect("重建窗口");
        let rebuilt = window.hwnd.get();
        assert!(unsafe { IsWindow(Some(rebuilt)) }.as_bool());
        assert_ne!(rebuilt, hwnd);
    }
}
