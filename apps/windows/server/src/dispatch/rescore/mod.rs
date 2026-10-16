//! 光标前文与重排调度（本地神经整句模型已在灰迹中移除）：
//!
//! 灰迹不挂本地整句 scorer，长句纠错与翻译交给云端；这里保留调度框架，在没有 scorer 时全部空转，
//! 仍在工作的是 [`Router::set_surrounding`]：把 DLL 送来的光标前文交给云联想（`before`）。
//! 节拍由工人循环驱动：[`Router::next_tick`] 说下次多久来一次 [`Router::tick`]；DLL 组句期间每 80 ms 的 `Poll` 也顺带 tick。
//! 进行态在 [`state`]。

mod state;

use std::time::Duration;

use qingjian_core::CandidateLayout;
use qingjian_platform::protocol::SessionId;

pub(crate) use self::state::RescoreState;
use super::Router;
use super::composed::{Composed, marked_parts};

impl Router {
    /// 组句结束：什么都不等了；应用前文也作废（下一段组句 DLL 会再送）。
    pub(super) fn stop_rescoring(&mut self) {
        self.rescore.stop();
        self.surrounding_before.clear();
        self.engine.set_rescoring_context(None);
    }

    /// DLL 送来聚焦会话的光标前文：给 Engine 当前文，缓存里按旧前文记的「要打分的」作废，重新攒一次并重新计时。
    /// 组句已经结束 / 不是聚焦会话的丢掉。
    pub(super) fn set_surrounding(&mut self, session: SessionId, text: String) {
        if self.focused != Some(session) || self.engine.composition().is_empty() {
            return;
        }
        // 云联想也按这份前文带 before：先记下，再交给本地整句重排。
        self.surrounding_before = text.clone();
        self.engine
            .set_rescoring_context((!text.is_empty()).then_some(text));
        if matches!(self.composed, Some(Composed::Candidates { .. })) {
            // 查一次只为按新前文重新记下要打分的文本，候选顺序此刻不变
            let _ = self.engine.query();
            self.schedule_rescoring();
            // 云联想也在等这份前文：带新上下文重发，作废在飞的空上下文请求（worker 有防抖）
            self.refresh_prediction();
        }
    }

    /// 缓冲变化之后：有整句路径等着打分就起防抖计时，否则停下。
    pub(super) fn schedule_rescoring(&mut self) {
        if self.engine.rescoring_pending() {
            self.rescore.schedule();
        } else {
            self.rescore.stop();
        }
    }

    /// 工人循环下次该多久后来一次 [`Self::tick`]：在等重排就按它的节拍，否则按配置热加载的一秒。
    pub fn next_tick(&self) -> Duration {
        self.rescore
            .next_deadline()
            .map_or(super::reload::CONFIG_POLL_INTERVAL, |deadline| {
                deadline.min(super::reload::CONFIG_POLL_INTERVAL)
            })
    }

    /// 到点了：推进重排、看一眼配置文件、探一次 UI 线程活性。
    /// 工人循环超时与 DLL 的 `Poll` 都会调。
    pub fn tick(&mut self) {
        self.advance_rescoring();
        self.poll_config_reload();
        self.candidates.watchdog();
    }

    /// 防抖到点就发请求；在等结果就收一次，收到了重查并重画当前页。
    fn advance_rescoring(&mut self) {
        if self.engine.composition().is_empty() || self.translation.is_some() {
            self.rescore.stop();
            return;
        }
        if self.rescore.debounce_elapsed() {
            if self.engine.request_rescoring() {
                self.rescore.start_polling();
            } else {
                self.rescore.stop();
            }
        }
        if !self.rescore.polling() {
            return;
        }
        if !self.engine.poll_rescoring() {
            // 等太久多半是前文变了（上屏后接着打下一段）、结果作废；真卡住也只是这轮不重排
            if self.rescore.expired() {
                tracing::debug!("等本地整句模型超时，本轮不重排");
                self.rescore.stop();
            }
            return;
        }
        self.rescore.stop();
        if self.highlight >= self.config.page_size || self.navigated {
            return;
        }
        self.requery_rescored();
    }

    /// 分回来了：按重排后的顺序重建候选布局，云端词与整句补全留着，重画当前页。
    fn requery_rescored(&mut self) {
        let Ok(query) = self.engine.query() else {
            return;
        };
        let Some(Composed::Candidates {
            preedit,
            cursor,
            typed_keys,
            layout,
        }) = self.composed.as_mut()
        else {
            return;
        };
        let cloud = layout.cloud().to_vec();
        let mut rebuilt = CandidateLayout::new(
            query.candidates.items.clone(),
            self.config.page_size,
            self.config.cloud_slots,
        );
        if !cloud.is_empty() {
            rebuilt.set_cloud(cloud);
        }
        *layout = rebuilt;
        (*preedit, *cursor, *typed_keys) = marked_parts(&query);
        let frame = self.self_drawn_frame();
        self.reconcile_candidates(&frame);
        // 前文在结果回来之前换过（Surrounding 晚到）：这次查询又记下了一批要打分的，再来一轮
        self.schedule_rescoring();
    }
}
