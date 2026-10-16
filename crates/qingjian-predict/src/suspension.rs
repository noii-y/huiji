//! 账户层面失败（密钥无效、欠费、被拒）后的暂停。
//!
//! 这类失败在充值或换 key 之前每次请求都必然失败，照发只会刷日志、白跑网络，所以停一段时间再试。
//! 暂停只在云后台线程内生效：缓存照常命中、本地候选和端侧译文不受影响，只是不再发新的网络请求。
//! 改配置时壳会重建 Worker，暂停随之清零。

use std::time::{Duration, Instant};

use crate::error::PredictError;

/// 账户失败后暂停多久再试一次。
pub const ACCOUNT_PAUSE: Duration = Duration::from_secs(10 * 60);

/// 云后台线程的暂停状态。
#[derive(Debug, Default)]
pub struct Suspension {
    /// 暂停到什么时候；`None` 表示没在暂停。
    until: Option<Instant>,
}

impl Suspension {
    /// 现在是否还在暂停中；暂停到期时记一条日志并恢复。
    pub fn active(&mut self, now: Instant) -> bool {
        match self.until {
            Some(until) if now < until => true,
            Some(_) => {
                self.until = None;
                tracing::info!("云端请求暂停到期，重新尝试");
                false
            }
            None => false,
        }
    }

    /// 按这次的错误决定要不要暂停；真的暂停了返回 `true`（调用方不必再为这次失败记一条普通日志）。
    pub fn record(&mut self, error: &PredictError, now: Instant, what: &str) -> bool {
        if !error.is_account_failure() {
            return false;
        }
        self.until = Some(now + ACCOUNT_PAUSE);
        tracing::warn!(
            %error,
            pause_minutes = ACCOUNT_PAUSE.as_secs() / 60,
            "{what}失败：账户不可用（密钥无效、欠费或被拒），暂停云端请求，改配置或到期后再试"
        );
        true
    }
}

#[cfg(test)]
mod tests {
    use async_openai::error::{ApiError, ApiErrorResponse, OpenAIError};

    use super::*;

    fn api_error(status: u16) -> PredictError {
        PredictError::Api(OpenAIError::ApiError(ApiErrorResponse {
            status_code: reqwest::StatusCode::from_u16(status).unwrap(),
            api_error: ApiError {
                message: "Insufficient Balance".to_owned(),
                r#type: None,
                param: None,
                code: None,
                misalignment: None,
            },
        }))
    }

    #[test]
    fn account_failures_pause_until_the_deadline() {
        let now = Instant::now();
        let mut suspension = Suspension::default();
        assert!(!suspension.active(now));
        assert!(suspension.record(&api_error(402), now, "联想"));
        assert!(suspension.active(now + Duration::from_secs(60)));
        assert!(!suspension.active(now + ACCOUNT_PAUSE));
        // 到期后再查保持恢复，不会反复刷到期日志
        assert!(!suspension.active(now + ACCOUNT_PAUSE));
    }

    #[test]
    fn transient_failures_do_not_pause() {
        let now = Instant::now();
        let mut suspension = Suspension::default();
        // 限流、参数错误、服务端错误、超时都不暂停
        for status in [400, 429, 500, 503] {
            assert!(!suspension.record(&api_error(status), now, "联想"));
        }
        assert!(!suspension.record(&PredictError::Timeout(5000), now, "联想"));
        assert!(!suspension.active(now));
        // 401 / 402 / 403 才暂停
        for status in [401, 402, 403] {
            assert!(suspension.record(&api_error(status), now, "联想"));
        }
    }
}
