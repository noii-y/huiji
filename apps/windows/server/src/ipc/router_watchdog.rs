//! 路由循环看门狗：跑在独立线程上，不依赖主路由循环来调度。
//!
//! UI 看门狗（[`crate::ui::watchdog`]）由主路由循环在 tick 里驱动，主循环一旦在
//! `router.handle` / `router.tick` 里死锁，它也跟着停摆；此时管道仍被首个实例占着，
//! 替补 Server 全部撞车退出，输入法会永久不可用。这里每几秒以「普通客户端」身份开一条
//! 新连接、发一条轻量消息，能完整走通 accept → serve_connection → 主循环应答才算活着；
//! 连续一段时间拿不到应答，就结束整个 Server，由 TSF 重拉干净的新进程。

use std::fs::OpenOptions;
use std::os::windows::fs::OpenOptionsExt;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use qingjian_platform::protocol::{ClientMessage, SessionId, read_message, write_message};

use crate::ui::watchdog::WATCHDOG_EXIT_CODE;

/// 启动后先等多久再开始探测：避开正常启动时 UI 线程、翻译模型接入的忙乱期。
const START_GRACE: Duration = Duration::from_secs(6);

/// 两次探测之间的间隔。
const PROBE_INTERVAL: Duration = Duration::from_secs(2);

/// 发出探测后等应答的上限。
const PROBE_READ_TIMEOUT: Duration = Duration::from_millis(1500);

/// 连续探测失败多少次判定主循环死亡。约 8～14 秒窗口，足够宽松，不会因机器短暂繁忙误杀。
const MAX_FAILURES: u32 = 4;

/// 看门狗专用的会话号，避开真实 DLL 会话（取一个不会与应用进程冲突的高值）。
const WATCHDOG_SESSION: SessionId = SessionId(0xFFFF_FF00);

/// 起一条看门狗线程。`pipe_name` 是本 Server 监听的完整管道名。
pub fn spawn(pipe_name: String) {
    thread::spawn(move || run(pipe_name));
}

fn run(pipe_name: String) {
    name_thread();
    thread::sleep(START_GRACE);
    let mut failures = 0;
    loop {
        if probe(&pipe_name) {
            failures = 0;
        } else {
            failures += 1;
            if failures >= MAX_FAILURES {
                tracing::error!("路由循环看门狗：连续 {failures} 次无应答，结束 Server");
                std::process::exit(WATCHDOG_EXIT_CODE);
            }
        }
        thread::sleep(PROBE_INTERVAL);
    }
}

/// 开一条新连接、发 SyncMode、等任意应答。完整走通返回 true。
fn probe(pipe_name: &str) -> bool {
    let mut stream = match OpenOptions::new()
        .read(true)
        .write(true)
        .share_mode(0)
        .open(pipe_name)
    {
        Ok(stream) => stream,
        Err(_) => return false,
    };
    if write_message(
        &mut stream,
        &ClientMessage::SyncMode {
            session: WATCHDOG_SESSION,
        },
    )
    .is_err()
    {
        return false;
    }
    // 读应答放到独立线程，给主循环一个有界等待；超时即判本次失败。
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let result = read_message::<_, qingjian_platform::protocol::ServerMessage>(&mut stream);
        let _ = tx.send(result);
    });
    matches!(rx.recv_timeout(PROBE_READ_TIMEOUT), Ok(Ok(Some(_))))
}

/// 给看门狗线程起个名，真机测试时据此把它与其它线程区分开（只挂起主循环线程）。
#[cfg(windows)]
fn name_thread() {
    use windows::Win32::System::Threading::{GetCurrentThread, SetThreadDescription};
    use windows::core::HSTRING;
    unsafe {
        let _ = SetThreadDescription(GetCurrentThread(), &HSTRING::from("huiji-router-watchdog"));
    }
}
