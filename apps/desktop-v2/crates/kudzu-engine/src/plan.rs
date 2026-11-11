//! Platform-independent edit planning: how many backspaces, what to type, where the caret goes.

use std::time::Duration;

use kudzu_core::formatting::{FormatSpan, FormatStyle};
use kudzu_core::matching::{case_propagation, CaseStyle, TriggerMatch};
use kudzu_core::template::{ActionKind, Key, RenderAction};
use kudzu_core::{FireMode, InsertionMethod};
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
mod tests;
