//! 「退出灰迹」：确认后先在本地断开与 Server 的连接（不等待，Server 卡死也不影响），
//! 再把输入法切到一个非灰迹的 profile，效果等同于手动 Win+Space 切走。

use windows::Win32::UI::WindowsAndMessaging::{
    IDCANCEL, MB_DEFBUTTON2, MB_ICONQUESTION, MB_OKCANCEL, MessageBoxW,
};

use super::{TextService_Impl, switch_profile};
use crate::com::log::log;

impl TextService_Impl {
    pub(super) fn exit_huiji(&self) {
        // 确认框挂在本线程窗口上，系统模态，只等用户点按钮，不碰管道。
        let owner = self.menu_owner();
        let choice = unsafe {
            MessageBoxW(
                owner,
                windows::core::w!(
                    "退出灰迹后，将切换到 Windows 默认输入法（如微软拼音）。\n\n尚未上屏的拼音会被清除，是否继续？"
                ),
                windows::core::w!("退出灰迹"),
                MB_OKCANCEL | MB_ICONQUESTION | MB_DEFBUTTON2,
            )
        };
        if choice == IDCANCEL {
            return;
        }

        // 先本地断开：drop 管道句柄、结束组句、隐藏候选窗并清状态，全程不等待 Server。
        self.disconnect();
        self.shared.hide_candidates();
        self.shared.reset();

        // 再切 profile，整条切换路径不经过 Server。
        match switch_profile::switch_to_default() {
            Ok(()) => log("已退出灰迹，切换到 Windows 默认输入法"),
            Err(error) => log(&format!("切换输入法失败: {error}")),
        }
    }
}
