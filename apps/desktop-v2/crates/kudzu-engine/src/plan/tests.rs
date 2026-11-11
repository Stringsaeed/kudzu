//! Edit planning: backspaces, case, cursor, images, sequences and the type-or-paste rule.

use super::*;
use uuid::Uuid;

fn trigger(typed: &str, separator: Option<&str>, fire_mode: FireMode) -> TriggerMatch {
    TriggerMatch {
        snippet_id: Uuid::nil(),
        trigger: typed.to_lowercase(),
        typed_trigger: typed.to_string(),
        delete_count: grapheme_count(typed),
        separator: separator.map(String::from),
        insertion: InsertionMethod::Automatic,
        case_style: CaseStyle::AsIs,
        fire_mode,
    }
}

fn key_at(offset: usize, key: Key, count: u32) -> RenderAction {
    RenderAction {
        offset,
        action: ActionKind::Key { key, count },
    }
}

fn pause_at(offset: usize, ms: u32) -> RenderAction {
    RenderAction {
        offset,
        action: ActionKind::Pause { ms },
    }
}

fn plain(text: &str) -> Step {
    Step::Text {
        text: text.into(),
        spans: None,
    }
}

#[test]
fn sequence_splits_text_at_each_action_in_order() {
    // "user", Tab, "pass", pause, Return ×2 — a login form.
    let actions = [
        key_at(4, Key::Tab, 1),
        pause_at(8, 200),
        key_at(8, Key::Return, 2),
    ];
    let seq = Sequence::new("userpass", None, &actions, None);
    assert_eq!(
        seq.steps,
        [
            plain("user"),
            Step::Key {
                key: Key::Tab,
                count: 1
            },
            plain("pass"),
            Step::Pause(Duration::from_millis(200)),
            Step::Key {
                key: Key::Return,
                count: 2
            },
        ]
    );
    assert!(seq.has_keys && !seq.has_images);
    assert_eq!(seq.caret_left, 0);
}

#[test]
fn sequence_handles_edges_and_wide_characters() {
    // Actions at the very start and end, and offsets after a surrogate pair.
    let actions = [
        key_at(0, Key::Tab, 1),
        key_at(2, Key::Down, 3),
        key_at(3, Key::Escape, 1),
    ];
    let seq = Sequence::new("😀a", None, &actions, None);
    assert_eq!(
        seq.steps,
        [
            Step::Key {
                key: Key::Tab,
                count: 1
            },
            plain("😀"),
            Step::Key {
                key: Key::Down,
                count: 3
            },
            plain("a"),
            Step::Key {
                key: Key::Escape,
                count: 1
            },
        ]
    );
    // An offset past the end (or out of order) never panics or repeats text.
    let odd = Sequence::new(
        "ab",
        None,
        &[key_at(9, Key::Tab, 1), key_at(1, Key::Tab, 1)],
        None,
    );
    assert_eq!(
        odd.steps,
        [
            plain("ab"),
            Step::Key {
                key: Key::Tab,
                count: 1
            },
            Step::Key {
                key: Key::Tab,
                count: 1
            }
        ]
    );
    // Pauses alone keep the expansion one contiguous text.
    let paused = Sequence::new("ab", None, &[pause_at(1, 50)], Some(1));
    assert!(!paused.has_keys);
    assert_eq!(paused.caret_left, 1);
    assert_eq!(Sequence::new("", None, &[], None).steps, []);
}

#[test]
fn sequence_caret_counts_graphemes_after_the_cursor() {
    let seq = Sequence::new("to: 👋🏽 end", None, &[key_at(3, Key::Tab, 1)], Some(4));
    assert_eq!(seq.caret_left, 5);
}

#[test]
fn sequence_clips_formatting_to_each_text_step() {
    let image = FormatStyle::Image {
        data_url: "data:image/png;base64,AAAA".into(),
        width: 10,
    };
    let spans = [
        FormatSpan {
            start: 1,
            end: 6,
            style: FormatStyle::Bold,
        },
        FormatSpan {
            start: 2,
            end: 5,
            style: image.clone(),
        },
        FormatSpan {
            start: 6,
            end: 8,
            style: image.clone(),
        },
    ];
    let seq = Sequence::new("abcdefgh", Some(&spans), &[key_at(4, Key::Tab, 1)], None);
    assert_eq!(
        seq.steps,
        [
            Step::Text {
                text: "abcd".into(),
                spans: Some(vec![FormatSpan {
                    start: 1,
                    end: 4,
                    style: FormatStyle::Bold
                }])
            },
            Step::Key {
                key: Key::Tab,
                count: 1
            },
            Step::Text {
                text: "efgh".into(),
                spans: Some(vec![
                    FormatSpan {
                        start: 0,
                        end: 2,
                        style: FormatStyle::Bold
                    },
                    FormatSpan {
                        start: 2,
                        end: 4,
                        style: image
                    },
                ]),
            },
        ]
    );
    assert!(seq.has_images);
}

#[test]
fn action_offsets_follow_unicode_case_expansion() {
    let cased = cased_actions(CaseStyle::Upper, "ß hi", &[key_at(2, Key::Tab, 1)]);
    assert_eq!(cased[0].offset, 3);
}

#[test]
fn utf16_slice_snaps_to_characters() {
    assert_eq!(utf16_slice("a😀b", 1, 3), "😀");
    assert_eq!(utf16_slice("a😀b", 2, 3), "😀");
    assert_eq!(utf16_slice("abc", 2, 1), "");
    assert_eq!(utf16_slice("abc", 1, usize::MAX), "bc");
}

#[test]
fn formatting_follows_unicode_case_expansion() {
    use kudzu_core::formatting::{FormatSpan, FormatStyle};
    let spans = cased_formatting(
        CaseStyle::Upper,
        "ß hi",
        &[FormatSpan {
            start: 2,
            end: 4,
            style: FormatStyle::Bold,
        }],
    );
    assert_eq!((spans[0].start, spans[0].end), (3, 5));
}

#[test]
fn image_caret_and_undo_use_the_confirmed_clipboard_flavor() {
    use kudzu_core::formatting::{FormatSpan, FormatStyle};
    let spans = [FormatSpan { start: 3, end: 9, style: FormatStyle::Image {
        data_url: "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAABgAAAAYCAYAAADgdz34AAAAJUlEQVR4nGMICLj0n5aYYdSCUQtGLRi1YNSCUQtGLRi1YGhYAACpwH6M7RqhEQAAAABJRU5ErkJggg==".into(), width: 120,
    } }];
    assert_eq!(
        image_delivery("Hi [Logo]!", &spans, Some(9), Some(5)),
        Some(("Hi \u{fffc}!".into(), Some(4)))
    );
    assert_eq!(
        image_delivery("Hi [Logo]!", &spans, Some(9), Some(10)),
        Some(("Hi [Logo]!".into(), Some(9)))
    );
    assert_eq!(image_delivery("Hi [Logo]!", &spans, Some(9), None), None);
    assert_eq!(
        image_delivery("Hi [Logo]!", &spans, Some(9), Some(99)),
        None
    );
}

#[test]
fn active_immediate_skips_the_swallowed_key() {
    let m = trigger(";sig", None, FireMode::Immediate);
    let plan = InsertionPlan::new(&m, "Best".into(), None, PlanMode::Active, Some("g"));
    assert_eq!(plan.delete_count, 3);
    assert_eq!(plan.reemit_separator, None);
    assert_eq!(plan.caret_left_count, 0);
}

#[test]
fn active_immediate_with_combining_mark_measures_the_visible_part() {
    // "e" + combining acute is one grapheme; swallowing the mark leaves "e" visible.
    let m = trigger(";e\u{301}", None, FireMode::Immediate);
    assert_eq!(m.delete_count, 2);
    let plan = InsertionPlan::new(&m, "x".into(), None, PlanMode::Active, Some("\u{301}"));
    assert_eq!(plan.delete_count, 2);
}

#[test]
fn active_immediate_without_swallowed_text_falls_back() {
    let m = trigger("abc", None, FireMode::Immediate);
    let plan = InsertionPlan::new(&m, "x".into(), None, PlanMode::Active, None);
    assert_eq!(plan.delete_count, 2);
}

#[test]
fn active_whole_word_keeps_separator_out_of_the_document() {
    let m = trigger("brb", Some(" "), FireMode::WholeWord);
    let plan = InsertionPlan::new(
        &m,
        "be right back".into(),
        None,
        PlanMode::Active,
        Some(" "),
    );
    assert_eq!(plan.delete_count, 3);
    assert_eq!(plan.reemit_separator.as_deref(), Some(" "));
}

#[test]
fn listen_only_deletes_the_separator_too() {
    let m = trigger("brb", Some("."), FireMode::WholeWord);
    let plan = InsertionPlan::new(&m, "be right back".into(), None, PlanMode::ListenOnly, None);
    assert_eq!(plan.delete_count, 4);
    let immediate = trigger(";x", None, FireMode::Immediate);
    let plan = InsertionPlan::new(
        &immediate,
        "y".into(),
        None,
        PlanMode::ListenOnly,
        Some("x"),
    );
    assert_eq!(plan.delete_count, 2);
}

#[test]
fn caret_counts_graphemes_after_cursor_plus_separator() {
    let m = trigger("sig", Some(" "), FireMode::WholeWord);
    // "Hi 👋🏽 there", cursor after "Hi " (3 UTF-16 units).
    let text = "Hi 👋🏽 there".to_string();
    let plan = InsertionPlan::new(&m, text, Some(3), PlanMode::Active, None);
    assert_eq!(plan.caret_left_count, 7 + 1);
}

#[test]
fn caret_inside_a_grapheme_snaps_to_its_start() {
    // 👋🏽 is 4 UTF-16 units; an offset of 2 lands inside it.
    assert_eq!(graphemes_after_utf16("👋🏽a", 2), 2);
    assert_eq!(graphemes_after_utf16("ab", 0), 2);
    assert_eq!(graphemes_after_utf16("ab", 5), 0);
    assert_eq!(graphemes_after_utf16("a\r\nb", 2), 2);
}

#[test]
fn utf16_prefix_never_splits_surrogates() {
    assert_eq!(utf16_prefix("a😀b", 1), "a");
    assert_eq!(utf16_prefix("a😀b", 2), "a");
    assert_eq!(utf16_prefix("a😀b", 3), "a😀");
    assert_eq!(utf16_prefix("a😀b", 99), "a😀b");
}

#[test]
fn as_is_cursor_offset_is_unchanged() {
    assert_eq!(cased_cursor_offset(CaseStyle::AsIs, "abc", 2), 2);
}

#[test]
fn chars_saved_never_negative() {
    assert_eq!(chars_saved("Best regards", ";sig"), 8);
    assert_eq!(chars_saved("a", ";long"), 0);
    assert_eq!(chars_saved("👋🏽👋🏽", "w"), 1);
}

#[test]
fn automatic_types_short_single_line_text() {
    assert!(should_type(InsertionMethod::Automatic, "hello"));
    assert!(should_type(InsertionMethod::Automatic, "one\ntwo"));
    assert!(!should_type(InsertionMethod::Automatic, "one\ntwo\nthree"));
    assert!(!should_type(
        InsertionMethod::Automatic,
        "one\r\ntwo\r\nthree"
    ));
    assert!(!should_type(InsertionMethod::Automatic, &"x".repeat(100)));
    assert!(should_type(InsertionMethod::Automatic, &"x".repeat(99)));
    assert!(should_type(InsertionMethod::Type, &"x".repeat(500)));
    assert!(!should_type(InsertionMethod::Paste, "x"));
}

#[test]
fn formatted_text_is_pasted_unless_typing_was_asked_for() {
    assert!(types(InsertionMethod::Automatic, "hello", false, false));
    assert!(!types(InsertionMethod::Automatic, "hello", true, false));
    assert!(types(InsertionMethod::Type, "hello", true, false));
    assert!(!types(InsertionMethod::Paste, "hello", false, false));
    assert!(!types(
        InsertionMethod::Automatic,
        &"x".repeat(100),
        false,
        false
    ));
}

#[test]
fn nothing_is_typed_into_a_composing_input_method() {
    assert!(!types(InsertionMethod::Automatic, "hello", false, true));
    assert!(!types(InsertionMethod::Type, "hello", false, true));
    assert!(!types(InsertionMethod::Type, "hello", true, true));
    assert!(!types(InsertionMethod::Paste, "hello", false, true));
}
