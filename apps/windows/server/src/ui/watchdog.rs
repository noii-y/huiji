//! UI 线程看门狗：候选窗在独立线程上自绘，这条线程一旦在渲染时卡死或 panic 退出，
//! Router 的管道请求仍会被正常应答（Router 线程活着），TSF 的 1 秒超时检测不到，
//! 表现为「能打字、候选窗不出来」。这里用请求-应答式心跳探测 UI 线程是否还活着：
//! Router 定期发心跳命令，UI 线程收到后更新时间戳；超时没应答就判定它死了，
//! 结束整个 Server 进程，由 TSF 下一键重拉干净的新进程。窗口有线程亲和，同进程里
//! 跨线程销毁不了卡死线程拥有的窗口，整体重启才干净。

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// 多久向 UI 线程要一次心跳。
pub(super) const PING_INTERVAL: Duration = Duration::from_secs(2);

/// 距上次心跳超过它就判定 UI 线程死亡。一帧渲染正常在百毫秒内，6 秒足够宽松，不会误判。
pub(super) const DEAD_THRESHOLD: Duration = Duration::from_secs(6);

/// Server 被看门狗结束时的退出码（非 0，便于日后排查日志）。
pub(super) const WATCHDOG_EXIT_CODE: i32 = 77;

/// 当前时间的 Unix 毫秒；系统时钟异常（早于 1970）时返回 0。
pub(super) fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// UI 线程心跳：最后一次应答的时间戳（Unix 毫秒），Router 与 UI 线程共享。
pub(super) struct Heartbeat {
    last: AtomicU64,
}

impl Heartbeat {
    pub(super) fn new() -> Self {
        Self {
            last: AtomicU64::new(now_millis()),
        }
    }

    /// UI 线程应答心跳：记下当前时间。
    pub(super) fn beat(&self) {
        self.last.store(now_millis(), Ordering::Relaxed);
    }

    /// 距上次应答多久；时钟异常（时间戳为 0 或时间回拨）返回 `None`。
    fn age(&self) -> Option<Duration> {
        let last = self.last.load(Ordering::Relaxed);
        if last == 0 {
            return None;
        }
        let now = now_millis();
        now.checked_sub(last).map(Duration::from_millis)
    }

    /// 心跳是否已超时（UI 线程该判死）。时钟异常时不判死，宁可不动也不误杀。
    pub(super) fn is_stale(&self) -> bool {
        self.age().is_some_and(|age| age >= DEAD_THRESHOLD)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fresh_heartbeat_is_not_stale() {
        assert!(!Heartbeat::new().is_stale());
    }

    #[test]
    fn stale_heartbeat_is_detected() {
        // 把最后应答时间拨到阈值之前 1 秒
        let old =
            now_millis() - PING_INTERVAL.as_millis() as u64 - DEAD_THRESHOLD.as_millis() as u64;
        let heartbeat = Heartbeat {
            last: AtomicU64::new(old),
        };
        assert!(heartbeat.is_stale());
    }

    #[test]
    fn zero_timestamp_is_never_stale() {
        let heartbeat = Heartbeat {
            last: AtomicU64::new(0),
        };
        assert!(!heartbeat.is_stale());
    }
}
