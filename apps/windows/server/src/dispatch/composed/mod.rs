//! 组句的展示状态：缓冲变化时重查候选并重建 [`Composed`]，云端词异步并入，高亮 / 翻页，按状态生成给 DLL 的帧。

mod state;

use qingjian_core::{
    Candidate, CandidateLayout, CandidateList, CloudWord, Query, SurroundingText,
};
use qingjian_platform::protocol::{Frame, PROTOCOL_VERSION, PreeditKind, PreeditSegment};

pub(super) use self::state::{Composed, TypedKeys};
use super::Router;

impl Router {
    /// 缓冲变化后：按 Engine 状态重建 [`Composed`]，发一次云联想请求，归零高亮与整句补全。
    pub(super) fn recompose(&mut self) {
        self.highlight = 0;
        self.navigated = false;
        self.sentence = None;
        self.sentence_translation = None;
        self.cloud_selected = false;
        self.compose_translation = None;
        if self.engine.composition().is_empty() {
            self.composed = None;
            self.cancel_prediction();
            self.stop_rescoring();
            return;
        }
        self.attach_loaded_model();
        let built = self.engine.query().ok().map(|query| {
            let (preedit, cursor, typed_keys) = marked_parts(&query);
            (query.candidates.items.clone(), preedit, cursor, typed_keys)
        });
        self.composed = Some(match built {
            Some((items, preedit, cursor, typed_keys)) => {
                let layout =
                    CandidateLayout::new(items, self.config.page_size, self.config.cloud_slots);
                if self.engine.prediction_enabled() {
                    self.engine
                        .request_prediction(self.surrounding_for_prediction(), layout.local());
                }
                Composed::Candidates {
                    preedit,
                    cursor,
                    typed_keys,
                    layout,
                }
            }
            None => {
                self.cancel_prediction();
                let composition = self.engine.composition();
                let text = composition.text().to_owned();
                let cursor = text[..composition.cursor()].chars().count();
                Composed::Raw { text, cursor }
            }
        });
        self.schedule_rescoring();
    }

    /// DLL 送来新的光标前文后重发一次云联想：作废在飞的空上下文请求，让云端拿得到 before。
    /// 只在候选态有意义；云没启用时 [`Engine::request_prediction`] 内部自己早退。
    pub(super) fn refresh_prediction(&mut self) {
        let Some(Composed::Candidates { layout, .. }) = &self.composed else {
            return;
        };
        let local = layout.local().to_vec();
        let surrounding = self.surrounding_for_prediction();
        self.engine.request_prediction(surrounding, &local);
    }

    /// DLL 报来的光标前文包成云联想要的形状；没有就 `None`（Engine 只靠拼音猜）。
    pub(super) fn surrounding_for_prediction(&self) -> Option<SurroundingText> {
        (!self.surrounding_before.is_empty()).then(|| SurroundingText {
            before: self.surrounding_before.clone(),
            after: String::new(),
        })
    }

    /// 拉一次云联想结果：云端词并进候选布局，整句补全记下；翻译评审时结果是译文。
    pub(super) fn poll_prediction(&mut self) {
        if !self.engine.prediction_enabled() {
            return;
        }
        let Some(prediction) = self.engine.poll_prediction() else {
            return;
        };
        if self.translation.is_some() {
            match prediction.sentence {
                Some(text) => {
                    if let Some(translation) = self.translation.as_mut() {
                        translation.result = Some(text);
                    }
                }
                None => {
                    tracing::info!("翻译选中文字：云端没有给出译文");
                    self.translation = None;
                }
            }
            return;
        }
        if let Some(Composed::Candidates { layout, .. }) = self.composed.as_mut() {
            let words: Vec<Candidate> = prediction
                .words
                .into_iter()
                .map(CloudWord::into_candidate)
                .collect();
            // DualPredictor 把云端纠错（sentence）和端侧译文（translation）分两条回、字段互补：
            // 空词不覆盖已并入的云端词，sentence / translation 只并入本次新到的 Some，
            // 避免后到的一条（云端约 1 秒）带着空字段冲掉先到的另一条（端侧约 50 毫秒）。
            if !words.is_empty() {
                layout.set_cloud(words);
            }
            if prediction.sentence.is_some() {
                // 云端整句：中文和外文同一次返回，绑定在一起。未选中时它只在云行待选，外文不顶上来；
                // 等用户按 Tab 选中（见 input.rs），顶部外文才切到这句。
                self.sentence = prediction.sentence;
                self.sentence_translation = prediction.translation;
            } else if prediction.translation.is_some() {
                // 端侧整句译文只对应本地组出的中文，跟随本地首位候选。
                self.compose_translation = prediction.translation;
            }
        }
    }

    pub(super) fn cancel_prediction(&mut self) {
        if self.engine.prediction_enabled() {
            self.engine.cancel_prediction();
        }
    }

    /// Tab 把云整句选为首位：有云整句且尚未选中时置为选中态。返回 true 表示这次 Tab 被云整句吃掉
    /// （不翻页）；已选中时再按 Tab 保持原位（同样吃掉、不翻走）；没有云整句返回 false，调用方按翻页处理。
    pub(super) fn select_cloud_sentence(&mut self) -> bool {
        if self.sentence.is_some() {
            self.cloud_selected = true;
            true
        } else {
            false
        }
    }

    /// 云整句是否在首位。
    pub(super) fn cloud_selected(&self) -> bool {
        self.cloud_selected
    }

    /// 高亮移动 `delta`，夹在 `[0, 末尾]`，到页边自然换页。
    pub(super) fn move_highlight(&mut self, delta: isize) {
        // 一旦手动浏览本地候选，云整句退出首位。
        self.cloud_selected = false;
        let count = self.candidate_count();
        if count == 0 {
            self.highlight = 0;
            return;
        }
        let next = (self.highlight as isize + delta).clamp(0, count as isize - 1) as usize;
        self.navigated |= next != self.highlight;
        self.highlight = next;
    }

    /// 整页翻 `step`，高亮落到目标页第一个候选。
    pub(super) fn page(&mut self, step: isize) {
        // 加减号翻页 = 回去浏览本地候选，云整句退出首位。
        self.cloud_selected = false;
        let count = self.candidate_count();
        if count == 0 {
            self.highlight = 0;
            return;
        }
        let page_size = self.config.page_size;
        let page_count = count.div_ceil(page_size);
        let current = (self.highlight / page_size) as isize;
        let target = (current + step).clamp(0, page_count as isize - 1) as usize;
        if target != current as usize {
            self.navigated = true;
            self.engine.note_page_turn();
        }
        self.highlight = (target * page_size).min(count - 1);
    }

    pub(super) fn candidate_count(&self) -> usize {
        match &self.composed {
            Some(Composed::Candidates { layout, .. }) => layout.len(),
            _ => 0,
        }
    }

    /// 候选布局里第 `index` 个（跨页下标）。
    pub(super) fn layout_candidate(&self, index: usize) -> Option<Candidate> {
        match &self.composed {
            Some(Composed::Candidates { layout, .. }) => layout.candidate(index).cloned(),
            _ => None,
        }
    }

    pub(super) fn commit_index(&mut self, index: usize) -> Option<String> {
        // 数字选本地词：云整句退出首位。
        self.cloud_selected = false;
        let candidate = self.layout_candidate(index)?;
        Some(self.engine.commit(&candidate))
    }

    /// 按当前状态生成一帧：翻译评审优先；没在组句给空帧；否则给高亮所在的那一页。
    /// 焦点会话的 DLL 比 Server 老时按老协议降级（见 [`Self::downgrade_for_old_dll`]）。
    pub(super) fn current_frame(&self) -> Frame {
        let mut frame = self.raw_frame();
        self.show_typed_keys(&mut frame);
        self.downgrade_for_old_dll(&mut frame);
        frame
    }

    /// 给 DLL 的帧只管应用输入框：双拼「输入框显示原始按键」开着时换成敲的键，拼音行由 Server 自绘照旧全拼。
    fn show_typed_keys(&self, frame: &mut Frame) {
        if self.translation.is_some() {
            return;
        }
        let Some(Composed::Candidates {
            typed_keys: Some(keys),
            ..
        }) = &self.composed
        else {
            return;
        };
        frame.preedit = vec![PreeditSegment {
            text: keys.text.clone(),
            kind: PreeditKind::Typed,
        }];
        frame.cursor = keys.cursor;
    }

    /// 自绘候选窗用的帧：不做老 DLL 降级，码段照常画。
    pub(super) fn self_drawn_frame(&self) -> Frame {
        self.raw_frame()
    }

    /// 协议比 Server 老的 DLL 不认识 `AuxCode` 段，收到会整条消息解析失败；给它的码段降级成普通拼音段。
    fn downgrade_for_old_dll(&self, frame: &mut Frame) {
        if self.focused_dll_protocol() >= PROTOCOL_VERSION {
            return;
        }
        for segment in &mut frame.preedit {
            if segment.kind == PreeditKind::AuxCode {
                segment.kind = PreeditKind::Typed;
            }
        }
    }

    /// 焦点会话的 DLL 协议版本；没有焦点会话时按最新（不必降级）。
    fn focused_dll_protocol(&self) -> u32 {
        self.focused
            .and_then(|session| self.sessions.get(&session))
            .map_or(PROTOCOL_VERSION, |info| info.protocol)
    }

    fn raw_frame(&self) -> Frame {
        if let Some(translation) = &self.translation {
            return self.translation_frame(translation);
        }
        match &self.composed {
            None => Frame::default(),
            Some(Composed::Raw { text, cursor }) => Frame {
                preedit: vec![PreeditSegment {
                    text: text.clone(),
                    kind: PreeditKind::Typed,
                }],
                preedit_mode: self.config.preedit,
                cursor: *cursor,
                candidates: CandidateList { items: Vec::new() },
                highlight: usize::MAX,
                page: 0,
                page_count: 1,
                layout: self.config.layout,
                theme: self.config.theme,
                aux_code_show: self.config.aux_code_show,
                sentence: None,
                sentence_selected: false,
                translation: None,
                notice: self.notice.clone(),
            },
            Some(Composed::Candidates {
                preedit,
                cursor,
                layout,
                ..
            }) => {
                let page_size = self.config.page_size;
                // 云整句被 Tab 选中后它就是首位，候选区回到第 0 页、且本地候选行不高亮。
                let highlight = if self.cloud_selected {
                    0
                } else {
                    self.highlight.min(layout.len().saturating_sub(1))
                };
                let page = highlight / page_size;
                let items: Vec<Candidate> = layout
                    .page(page)
                    .into_iter()
                    .filter_map(|cell| cell.candidate().cloned())
                    .collect();
                let mut candidates = CandidateList { items };
                self.engine.annotate(&mut candidates);
                let page_highlight = highlight - page * page_size;
                let frame_highlight = if self.cloud_selected {
                    usize::MAX
                } else {
                    page_highlight
                };
                let translation = self.top_translation(&candidates.items, page_highlight);
                Frame {
                    preedit: preedit.clone(),
                    preedit_mode: self.config.preedit,
                    cursor: *cursor,
                    candidates,
                    highlight: frame_highlight,
                    page,
                    page_count: layout.pages().max(1),
                    layout: self.config.layout,
                    theme: self.config.theme,
                    aux_code_show: self.config.aux_code_show,
                    sentence: self.sentence.clone(),
                    sentence_selected: self.cloud_selected,
                    translation,
                    notice: self.notice.clone(),
                }
            }
        }
    }

    /// 顶部外文跟随当前首位候选：选中云整句时用云译文（没给就退回本地整句译文）；
    /// 未选中时，高亮落在整句候选上用本地整句译文，落在单词候选上用该词的释义。
    fn top_translation(&self, page_items: &[Candidate], page_highlight: usize) -> Option<String> {
        if self.cloud_selected {
            return self
                .sentence_translation
                .clone()
                .or_else(|| self.compose_translation.clone());
        }
        let top = page_items.get(page_highlight)?;
        if top.kind == qingjian_core::CandidateKind::Sentence
            || top.kind == qingjian_core::CandidateKind::Generated
        {
            return self.compose_translation.clone();
        }
        let translation = top.translation.as_ref()?;
        let senses: Vec<String> = translation.senses().iter().map(|s| s.text.clone()).collect();
        (!senses.is_empty()).then(|| senses.join("；"))
    }
}

/// 一次查询的拼音行分段与光标，以及输入框另显示原始按键时的那一串。
/// 光标用 Core 的映射：自动补的 `'` 会让显示串比敲的长。
pub(super) fn marked_parts(query: &Query) -> (Vec<PreeditSegment>, usize, Option<TypedKeys>) {
    let preedit = query.marked_segments().iter().map(Into::into).collect();
    let typed_keys = query.shuangpin_raw_preedit.then(|| TypedKeys {
        text: query.marked_text(),
        cursor: query.marked_cursor(),
    });
    (preedit, query.segments_cursor(), typed_keys)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dispatch::RouterConfig;
    use qingjian_core::Engine;
    use qingjian_dictionary::Dictionary;
    use qingjian_platform::protocol::{
        ClientMessage, KeyEvent, KeyModifiers, PROTOCOL_VERSION, SessionId,
    };

    fn router() -> Router {
        let mut router = Router::new(
            Engine::new(Dictionary::parse("你\tni\t100\n").unwrap()),
            RouterConfig::default(),
        );
        router.handle(ClientMessage::OpenSession {
            session: SessionId(1),
            app: None,
            protocol: PROTOCOL_VERSION,
        });
        router
    }

    fn compose(router: &mut Router, text: &str) {
        for c in text.chars() {
            router
                .handle(ClientMessage::Key {
                    session: SessionId(1),
                    event: KeyEvent::new(c as u32, Some(c), KeyModifiers::default()),
                })
                .unwrap();
        }
    }

    fn surrounding(router: &mut Router, session: SessionId, text: &str) {
        // 光标前文消息不回 ServerMessage，正常返回 None。
        let _ = router.handle(ClientMessage::Surrounding {
            session,
            text: text.to_owned(),
        });
    }

    /// 光标前文要成为云联想的 `before`；空文本不算上下文；组句结束即作废。
    #[test]
    fn surrounding_context_flows_to_prediction_and_clears_on_reset() {
        let mut router = router();
        compose(&mut router, "ni");
        assert!(router.surrounding_for_prediction().is_none());

        surrounding(&mut router, SessionId(1), "这个文件权限有问题，");
        let got = router.surrounding_for_prediction().expect("应带上前文");
        assert_eq!(got.before, "这个文件权限有问题，");
        assert_eq!(got.after, "");

        // 空文本不算上下文（只靠拼音猜）
        surrounding(&mut router, SessionId(1), "");
        assert!(router.surrounding_for_prediction().is_none());

        // 组句结束：前文作废，等下一段组句 DLL 再送
        surrounding(&mut router, SessionId(1), "别的内容");
        router.stop_rescoring();
        assert!(router.surrounding_for_prediction().is_none());
    }

    /// 非聚焦会话的前文不收，防止换应用后把 A 应用的文字带进 B 的联想。
    #[test]
    fn surrounding_from_unfocused_session_is_dropped() {
        let mut router = router();
        compose(&mut router, "ni");
        surrounding(&mut router, SessionId(99), "别的应用的前文");
        assert!(router.surrounding_for_prediction().is_none());
    }
}
