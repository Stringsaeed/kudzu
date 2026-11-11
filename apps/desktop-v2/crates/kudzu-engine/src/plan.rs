//! Platform-independent edit planning: how many backspaces, what to type, where the caret goes.

use std::time::Duration;

use kudzu_core::formatting::{FormatSpan, FormatStyle};
use kudzu_core::matching::{case_propagation, CaseStyle, TriggerMatch};
use kudzu_core::template::{ActionKind, Key, RenderAction};
use kudzu_core::types::{FireMode, InsertionMethod};
use unicode_segmentation::UnicodeSegmentation;

/// Whether the keystroke that completed a trigger could be withheld from the target app.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PlanMode {
    /// The tap swallowed the final key (immediate) or the separator (whole word).
    Active,
    /// Every typed key, including any separator, already reached the document.
    ListenOnly,
}

/// Edits needed to replace a matched trigger.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct InsertionPlan {
    /// Backspaces before inserting text.
    pub delete_count: usize,
    /// Rendered, case-propagated expansion.
    pub text: String,
    /// Whole-word terminator to type after the expansion.
    pub reemit_separator: Option<String>,
    /// Left-arrow presses after text and separator are inserted.
    pub caret_left_count: usize,
}

impl InsertionPlan {
    /// Plans edits. `cursor_offset` is in UTF-16 units of `text`; arrow counts are grapheme clusters.
    ///
    /// `swallowed` is the text of the keystroke an active tap withheld. One keystroke doesn't always
    /// add one grapheme (a combining mark merges into the previous character), so the visible
    /// remainder of the trigger is measured rather than assumed.
    pub fn new(
        m: &TriggerMatch,
        text: String,
        cursor_offset: Option<usize>,
        mode: PlanMode,
        swallowed: Option<&str>,
    ) -> Self {
        let delete_count = match mode {
            PlanMode::Active if m.fire_mode == FireMode::Immediate => {
                match swallowed
                    .filter(|s| !s.is_empty())
                    .and_then(|s| m.typed_trigger.strip_suffix(s))
                {
                    Some(visible) => grapheme_count(visible),
                    None => m.delete_count.saturating_sub(1),
                }
            }
            PlanMode::Active => m.delete_count,
            PlanMode::ListenOnly => m.delete_count_including_separator(),
        };
        let caret_left_count = match cursor_offset {
            Some(offset) => {
                graphemes_after_utf16(&text, offset)
                    + m.separator.as_deref().map_or(0, grapheme_count)
            }
            None => 0,
        };
        Self {
            delete_count,
            text,
            reemit_separator: m.separator.clone(),
            caret_left_count,
        }
    }
}

/// One step of an expansion that has key presses or pauses (`{{key}}`, `{{delay}}`).
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Step {
    /// Deliver this text (typed or pasted, like a whole expansion would be), with its own formatting.
    Text {
        text: String,
        spans: Option<Vec<FormatSpan>>,
    },
    Key {
        key: Key,
        count: u32,
    },
    Pause(Duration),
}

/// An expansion as ordered steps. See "Key presses and pauses" in docs/TEMPLATE_SYNTAX.md for the rules.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Sequence {
    pub steps: Vec<Step>,
    /// Some step presses a key, so the text may end up spread over several fields: the expansion
    /// can't be undone with Backspace and a whole-word trigger's separator isn't typed again.
    pub has_keys: bool,
    /// Left-arrow presses after the last step that put the caret at `{{cursor}}`. The renderer only
    /// keeps a cursor that comes after the last key press, so the caret is still in that text.
    pub caret_left: usize,
    /// Some text carries an image, whose delivered length depends on the target app.
    pub has_images: bool,
}

impl Sequence {
    /// Splits `text` at each action. `spans`, `actions` and `cursor` are in UTF-16 units of `text`
    /// (already case-propagated). Empty text between two actions produces no step.
    pub fn new(
        text: &str,
        spans: Option<&[FormatSpan]>,
        actions: &[RenderAction],
        cursor: Option<usize>,
    ) -> Self {
        let mut steps = Vec::new();
        let mut from = 0;
        let push_text = |steps: &mut Vec<Step>, from: usize, to: usize| {
            let piece = utf16_slice(text, from, to);
            if !piece.is_empty() {
                steps.push(Step::Text {
                    text: piece.to_string(),
                    spans: spans.map(|s| clip_spans(s, from, to)),
                });
            }
        };
        for action in actions {
            let at = action.offset.max(from);
            push_text(&mut steps, from, at);
            from = at;
            steps.push(match action.action {
                ActionKind::Key { key, count } => Step::Key { key, count },
                ActionKind::Pause { ms } => Step::Pause(Duration::from_millis(u64::from(ms))),
            });
        }
        push_text(&mut steps, from, usize::MAX);
        let is_image = |s: &FormatSpan| matches!(s.style, FormatStyle::Image { .. });
        Sequence {
            has_keys: steps.iter().any(|s| matches!(s, Step::Key { .. })),
            caret_left: cursor.map_or(0, |offset| graphemes_after_utf16(text, offset)),
            has_images: steps.iter().any(|s| matches!(s, Step::Text { spans: Some(spans), .. } if spans.iter().any(is_image))),
            steps,
        }
    }
}

/// `text` between UTF-16 offsets `from` and `to`, snapped to character boundaries.
pub(crate) fn utf16_slice(text: &str, from: usize, to: usize) -> &str {
    let start = utf16_prefix(text, from).len();
    let end = utf16_prefix(text, to).len();
    &text[start..end.max(start)]
}

/// The parts of `spans` inside `from..to`, re-based to start at 0. An image cut by the boundary
/// is dropped (its text alternative stays), since half an image can't be delivered.
fn clip_spans(spans: &[FormatSpan], from: usize, to: usize) -> Vec<FormatSpan> {
    spans
        .iter()
        .filter_map(|span| {
            let (start, end) = (span.start.max(from), span.end.min(to));
            let whole = span.start >= from && span.end <= to;
            let keep = start < end && (whole || !matches!(span.style, FormatStyle::Image { .. }));
            keep.then(|| FormatSpan {
                start: start - from,
                end: end - from,
                style: span.style.clone(),
            })
        })
        .collect()
}

/// Re-maps action offsets after case propagation, like [`cased_cursor_offset`].
pub(crate) fn cased_actions(
    style: CaseStyle,
    rendered: &str,
    actions: &[RenderAction],
) -> Vec<RenderAction> {
    actions
        .iter()
        .map(|a| RenderAction {
            offset: cased_cursor_offset(style, rendered, a.offset),
            action: a.action,
        })
        .collect()
}

/// Number of grapheme clusters (what one backspace or arrow press moves over).
pub(crate) fn grapheme_count(text: &str) -> usize {
    text.graphemes(true).count()
}

/// Graphemes at or after UTF-16 `offset`; an offset inside a grapheme snaps to its start.
pub(crate) fn graphemes_after_utf16(text: &str, offset: usize) -> usize {
    let mut position = 0;
    let mut after = 0;
    for g in text.graphemes(true) {
        let len = g.encode_utf16().count();
        if position >= offset || position + len > offset {
            after += 1;
        }
        position += len;
    }
    after
}

/// The prefix of `text` covering at most `offset` UTF-16 units.
pub(crate) fn utf16_prefix(text: &str, offset: usize) -> &str {
    let mut units = 0;
    for (index, c) in text.char_indices() {
        if units + c.len_utf16() > offset {
            return &text[..index];
        }
        units += c.len_utf16();
    }
    text
}

/// Re-maps a UTF-16 cursor offset after case propagation (which can change lengths, e.g. ß → SS).
pub(crate) fn cased_cursor_offset(style: CaseStyle, rendered: &str, offset: usize) -> usize {
    if style == CaseStyle::AsIs {
        return offset;
    }
    case_propagation::apply(style, utf16_prefix(rendered, offset))
        .encode_utf16()
        .count()
}

pub(crate) fn cased_formatting(
    style: CaseStyle,
    text: &str,
    spans: &[kudzu_core::formatting::FormatSpan],
) -> Vec<kudzu_core::formatting::FormatSpan> {
    spans
        .iter()
        .map(|s| kudzu_core::formatting::FormatSpan {
            start: cased_cursor_offset(style, text, s.start),
            end: cased_cursor_offset(style, text, s.end),
            style: s.style.clone(),
        })
        .collect()
}

/// Choose the attachment or plain-text shape only when the app's caret confirms it.
/// Unknown delivery must not arm a destructive Backspace undo or move the caret by a guess.
pub(crate) fn image_delivery(
    text: &str,
    spans: &[kudzu_core::formatting::FormatSpan],
    cursor: Option<usize>,
    observed: Option<usize>,
) -> Option<(String, Option<usize>)> {
    let length = observed?;
    if length == text.encode_utf16().count() {
        return Some((text.into(), cursor));
    }
    let visual = kudzu_core::formatting::insertion_text(text, spans);
    (length == visual.encode_utf16().count()).then(|| {
        (
            visual,
            cursor.map(|o| kudzu_core::formatting::insertion_offset(o, spans)),
        )
    })
}

/// Inserted graphemes minus typed trigger graphemes, never negative.
pub(crate) fn chars_saved(inserted: &str, typed_trigger: &str) -> i64 {
    let saved = grapheme_count(inserted) as i64 - grapheme_count(typed_trigger) as i64;
    saved.max(0)
}

/// Whether `method` should deliver `text` as keystrokes (true) or via paste (false).
///
/// Automatic types short text with at most one line break; long or multi-line text pastes, because
/// synthetic keystrokes are slow and auto-indenting editors mangle typed newlines.
pub(crate) fn should_type(method: InsertionMethod, text: &str) -> bool {
    match method {
        InsertionMethod::Type => true,
        InsertionMethod::Paste => false,
        InsertionMethod::Automatic => {
            let mut graphemes = 0;
            let mut newlines = 0;
            for g in text.graphemes(true) {
                graphemes += 1;
                if g.chars().next().is_some_and(is_newline) {
                    newlines += 1;
                }
            }
            graphemes < 100 && newlines < 2
        }
    }
}

/// Whether a piece of an expansion is typed (true) or pasted (false).
///
/// Formatted text needs the clipboard unless typing was asked for. While an input method is
/// composing, typed events would be fed into the composition, so everything is pasted.
pub(crate) fn types(method: InsertionMethod, text: &str, formatted: bool, composing: bool) -> bool {
    !composing && should_type(method, text) && (!formatted || method == InsertionMethod::Type)
}

fn is_newline(c: char) -> bool {
    matches!(
        c,
        '\n' | '\r' | '\u{0B}' | '\u{0C}' | '\u{85}' | '\u{2028}' | '\u{2029}'
    )
}

#[cfg(test)]
mod tests {
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
}
