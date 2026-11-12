//! The content field. **S51's stand-in, which S52 replaces**: a plain multi-line text area bound
//! to the template source, in the content card of `snippet-editor.tsx:446`. The rest of the
//! editor reaches the field only through this interface, so S52 swaps this file and nothing else:
//!
//! - [`Field`]: `load(source)` shows a snippet's source (open, outside change); `source()` is
//!   what the draft saves; `is_composing`, `is_focused`, `blur`, `focus(tx)`.
//! - [`toolbar`]: the controls at the end of the "Expands to" row (the quick toolbar and Insert
//!   are S52's; the stand-in has none).
//! - [`view`]: the field in its card, keyed [`super::CONTENT_KEY`]. Every edit calls
//!   [`super::content_changed`], which copies `source()` into the draft and restarts autosave.
//!
//! Its key handler ends in [`super::field_key`] (⌘S writes now, Escape returns to the list);
//! keys do nothing mid-composition.

use rune::{col, text_input, El, KeyEvent, TextEdit, Tx};

use crate::shell::MainState;
use crate::theme::{radius, Theme};

/// `.kz-editor`: Paper Mono 13 on a 1.7 line, 12 pt above and below, 14 pt at the sides.
const PAD_X: f32 = 14.0;
const PAD_Y: f32 = 12.0;
/// `min-h-[168px]` on the editor.
pub const MIN_HEIGHT: f32 = 168.0;
pub const PLACEHOLDER: &str =
    "Type the text to expand. Press / to add a date, clipboard, fill-in field…";

/// The template source being edited.
#[derive(Clone, Debug, Default)]
pub struct Field {
    edit: TextEdit,
}

impl Field {
    /// Shows `source`, keeping focus.
    pub fn load(&mut self, source: &str) {
        let focused = self.edit.focused;
        self.edit = TextEdit::new(source);
        self.edit.focused = focused;
    }

    pub fn source(&self) -> &str {
        &self.edit.text
    }

    pub fn is_composing(&self) -> bool {
        self.edit.is_composing()
    }

    pub fn is_focused(&self) -> bool {
        self.edit.focused
    }

    pub fn blur(&mut self) {
        self.edit.focused = false;
    }

    pub fn focus(&mut self, tx: &mut Tx) {
        self.edit.focused = true;
        tx.focus(super::CONTENT_KEY, true);
    }
}

/// Controls beside "Expands to". None until S52.
pub fn toolbar(_: &MainState, _: &Theme) -> Option<El> {
    None
}

/// The field inside the content card (`rounded-xl bg-surface shadow-card`, with the focus ring
/// `0 0 0 1px ring, 0 0 0 4px iris-soft`).
pub fn view(s: &MainState, theme: &Theme) -> El {
    let c = &theme.c;
    let field = text_input(&s.editor.content.edit, theme.ty.mono)
        .key(super::CONTENT_KEY)
        .label("Snippet content")
        .w_full()
        .min_h(MIN_HEIGHT)
        .px(PAD_X)
        .py(PAD_Y)
        .color(c.foreground)
        .placeholder(PLACEHOLDER, c.faint)
        .caret(c.iris, theme.kit().selection())
        .spellcheck(true)
        .on_edit(|st: &mut MainState| &mut st.editor.content.edit)
        .on_input_change(super::content_changed)
        .on_submit(|st: &mut MainState, tx: &mut Tx| {
            st.editor.content.edit.insert("\n");
            super::content_changed(st, tx);
        })
        .on_input_key(on_key);
    let mut card = col()
        .key("editor-content-card")
        .w_full()
        .radius(radius::XL)
        .fill(c.surface)
        .child(field);
    let ring = |spread: f32, color: rune::Color| rune::Shadow {
        x: 0.0,
        y: 0.0,
        blur: 0.0,
        spread,
        color,
        inset: false,
    };
    let shadows = if s.editor.content.is_focused() {
        vec![ring(1.0, c.ring), ring(4.0, c.iris_soft)]
    } else {
        theme.depth.card.clone()
    };
    for shadow in shadows {
        card = card.box_shadow(shadow);
    }
    card
}

fn on_key(s: &mut MainState, ev: &KeyEvent, tx: &mut Tx) -> bool {
    if s.editor.content.is_composing() {
        return false;
    }
    super::field_key(s, ev, tx)
}
