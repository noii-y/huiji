use thiserror::Error;

#[derive(Debug, Error)]
pub enum PredictError {
    #[error("no API key: set `api_key` in config or the `{0}` environment variable")]
    MissingApiKey(String),

    #[error("failed to start async runtime: {0}")]
    Runtime(#[from] std::io::Error),

    #[error("API request failed: {0}")]
    Api(#[from] async_openai::error::OpenAIError),

    #[error("API request timed out after {0} ms")]
    Timeout(u64),

    #[error("API returned no usable content")]
    EmptyReply,

    /// 正文为空且回复因长度截断：模型开着思考，输出额度花在思考上了。
    #[error("API reply was cut off before any content (output budget spent on reasoning)")]
    BudgetExhausted,

    #[error("failed to encode request: {0}")]
    Encode(#[from] serde_json::Error),
}

impl PredictError {
    /// 账户层面的失败：密钥无效、欠费、被拒（401 / 402 / 403）。
    /// 充值或换 key 之前每次请求都必然失败，照发只会刷日志、白跑网络，应该暂停一段时间。
    /// 限流（429）、参数错误（400）、服务端错误（5xx）和超时是临时的，不在此列，照常按失败处理。
    pub fn is_account_failure(&self) -> bool {
        let Self::Api(error) = self else {
            return false;
        };
        let async_openai::error::OpenAIError::ApiError(response) = error else {
            return false;
        };
        matches!(response.status_code.as_u16(), 401..=403)
    }
}
