//! S13b: what the app can know about a chip gesture, and what a chip puts on the pasteboard.
//!
//! Rune routes a press on a `text_input` straight into the field (`input_down`). The field's own
//! `on_press`, `on_double_press` and `Drag::clicks` never run for it, and nothing reports a click
//! count, a drag, or what moved the selection. What the app does get is:
//!
//! * `on_pointer_move_local` on the field. `Runtime::release` calls `move_pointer` with the
//!   release point, so the handler also runs once on every mouse-up, after the press's
//!   selection step (events queue in that order);
//! * `on_input_key`, for every key that reaches the focused field before the editor sees it;
//! * the field's `TextEdit` (`anchor`, `focus`, `pills()`), which the handlers can read.
//!
//! [`ChipGesture`] infers a double-click from those three. It is a heuristic over event order,
//! not an API; `docs/migration/spikes/S13-editor.md` lists where it fails.

use std::ops::Range;

use kudzu_core::template::{self, Node};
use rune::{ClipboardContent, TextEdit};

/// Chip ids carry their template source: `{tone}:{source}`, for example `time:{{date}}`.
///
/// Rune's rich copy keeps only the pill id in `<span data-pill="id">`, so an id that is the
/// source is the one thing that travels through the pasteboard to another field or window.
pub fn chip_id(tone: &str, source: &str) -> String {
    format!("{tone}:{source}")
}

/// The tone of a chip id (the text before the first `:`).
pub fn chip_tone(id: &str) -> &str {
    id.split(':').next().unwrap_or("form")
}

/// The template source in a chip id, when it is exactly one tag. A foreign id, or one whose
/// source is not a single tag, is not ours.
pub fn chip_source(id: &str) -> Option<&str> {
    let (_, source) = id.split_once(':')?;
    let parsed = template::parse(source);
    matches!(parsed.nodes.as_slice(), [Node::Tag(_)]).then_some(source)
}

fn push_literal(out: &mut String, text: &str) {
    if text.is_empty() {
        return;
    }
    out.push_str(&template::serialize(&[Node::Text {
        text: text.to_string(),
        range: 0..0,
    }]));
}

/// The template source for `range` of the field's display text: chips become their tags and
/// literal runs are escaped (`\` as `\\`, `{{` as `\{{`) by `kudzu_core`'s own serializer.
/// A chip the range only partly covers reads as its label, as plain literal text.
pub fn source_of(edit: &TextEdit, range: Range<usize>) -> String {
    let mut out = String::new();
    let mut at = range.start;
    for (pill, id) in edit.pills() {
        if pill.end <= range.start || pill.start >= range.end {
            continue;
        }
        let (start, end) = (pill.start.max(range.start), pill.end.min(range.end));
        push_literal(&mut out, &edit.text[at..start]);
        match chip_source(id).filter(|_| pill.start >= range.start && pill.end <= range.end) {
            Some(source) => out.push_str(source),
            None => push_literal(&mut out, &edit.text[start..end]),
        }
        at = end;
    }
    push_literal(&mut out, &edit.text[at..range.end]);
    out
}

/// What Cmd+C or Cmd+X puts on the pasteboard: the template source as plain text, and Rune's
/// own rich copy beside it (`selected_html`: marks, and `<span data-pill=id>` for each chip).
pub fn copy_content(edit: &TextEdit) -> Option<ClipboardContent> {
    let (start, end) = edit.range();
    if start == end || edit.is_secure() {
        return None;
    }
    let plain = source_of(edit, start..end);
    let html = edit.selected_html();
    let content = ClipboardContent::new(plain);
    Some(if html.is_empty() {
        content
    } else {
        content.with_html(html)
    })
}

/// A paste this app wrote itself: its HTML carries only chips this app can read back, and the
/// plain text is exactly the template source those chips and the text between them make.
/// Another app's HTML, or a hand-edited clipboard, fails this and pastes the way Rune pastes it.
pub fn own_fragment(content: &ClipboardContent) -> Option<TextEdit> {
    let html = content.html()?;
    let fragment = TextEdit::from_html(html);
    if !fragment.pills().all(|(_, id)| chip_source(id).is_some()) {
        return None;
    }
    let source = source_of(&fragment, 0..fragment.text.len());
    (source == content.text()).then_some(fragment)
}

/// Pastes an own copy into `edit`: chips come back as chips, and an all-literal copy pastes its
/// display text instead of its escaped source. One undo step. False when the copy is not ours.
pub fn paste_own(edit: &mut TextEdit, content: &ClipboardContent) -> bool {
    let Some(fragment) = own_fragment(content) else {
        return false;
    };
    let Some(html) = content.html() else {
        return false;
    };
    edit.paste_html_with_pills(html, &fragment.text);
    true
}

/// The chip that is exactly the selection.
pub fn selected_chip(edit: &TextEdit) -> Option<(Range<usize>, String)> {
    crate::field::selected_pill(edit)
}

/// Infers "the user double-clicked this chip" from the events Rune does give the app.
///
/// The field's mouse-up arrives as one more pointer event at the same point as the one before
/// it. A double-click on a chip is therefore the approach (one move event) and two releases:
/// three events at one point, and the second release is the first to see the selection become
/// exactly the chip, from the caret the first press left on a chip edge. A single click is two
/// events. A drag moves the point between events, so its release is at most the second at its
/// final point. A key since the last event hands the selection to the keyboard.
#[derive(Debug, Default, Clone)]
pub struct ChipGesture {
    point: Option<(f32, f32)>,
    /// Pointer events in a row at `point`.
    run: u32,
    /// Keys the field saw since the last pointer event.
    keys: u32,
    selection: Option<(usize, usize)>,
}

impl ChipGesture {
    /// A key reached the focused field.
    pub fn key(&mut self) {
        self.keys += 1;
    }

    /// A pointer event over the field at window point `at`; `Some` when it completes a
    /// double-click on a chip.
    pub fn pointer(&mut self, at: (f32, f32), edit: &TextEdit) -> Option<(Range<usize>, String)> {
        self.run = if self.point == Some(at) {
            self.run + 1
        } else {
            1
        };
        self.point = Some(at);
        let now = (edit.anchor, edit.focus);
        let before = self.selection.replace(now);
        let keyed = std::mem::take(&mut self.keys) > 0;
        if before == Some(now) || keyed {
            return None;
        }
        let from_caret = before.is_some_and(|(a, f)| a == f);
        if self.run >= 3 && from_caret {
            return selected_chip(edit);
        }
        None
    }
}
