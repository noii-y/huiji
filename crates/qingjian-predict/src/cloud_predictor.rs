use std::path::Path;
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::Duration;

use qingjian_core::{Prediction, PredictionPolicy, PredictionRequest, Predictor};

use crate::chat_client::ChatClient;
use crate::config::PredictConfig;
use crate::disk_cache::{DEFAULT_DISK_CAPACITY, DiskCache};
use crate::error::PredictError;
use crate::worker::Worker;

/// 云结果磁盘缓存文件名，放在用户数据目录（`%APPDATA%\Qingjian`）下。
pub const CLOUD_CACHE_FILE: &str = "cloud-cache.json";

/// 走网络的联想器。构造时起一个后台线程，`submit` / `poll` 都只碰通道，不阻塞。
pub struct CloudPredictor {
    /// 观察窗口、条数上限与开关，来自配置。
    policy: PredictionPolicy,

    /// 往后台线程发请求。
    requests: Sender<PredictionRequest>,

    /// 从后台线程收结果。
    responses: Receiver<Prediction>,
}

impl CloudPredictor {
    /// 没有密钥直接报错，让壳退回 [`qingjian_core::NoPredictor`] 并记日志。
    /// `user_dir` 给出来时，云结果在该目录下做磁盘缓存，跨重启复用、省 token。
    pub fn new(config: &PredictConfig, user_dir: Option<&Path>) -> Result<Self, PredictError> {
        let api_key = config
            .resolve_api_key()
            .ok_or_else(|| PredictError::MissingApiKey(config.api_key_env.clone()))?;
        let client = ChatClient::new(config, api_key);
        let disk = user_dir.map(|dir| {
            DiskCache::open(
                dir.join(CLOUD_CACHE_FILE),
                config.model.clone(),
                DEFAULT_DISK_CAPACITY,
            )
        });
        let (requests, request_rx) = mpsc::channel();
        let (response_tx, responses) = mpsc::channel();
        let worker = Worker::new(
            request_rx,
            response_tx,
            client,
            Duration::from_millis(config.debounce_ms),
            disk,
        );
        std::thread::Builder::new()
            .name("qingjian-predict".to_owned())
            .spawn(move || {
                if let Err(error) = worker.run() {
                    tracing::error!(%error, "联想线程退出");
                }
            })?;
        tracing::info!(
            base_url = %config.base_url,
            model = %config.model,
            lookback = config.lookback,
            lookahead = config.lookahead,
            "云联想已启用"
        );
        Ok(Self {
            policy: config.policy(),
            requests,
            responses,
        })
    }
}

impl Predictor for CloudPredictor {
    fn policy(&self) -> PredictionPolicy {
        self.policy
    }

    fn submit(&mut self, request: PredictionRequest) {
        if self.requests.send(request).is_err() {
            tracing::warn!("联想线程已退出，请求被丢弃");
        }
    }

    fn poll(&mut self) -> Option<Prediction> {
        // Empty 与 Disconnected 都当没有：线程退出时已经记过日志
        self.responses.try_recv().ok()
    }
}
