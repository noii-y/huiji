//! Tab 与分页的三端约定；直接注入整句补全状态，不接云服务。
use crate::dispatch::{Router, RouterConfig};
use qingjian_core::{CustomPhrase, Engine};
use qingjian_dictionary::{Dictionary, WordList};
use qingjian_platform::protocol::{
    ClientMessage, Frame, KeyEvent, KeyModifiers, KeyOutcome, PROTOCOL_VERSION, ServerMessage,
    SessionId,
};

fn router(size: usize) -> Router {
    let mut engine = Engine::new(Dictionary::parse("你\tni\t100\n").unwrap()).with_english(
        WordList::parse("hello\thello\t100\nhelp\thelp\t90\nheld\theld\t80\n").unwrap(),
    );
    engine
        .set_custom_phrases(
            (1..=9)
                .map(|position| CustomPhrase {
                    code: "qq".into(),
                    text: format!("第{position}项"),
                    position,
                    enabled: true,
                })
                .collect(),
        )
        .unwrap();
    let mut router = Router::new(
        engine,
        RouterConfig {
            page_size: size,
            ..Default::default()
        },
    );
    router.handle(ClientMessage::OpenSession {
        session: SessionId(1),
        app: None,
        protocol: PROTOCOL_VERSION,
    });
    router
}
fn key(
    router: &mut Router,
    code: u32,
    character: Option<char>,
    modifiers: KeyModifiers,
) -> (KeyOutcome, Option<String>, Frame) {
    match router
        .handle(ClientMessage::Key {
            session: SessionId(1),
            event: KeyEvent::new(code, character, modifiers),
        })
        .unwrap()
    {
        ServerMessage::KeyResult {
            outcome,
            commit,
            frame,
            ..
        } => (outcome, commit, frame),
        _ => panic!("key result"),
    }
}
fn compose(router: &mut Router, text: &str, modifiers: KeyModifiers) {
    for c in text.chars() {
        key(router, c as u32, Some(c), modifiers);
    }
}
#[test]
fn tab_and_backtab_page_boundaries_and_current_page_selection() {
    for size in [1, 4, 5, 9] {
        let mut router = router(size);
        let normal = KeyModifiers::default();
        let shift = KeyModifiers {
            shift: true,
            ..normal
        };
        assert_eq!(key(&mut router, 9, None, normal).0, KeyOutcome::Passthrough);
        assert_eq!(key(&mut router, 9, None, shift).0, KeyOutcome::Passthrough);
        compose(&mut router, "qq", normal);
        assert_eq!(key(&mut router, 9, None, shift).2.page, 0);
        let last = 9_usize.div_ceil(size) - 1;
        for page in 1..=last {
            let result = key(&mut router, 9, None, normal);
            assert_eq!(result.0, KeyOutcome::Consumed);
            assert_eq!((result.2.page, result.2.highlight), (page, 0));
        }
        let frame = key(&mut router, 9, None, normal).2;
        assert_eq!(frame.page, last);
        let expected = frame.candidates.items[0].text.clone();
        assert_eq!(
            key(&mut router, b'1' as u32, Some('1'), normal).1,
            Some(expected)
        );
        compose(&mut router, "qq", normal);
        key(&mut router, 0x22, None, normal);
        assert_eq!(key(&mut router, 9, None, shift).2.page, 0);
        assert_eq!(key(&mut router, 0x21, None, normal).2.page, 0);
    }
}
#[test]
fn shift_tab_precedes_prediction_and_english_commit() {
    let mut router = router(1);
    let normal = KeyModifiers::default();
    compose(&mut router, "qq", normal);
    router.sentence = Some("可控补全".into());
    let result = key(
        &mut router,
        9,
        None,
        KeyModifiers {
            shift: true,
            ..normal
        },
    );
    assert_eq!(result.1, None);
    assert_eq!(router.sentence.as_deref(), Some("可控补全"));
    // 普通 Tab 第一次只把云整句选为首位、不上屏；空格才上屏。
    let selected = key(&mut router, 9, None, normal);
    assert_eq!(selected.1, None);
    assert!(selected.2.sentence_selected);
    assert_eq!(
        key(&mut router, 0x20, Some(' '), normal).1.as_deref(),
        Some("可控补全")
    );
    let english = KeyModifiers {
        english_mode: true,
        ..normal
    };
    compose(&mut router, "hel", english);
    key(&mut router, 0x22, None, english);
    let previous = key(
        &mut router,
        9,
        None,
        KeyModifiers {
            shift: true,
            ..english
        },
    );
    assert_eq!(previous.1, None);
    assert_eq!(previous.2.page, 0);
    let expected = previous.2.candidates.items[previous.2.highlight]
        .text
        .clone();
    assert_eq!(key(&mut router, 9, None, english).1, Some(expected));
}
#[test]
fn tab_with_raw_input_and_no_candidates_is_consumed_without_commit() {
    let mut router = router(5);
    compose(&mut router, "zzzz", KeyModifiers::default());
    let result = key(&mut router, 9, None, KeyModifiers::default());
    assert_eq!(result.0, KeyOutcome::Consumed);
    assert_eq!(result.1, None);
    assert_eq!(result.2.page, 0);
}

#[test]
fn tab_selects_cloud_sentence_keeps_selection_and_space_commits() {
    let mut router = router(5);
    let normal = KeyModifiers::default();
    compose(&mut router, "qq", normal);
    router.sentence = Some("可控补全".into());
    router.sentence_translation = Some("controllable completion".into());

    // 第一次 Tab：云整句选为首位，不上屏，顶部外文同步成云译文。
    let first = key(&mut router, 9, None, normal);
    assert_eq!(first.1, None);
    assert!(first.2.sentence_selected);
    assert_eq!(first.2.translation.as_deref(), Some("controllable completion"));
    // 选中后本地候选行不高亮。
    assert_eq!(first.2.highlight, usize::MAX);

    // 再按 Tab：保持原位、不翻页、不上屏。
    let second = key(&mut router, 9, None, normal);
    assert_eq!(second.1, None);
    assert!(second.2.sentence_selected);
    assert_eq!(second.2.page, 0);

    // 空格上云整句。
    assert_eq!(
        key(&mut router, 0x20, Some(' '), normal).1.as_deref(),
        Some("可控补全")
    );
}

#[test]
fn enter_commits_cloud_sentence_when_selected_but_raw_when_not() {
    let mut router = router(5);
    let normal = KeyModifiers::default();
    compose(&mut router, "qq", normal);
    router.sentence = Some("可控补全".into());

    // 选中后回车上云整句。
    key(&mut router, 9, None, normal);
    assert_eq!(
        key(&mut router, 0x0D, None, normal).1.as_deref(),
        Some("可控补全")
    );

    // 未选中时回车仍是原始字母（微软拼音习惯）。
    compose(&mut router, "qq", normal);
    router.sentence = Some("可控补全".into());
    assert_eq!(key(&mut router, 0x0D, None, normal).1.as_deref(), Some("qq"));
}

#[test]
fn esc_cancels_cloud_selection_but_keeps_pinyin() {
    let mut router = router(5);
    let normal = KeyModifiers::default();
    compose(&mut router, "qq", normal);
    router.sentence = Some("可控补全".into());
    key(&mut router, 9, None, normal);

    // Esc 只取消选中：云整句回到待选状态，拼音 / 候选还在。
    let frame = key(&mut router, 0x1B, None, normal).2;
    assert!(!frame.sentence_selected);
    assert_eq!(frame.sentence.as_deref(), Some("可控补全"));
    assert!(!frame.candidates.items.is_empty());
}

#[test]
fn browsing_candidates_cancels_cloud_selection() {
    let mut router = router(5);
    let normal = KeyModifiers::default();
    compose(&mut router, "qq", normal);
    router.sentence = Some("可控补全".into());
    key(&mut router, 9, None, normal);

    // 方向下移动高亮：云整句退出首位，回到本地候选浏览。
    let frame = key(&mut router, 0x28, None, normal).2;
    assert!(!frame.sentence_selected);
    assert_eq!(frame.sentence.as_deref(), Some("可控补全"));
}

#[test]
fn tab_still_pages_without_cloud_sentence() {
    let mut router = router(1);
    let normal = KeyModifiers::default();
    compose(&mut router, "qq", normal);
    assert!(router.sentence.is_none());
    let result = key(&mut router, 9, None, normal);
    assert_eq!(result.0, KeyOutcome::Consumed);
    assert_eq!(result.2.page, 1);
    assert!(!result.2.sentence_selected);
}
