//! 组合预测器：云端拼音纠错与端侧整句翻译同时跑、互不覆盖。
//!
//! 青简原架构里 Engine 只有一个预测器槽位，端侧翻译器一挂上就把云端纠错整个顶掉，
//! 两者只能活一个。[`DualPredictor`] 把两边收进同一个预测器，同一次组句请求同时发给两者，
//! 各自的结果分别回到 `sentence`（中文纠错，Tab 上屏）与 `translation`（外文译文，只展示）。

use qingjian_core::{Prediction, PredictionPolicy, PredictionRequest, Predictor};

use crate::cloud_predictor::CloudPredictor;
use crate::local_translator::LocalTranslator;

/// 同时挂「云端拼音→中文纠错」与「端侧中文→外文翻译」的组合预测器。
pub struct DualPredictor {
    /// 云端纠错：产出中文词与整句。
    cloud: Option<CloudPredictor>,

    /// 端侧翻译：产出外文译文。
    local: Option<LocalTranslator>,

    /// 观察窗口与开关，沿用云端那套（要整句、要云端词）。
    policy: PredictionPolicy,
}

impl DualPredictor {
    /// 至少要有一个；两个都没有时壳应直接用 [`qingjian_core::NoPredictor`]，不必构造本结构。
    pub fn new(cloud: Option<CloudPredictor>, local: Option<LocalTranslator>) -> Self {
        let policy = cloud
            .as_ref()
            .map(Predictor::policy)
            .or_else(|| local.as_ref().map(Predictor::policy))
            .unwrap_or_default();
        Self {
            cloud,
            local,
            policy,
        }
    }
}

impl Predictor for DualPredictor {
    fn policy(&self) -> PredictionPolicy {
        self.policy
    }

    fn submit(&mut self, request: PredictionRequest) {
        if let Some(cloud) = self.cloud.as_mut() {
            cloud.submit(request.clone());
        }
        if let Some(local) = self.local.as_mut() {
            local.submit(request);
        }
    }

    fn poll(&mut self) -> Option<Prediction> {
        // 云端纠错优先取（它决定 Tab 上屏）；云端没就绪再取端侧译文，译文因此能先显示。
        if let Some(cloud) = self.cloud.as_mut()
            && let Some(prediction) = cloud.poll()
        {
            return Some(prediction);
        }
        if let Some(local) = self.local.as_mut() {
            return local.poll();
        }
        None
    }
}
