//! 冷启动实验错误：失败必须中断，不能静默跳过难例。

#[derive(Debug, thiserror::Error)]
pub enum ColdError {
    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Json(#[from] serde_json::Error),

    #[error("invalid cold evaluation row: {0}")]
    Invalid(String),
}
