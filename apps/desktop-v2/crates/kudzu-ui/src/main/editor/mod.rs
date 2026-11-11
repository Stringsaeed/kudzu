//! Snippet editor screen stub. Owned and filled out by S51–S54 (`crates/kudzu-ui/src/main/editor/**`).

use rune::{col, text, text_input, El, Key, KeyEvent, TextEdit, Tx};

use crate::main::library;
use crate::main::state::MainPane;
use crate::shell::MainState;
use crate::theme::{icons, radius, Theme};
use crate::widgets::empty_state::EmptyState;

pub const CONTENT_KEY: &str = "editor-content";
pub const PANE_KEY: &str = "editor-pane-slot";

/// Per-screen state owned by the snippet editor module (S51–S54).
#[derive(Clone, Debug, Default)]
pub struct State {
    pub content: TextEdit,
}

impl State {
    pub fn is_composing(&self) -> bool {
        self.content.is_composing()
    }

    pub fn is_editing_text(&self) -> bool {
        self.content.focused
    }

    pub fn focus_content(&mut self, tx: &mut Tx) {
        self.content.focused = true;
        tx.focus(CONTENT_KEY, true);
    }

    pub fn blur_inputs(&mut self) {
        self.content.focused = false;
    }
}

/// Placeholder editor pane until S51 lands. Renders an empty state when no snippet is selected, or
/// a `"Snippet content"` field for keyboard focus (`⌘E` / `Escape`) and IME guard verification.
pub fn view(s: &MainState, theme: &Theme, pane_focused: bool) -> El {
    let c = &theme.c;
    let ty = &theme.ty;
    let lib = s.library_snapshot();
    let selected = s
        .frame
        .selected_snippet
        .and_then(|id| lib.snippets.iter().find(|sn| sn.id == id));

    let body = match selected {
        Some(sn) => {
            let name = if sn.name.is_empty() {
                "Untitled"
            } else {
                sn.name.as_str()
            };
            let field = text_input(&s.editor.content, ty.input)
                .key(CONTENT_KEY)
                .w_full()
                .label("Snippet content")
                .color(c.foreground)
                .placeholder("Type snippet content…", c.muted_foreground)
                .caret(c.foreground, theme.kit().selection())
                .on_edit(|st: &mut MainState| &mut st.editor.content)
                .on_input_key(|st: &mut MainState, ev: &KeyEvent, tx: &mut Tx| {
                    if ev.key == Key::Escape
                        && !ev.mods.cmd
                        && !ev.mods.ctrl
                        && !ev.mods.alt
                        && !ev.mods.shift
                        && !st.is_composing()
                        && !st.frame.any_modal_open()
                    {
                        st.blur_all_inputs();
                        tx.blur();
                        st.frame.focus.move_to(MainPane::List);
                        tx.focus(library::LIST_KEY, false);
                        return true;
                    }
                    false
                });
            let editor_box = col()
                .role("textbox")
                .w_full()
                .min_h(64.0)
                .px(11.0)
                .py(9.0)
                .radius(radius::LG)
                .fill(c.input.alpha(0.30))
                .border(
                    1.0,
                    if s.editor.content.focused {
                        c.ring
                    } else {
                        c.border
                    },
                )
                .child(field);

            col()
                .key(format!("editor-snippet-{}", sn.id))
                .w_full()
                .grow(1.0)
                .padding(24.0)
                .gap(12.0)
                .child(text(name).font(ty.title).color(c.foreground))
                .child(editor_box)
        }
        None => EmptyState::new(&icons::FILE_TEXT, "Select a snippet")
            .description("Choose a snippet from the list or press ⌘N to create a new one.")
            .build(theme),
    };

    let mut pane = col()
        .key(PANE_KEY)
        .grow(1.0)
        .min_w(0.0)
        .h_full()
        .min_h(0.0)
        .child(body);
    if pane_focused {
        pane = pane.focus_selected(true);
    }
    pane
}
