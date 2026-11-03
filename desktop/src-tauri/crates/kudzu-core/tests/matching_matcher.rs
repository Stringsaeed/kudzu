//! Key buffer, case propagation, undo tracker and the matcher facade
//! (ported from KeyBufferTests, CasePropagationTests, UndoTrackerTests, ExpansionMatcherTests).

mod matching_support;

use kudzu_core::matching::{
    case_propagation, CaseStyle, ExpansionMatcher, KeyBuffer, KeyInput, MatcherEvent,
    MatchingConfig, TriggerIndex, TriggerMatch, UndoAction, UndoTracker,
};
use kudzu_core::{FireMode, InsertionMethod};
use matching_support::*;
use unicode_segmentation::UnicodeSegmentation;
use uuid::Uuid;

fn ch(s: &str) -> KeyInput {
    KeyInput::Character(s.into())
}

// KeyBuffer

#[test]
fn buffer_appends_backspaces_and_resets() {
    let mut buffer = KeyBuffer::new();
    buffer.apply(&ch("a"));
    buffer.apply(&ch("b"));
    assert_eq!(buffer.string(), "ab");
    assert_eq!(buffer.len(), 2);
    buffer.apply(&KeyInput::Backspace);
    assert_eq!(buffer.string(), "a");
    buffer.apply(&KeyInput::Reset);
    assert!(buffer.is_empty());
    buffer.apply(&KeyInput::Backspace);
    assert!(buffer.is_empty());
    buffer.apply(&ch(""));
    assert!(buffer.is_empty());
}

#[test]
fn buffer_caps_at_capacity_keeping_newest() {
    let mut buffer = KeyBuffer::with_capacity(4);
    for c in ["a", "b", "c", "d", "e", "f"] {
        buffer.apply(&ch(c));
    }
    assert_eq!(buffer.string(), "cdef");
    let mut buffer = KeyBuffer::new();
    for _ in 0..200 {
        buffer.apply(&ch("x"));
    }
    assert_eq!(buffer.len(), 64);
    assert_eq!(KeyBuffer::DEFAULT_CAPACITY, 64);
}

#[test]
fn buffer_holds_grapheme_clusters() {
    let mut buffer = KeyBuffer::new();
    buffer.apply(&ch("👍🏽"));
    buffer.apply(&ch("e\u{301}"));
    assert_eq!(buffer.len(), 2);
    buffer.apply(&KeyInput::Backspace);
    assert_eq!(buffer.len(), 1);

    let mut buffer = KeyBuffer::new();
    buffer.apply(&ch("e"));
    buffer.apply(&ch("\u{301}"));
    assert_eq!(buffer.len(), 1);
    assert_eq!(buffer.last(), Some("e\u{301}"));

    let mut buffer = KeyBuffer::new();
    buffer.apply(&ch("🇺"));
    buffer.apply(&ch("🇸"));
    assert_eq!(buffer.len(), 1);
    buffer.apply(&ch("🇬"));
    assert_eq!(buffer.len(), 2);

    let mut buffer = KeyBuffer::new();
    buffer.apply(&ch("\r"));
    buffer.apply(&ch("\n"));
    assert_eq!(buffer.len(), 1);
}

#[test]
fn buffer_copies_are_independent() {
    let mut original = KeyBuffer::new();
    original.apply(&ch("a"));
    let mut copy = original.clone();
    copy.apply(&ch("b"));
    assert_eq!(original.string(), "a");
    assert_eq!(copy.string(), "ab");
}

// Case propagation

#[test]
fn detects_style() {
    use CaseStyle::*;
    let cases = [
        (";sig", AsIs),
        (";Sig", Capitalized),
        (";SIG", Upper),
        (";sIg", AsIs),
        (";SiG", Capitalized),
        ("sig", AsIs),
        ("Sig", Capitalized),
        ("SIG", Upper),
        (";S", Capitalized),
        (";s", AsIs),
        (";SIG1", Upper),
        (";sig1", AsIs),
        (";1sig", AsIs),
        (";1Sig", Capitalized),
        (";12", AsIs),
        (";", AsIs),
        ("", AsIs),
        (";Éa", Capitalized),
        (";ÉA", Upper),
        (";éa", AsIs),
        (";ß", AsIs),
        (";Straße", Capitalized),
        (";STRASSE", Upper),
        ("你好", AsIs),
        ("AB你", Upper),
        ("A_B", Upper),
        (";E\u{301}A", Upper),
    ];
    for (typed, expected) in cases {
        assert_eq!(case_propagation::detect(typed), expected, "{typed}");
    }
}

#[test]
fn applies_style() {
    use CaseStyle::*;
    let cases = [
        (AsIs, "best regards", "best regards"),
        (Upper, "best regards", "BEST REGARDS"),
        (Capitalized, "best regards", "Best regards"),
        (Capitalized, "best Regards", "Best Regards"),
        (Capitalized, "BEST", "BEST"),
        (Capitalized, "", ""),
        (Upper, "", ""),
        (Capitalized, "  hello", "  Hello"),
        (Capitalized, "\"hello\"", "\"Hello\""),
        (Capitalized, "42 things", "42 things"),
        (Capitalized, "éclair", "Éclair"),
        (Upper, "straße", "STRASSE"),
        (Upper, "été", "ÉTÉ"),
        (Capitalized, "line one\nline two", "Line one\nline two"),
        (Upper, "a1;b", "A1;B"),
    ];
    for (style, text, expected) in cases {
        assert_eq!(
            case_propagation::apply(style, text),
            expected,
            "{style:?} {text}"
        );
    }
}

// Undo tracker

fn immediate_match(typed: &str) -> TriggerMatch {
    TriggerMatch {
        snippet_id: Uuid::new_v4(),
        trigger: ";sig".into(),
        typed_trigger: typed.into(),
        delete_count: typed.graphemes(true).count(),
        separator: None,
        insertion: InsertionMethod::Type,
        case_style: CaseStyle::AsIs,
        fire_mode: FireMode::Immediate,
    }
}

fn whole_word_match(typed: &str, separator: &str) -> TriggerMatch {
    TriggerMatch {
        trigger: "brb".into(),
        separator: Some(separator.into()),
        fire_mode: FireMode::WholeWord,
        ..immediate_match(typed)
    }
}

#[test]
fn undo_tracker_basics() {
    let mut tracker = UndoTracker::new();
    tracker.record_expansion(&immediate_match(";Sig"), 12);
    assert_eq!(
        tracker.consume(&KeyInput::Backspace),
        Some(UndoAction {
            delete_count: 12,
            retype: ";Sig".into()
        })
    );
    assert!(!tracker.has_pending());

    tracker.record_expansion(&whole_word_match("BRB", "\n"), 20);
    assert_eq!(
        tracker.consume(&KeyInput::Backspace),
        Some(UndoAction {
            delete_count: 21,
            retype: "BRB\n".into()
        })
    );

    tracker.record_expansion(&whole_word_match("brb", "\r\n"), 3);
    assert_eq!(
        tracker.consume(&KeyInput::Backspace).unwrap().delete_count,
        4
    );

    assert_eq!(tracker.consume(&KeyInput::Backspace), None);
}

#[test]
fn any_other_input_clears_pending_undo() {
    for other in [ch("x"), KeyInput::Reset] {
        let mut tracker = UndoTracker::new();
        tracker.record_expansion(&immediate_match(";sig"), 5);
        assert_eq!(tracker.consume(&other), None);
        assert!(!tracker.has_pending());
        assert_eq!(tracker.consume(&KeyInput::Backspace), None);
    }
}

#[test]
fn undo_is_one_shot_replaceable_and_clearable() {
    let mut tracker = UndoTracker::new();
    tracker.record_expansion(&immediate_match(";sig"), 5);
    assert!(tracker.consume(&KeyInput::Backspace).is_some());
    assert!(tracker.consume(&KeyInput::Backspace).is_none());

    tracker.record_expansion(&immediate_match(";a"), 1);
    tracker.record_expansion(&immediate_match(";b"), 2);
    assert_eq!(
        tracker.consume(&KeyInput::Backspace),
        Some(UndoAction {
            delete_count: 2,
            retype: ";b".into()
        })
    );

    tracker.record_expansion(&immediate_match(";sig"), 5);
    tracker.clear();
    assert!(tracker.consume(&KeyInput::Backspace).is_none());

    tracker.record_expansion(&immediate_match(";sig"), 0);
    assert_eq!(
        tracker.consume(&KeyInput::Backspace),
        Some(UndoAction {
            delete_count: 0,
            retype: ";sig".into()
        })
    );
}

// Matcher facade

fn matcher(snippets: &[&Snip]) -> ExpansionMatcher {
    ExpansionMatcher::with_index(make_index(snippets))
}

fn type_into(
    text: &str,
    matcher: &mut ExpansionMatcher,
    bundle_id: Option<&str>,
) -> Option<MatcherEvent> {
    let mut last = None;
    for cluster in text.graphemes(true) {
        last = matcher.handle(ch(cluster), bundle_id);
    }
    last
}

fn expect_expand(event: Option<MatcherEvent>) -> TriggerMatch {
    match event {
        Some(MatcherEvent::Expand(m)) => m,
        other => panic!("expected expand, got {other:?}"),
    }
}

#[test]
fn expands_on_trigger_and_is_quiet_otherwise() {
    let s = snip(&[";sig"]);
    let mut m = matcher(&[&s]);
    assert_eq!(
        expect_expand(type_into(";sig", &mut m, None)).snippet_id,
        s.id()
    );
    let mut m = matcher(&[&s]);
    assert_eq!(type_into("hello", &mut m, None), None);
    assert_eq!(m.handle(KeyInput::Backspace, None), None);
    assert_eq!(m.handle(KeyInput::Reset, None), None);
}

#[test]
fn backspace_and_reset_mid_trigger() {
    let mut m = matcher(&[&snip(&[";sig"])]);
    type_into(";sx", &mut m, None);
    m.handle(KeyInput::Backspace, None);
    assert!(type_into("ig", &mut m, None).is_some());

    let mut m = matcher(&[&snip(&[";sig"])]);
    type_into(";s", &mut m, None);
    m.handle(KeyInput::Reset, None);
    assert!(type_into("ig", &mut m, None).is_none());
}

#[test]
fn backspace_after_expansion_undoes() {
    let mut m = matcher(&[&snip(&[";sig"])]);
    let found = expect_expand(type_into(";Sig", &mut m, None));
    m.did_expand(&found, 12);
    assert_eq!(
        m.handle(KeyInput::Backspace, None),
        Some(MatcherEvent::Undo(UndoAction {
            delete_count: 12,
            retype: ";Sig".into()
        }))
    );
    assert_eq!(m.buffer_string(), ";Sig");
    assert_eq!(m.handle(KeyInput::Backspace, None), None);
}

#[test]
fn whole_word_undo_restores_separator() {
    let mut m = matcher(&[&snip(&["brb"]).whole_word()]);
    let found = expect_expand(type_into("brb ", &mut m, None));
    assert_eq!(found.separator.as_deref(), Some(" "));
    m.did_expand(&found, 15);
    assert_eq!(
        m.handle(KeyInput::Backspace, None),
        Some(MatcherEvent::Undo(UndoAction {
            delete_count: 16,
            retype: "brb ".into()
        }))
    );
}

#[test]
fn typing_after_expansion_cancels_undo() {
    let mut m = matcher(&[&snip(&[";sig"])]);
    let found = expect_expand(type_into(";sig", &mut m, None));
    m.did_expand(&found, 5);
    m.handle(ch("x"), None);
    assert_eq!(m.handle(KeyInput::Backspace, None), None);
}

#[test]
fn expansion_resets_buffer_so_old_text_cannot_retrigger() {
    let mut m = matcher(&[&snip(&[";a"]), &snip(&["ab"])]);
    let found = expect_expand(type_into(";a", &mut m, None));
    m.did_expand(&found, 3);
    assert!(m.buffer().is_empty());
    assert_eq!(m.handle(ch("b"), None), None);
}

#[test]
fn undo_then_normal_typing_continues() {
    let mut m = matcher(&[&snip(&[";sig"])]);
    let found = expect_expand(type_into(";sig", &mut m, None));
    m.did_expand(&found, 3);
    m.handle(KeyInput::Backspace, None);
    m.handle(KeyInput::Backspace, None);
    expect_expand(m.handle(ch("g"), None));
}

#[test]
fn bundle_id_is_respected_per_call() {
    let mut m = matcher(&[&snip(&[";x"]).rule(allow(&["com.ok"]))]);
    assert!(type_into(";x", &mut m, Some("com.no")).is_none());
    m.handle(KeyInput::Reset, None);
    assert!(type_into(";x", &mut m, Some("com.ok")).is_some());
}

#[test]
fn replace_index_swaps_index_and_clears_undo() {
    let mut m = matcher(&[&snip(&[";old"])]);
    let found = expect_expand(type_into(";old", &mut m, None));
    m.did_expand(&found, 3);
    m.replace_index(TriggerIndex::new(
        &[snip(&[";new"]).record()],
        &[],
        MatchingConfig::default(),
    ));
    assert_eq!(m.handle(KeyInput::Backspace, None), None);
    m.handle(KeyInput::Reset, None);
    assert!(type_into(";old", &mut m, None).is_none());
    assert!(type_into(";new", &mut m, None).is_some());
}

#[test]
fn reset_clears_buffer_and_undo() {
    let mut m = matcher(&[&snip(&[";sig"])]);
    let found = expect_expand(type_into(";sig", &mut m, None));
    m.did_expand(&found, 3);
    type_into("ab", &mut ExpansionMatcher::new(), None);
    m.reset();
    assert_eq!(m.handle(KeyInput::Backspace, None), None);
    type_into("xy", &mut m, None);
    assert_eq!(m.buffer_string(), "xy");
    m.reset();
    assert_eq!(m.buffer_string(), "");
}

#[test]
fn empty_matcher_is_inert() {
    let mut m = ExpansionMatcher::new();
    assert!(type_into("anything", &mut m, None).is_none());
    assert_eq!(m.buffer_string(), "anything");
}
