//! `cfg(windows)`：打开 Server 的命名管道，得到一条双工流交给 [`EngineClient`](super::EngineClient)。

use std::fs::{File, OpenOptions};
use std::io;
use std::os::windows::fs::OpenOptionsExt;

use windows::Win32::Foundation::ERROR_PIPE_BUSY;
use windows::Win32::System::Pipes::WaitNamedPipeW;
use windows::core::HSTRING;

use qingjian_platform::protocol::session_pipe_name;

/// 连好的命名管道。对端关闭时读到 EOF；`flush` 是空操作（管道上 `FlushFileBuffers` 会阻塞到对端读完）。
pub type PipeStream = File;

/// 实例都被占着时等一个可用实例的超时（毫秒）与重试次数。要短：这里在应用 UI 线程上，等久了 TSF 看门狗会切走输入法。
const BUSY_WAIT_MS: u32 = 300;
const BUSY_RETRIES: u32 = 1;

/// 当前进程所在的终端服务会话 id。DLL 与它拉起的 Server 同会话，用它拼同一个会话管道名。
pub(crate) fn current_session_id() -> u32 {
    let mut session = 0u32;
    let _ = unsafe {
        windows::Win32::System::RemoteDesktop::ProcessIdToSessionId(
            windows::Win32::System::Threading::GetCurrentProcessId(),
            &mut session,
        )
    };
    session
}

pub fn connect_default() -> io::Result<PipeStream> {
    // 连本会话专属管道：别的会话即使先起了 Server，也不会让本会话连到窗口画在别处的实例。
    connect(&session_pipe_name(current_session_id()))
}

pub fn connect(name: &str) -> io::Result<PipeStream> {
    let mut attempts = 0;
    loop {
        match OpenOptions::new()
            .read(true)
            .write(true)
            .share_mode(0)
            .open(name)
        {
            Err(error)
                if error.raw_os_error() == Some(ERROR_PIPE_BUSY.0 as i32)
                    && attempts < BUSY_RETRIES =>
            {
                attempts += 1;
                let _ = unsafe { WaitNamedPipeW(&HSTRING::from(name), BUSY_WAIT_MS) };
            }
            result => return result,
        }
    }
}
