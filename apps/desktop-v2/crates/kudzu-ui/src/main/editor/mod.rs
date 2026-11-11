//! Snippet editor screen stub. Owned and filled out by S51–S54 (`crates/kudzu-ui/src/main/editor/**`).

use kudzu_core::SnippetRecord;
use rune::{col, text_input, El, Key, KeyEvent, TextEdit, Tx};
use uuid::Uuid;

use crate::main::library;
use crate::main::state::MainPane;
use crate::shell::MainState;
use crate::theme::{icons, radius, Theme};
use crate::widgets::empty_state::EmptyState;

pub const CONTENT_KEY: &str = "editor-content";
pub const NAME_KEY: &str = "editor-name";
pub const PANE_KEY: &str = "editor-pane-slot";

/// Per-screen state owned by the snippet editor module (S51–S54).
#[derive(Clone, Debug, Default)]
pub struct State {
    pub name: TextEdit,
    pub content: TextEdit,
    /// Tauri's `focusNameOf`: a new or duplicated snippet whose name field takes focus once it
    /// shows. It outlives the route that asked for it; S51 takes it with [`State::take_name_focus`].
    pub focus_name_of: Option<Uuid>,
}

impl State {
    pub fn is_composing(&self) -> bool {
        self.name.is_composing() || self.content.is_composing()
    }

    pub fn is_editing_text(&self) -> bool {
        self.name.focused || self.content.focused
    }

    /// Loads the fields from the snippet that just opened.
    pub fn show(&mut self, snippet: &SnippetRecord) {
        self.name = TextEdit::new(&snippet.name);
        self.content = TextEdit::new(&snippet.content);
    }

    /// Focuses `id`'s name once it shows (`store.ts` `newSnippet`). The field is focused through
    /// its model, so this needs no `Tx` and works from a route applied in `poll`.
    pub fn request_name_focus(&mut self, id: Uuid) {
        self.blur_inputs();
        self.focus_name_of = Some(id);
        self.name.focused = true;
    }

    /// Tauri's `consumeFocusName`.
    pub fn take_name_focus(&mut self) -> Option<Uuid> {
        self.focus_name_of.take()
    }

    pub fn focus_content(&mut self, tx: &mut Tx) {
        self.content.focused = true;
        tx.focus(CONTENT_KEY, true);
    }

    pub fn blur_inputs(&mut self) {
        self.name.focused = false;
        self.content.focused = false;
    }
}

/// Escape in an editor field returns to the list (`main-app.tsx:146-156`).
fn escape_to_list(st: &mut MainState, ev: &KeyEvent, tx: &mut Tx) -> bool {
    let bare = !ev.mods.cmd && !ev.mods.ctrl && !ev.mods.alt && !ev.mods.shift;
    if ev.key == Key::Escape && bare && !st.is_composing() && !st.frame.any_modal_open() {
        library::actions::focus_list(st, tx);
        return true;
    }
    false
}

/// `⌘E`: focus the open snippet's content.
pub fn edit_content(s: &mut MainState, tx: &mut Tx) {
    if s.frame.selected_snippet.is_some() {
        s.blur_all_inputs();
        s.frame.focus.move_to(MainPane::Editor);
        s.editor.focus_content(tx);
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
            let name = text_input(&s.editor.name, ty.title)
                .key(NAME_KEY)
                .w_full()
                .label("Snippet name")
                .color(c.foreground)
                .placeholder("Untitled", c.muted_foreground)
                .caret(c.foreground, theme.kit().selection())
                .on_edit(|st: &mut MainState| &mut st.editor.name)
                .on_input_key(escape_to_list);
            let field = text_input(&s.editor.content, ty.input)
                .key(CONTENT_KEY)
                .w_full()
                .label("Snippet content")
                .color(c.foreground)
                .placeholder("Type snippet content…", c.muted_foreground)
                .caret(c.foreground, theme.kit().selection())
                .on_edit(|st: &mut MainState| &mut st.editor.content)
                .on_input_key(escape_to_list);
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
                .child(name)
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
