//! 连 Server：激活 / 获焦时开会话，失败按 [`RECONNECT_INTERVAL`] 退避重试；转发出错就断开、下一键重连。

use std::time::{Duration, Instant};

use super::launch;
use super::{RECONNECT_INTERVAL, TextService_Impl};
use crate::client::EngineClient;
use crate::client::pipe::connect_default;
use crate::com::log::log;

impl TextService_Impl {
    /// 尝试建立一条到 Server 的连接（不做退避判断、不触发拉起），成功就把客户端放进引擎槽。
    /// 拆出来让有界等待可以反复轮询。
    fn try_open(&self) -> bool {
        if self.engine.borrow().is_some() {
            return true;
        }
        let session = crate::com::session_id();
        let app = crate::com::host_app_name();
        let connected = connect_default()
            .map_err(|e| e.to_string())
            .and_then(|stream| EngineClient::open(stream, session, app).map_err(|e| e.to_string()));
        match connected {
            Ok((client, input)) => {
                *self.engine.borrow_mut() = Some(client);
                self.last_connect_failure.set(None);
                log("已连上 Server");
                // 按键行为设置随 `OpenSession` 的回包一起下来（DLL 不读配置文件，AppContainer 里读不到）。
                self.apply_input_settings(input);
                true
            }
            Err(error) => {
                log(&format!(
                    "连 Server 失败（qingjian-server 没起？）: {error}"
                ));
                false
            }
        }
    }

    /// 连 Server 并开会话（会话号见 [`crate::com::session_id`]，带上宿主 exe 名）。
    pub(super) fn connect(&self) {
        if self.try_open() {
            return;
        }
        self.last_connect_failure.set(Some(Instant::now()));
        // Server 只在登录时由「启动」文件夹拉起，中途挂了以前只能等下次登录；
        // 这里自己起一次（进程内冷却 + 跨进程互斥体，不会砸出一串 Server）。
        if launch::launch_server() {
            // 不等 RECONNECT_INTERVAL：Server 一百多毫秒就监听管道，下一键就该连上
            self.last_connect_failure.set(None);
        }
    }

    /// 没连上就重连一次（距上次失败不到 [`RECONNECT_INTERVAL`] 则跳过）。返回此刻是否连着。
    pub(super) fn ensure_connected(&self) -> bool {
        if self.engine.borrow().is_some() {
            return true;
        }
        let recently_failed = self
            .last_connect_failure
            .get()
            .is_some_and(|at| at.elapsed() < RECONNECT_INTERVAL);
        if recently_failed {
            return false;
        }
        self.connect();
        let connected = self.engine.borrow().is_some();
        // 重连上的多半是重启过的 Server，它不知道当前模式：前台这边报一次，成为全局模式。
        if connected && self.shared.foreground() {
            self.report_mode();
        }
        connected
    }

    /// 在按键路径上有界等待 Server：先确保已请求拉起，再轮询到连上或超时。
    ///
    /// Server 崩溃 / 被杀后，新进程约 1 秒内监听管道；这段时间里的键若直接兜底，会出现
    /// 「首字母漏进文档、后面的键被吞」。这里阻塞当前键最多 [`RECONNECT_WAIT`]，后续键在
    /// 线程消息队列里自然排队，连上后依次转发，整段拼音就不会断。全屏游戏与密码框在上游已放行，
    /// 不会走到这里。
    pub(super) fn connect_within(&self, timeout: Duration) -> bool {
        if self.engine.borrow().is_some() {
            return true;
        }
        // 先按正常路径尝试 + 触发拉起（受退避 / 冷却约束，最多起一个 Server）。
        if self.engine.borrow().is_none() {
            self.ensure_connected();
        }
        let deadline = Instant::now() + timeout;
        while self.engine.borrow().is_none() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(60));
            self.try_open();
        }
        let connected = self.engine.borrow().is_some();
        if connected && self.shared.foreground() {
            self.report_mode();
        }
        connected
    }

    /// 转发失败后断开，下一键重连。
    pub(super) fn disconnect(&self) {
        *self.engine.borrow_mut() = None;
        self.last_connect_failure.set(None);
        self.shared.end_composing();
    }
}
