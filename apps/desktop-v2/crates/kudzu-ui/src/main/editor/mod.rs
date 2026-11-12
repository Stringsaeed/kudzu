//! The snippet editor (`PLAN.md` S51–S54; `snippet-editor.tsx`, `trigger-input.tsx`,
//! `behavior.tsx`). S51 is the shell; the content field is one module, [`content`], which S52
//! replaces with the chip editor.
//!
//! - `model`: `State`, the draft [`model::Session`] (an outside change is taken only while the
//!   draft is clean) and timed motion.
//! - `autosave`: the 350 ms debounce, flushes, and "Saved"/"Couldn't save" from
//!   `library.saved()`.
//! - `header`: toolbar, menus, favorite burst. `triggers`: the chip field. `behavior`: rules.
//! - `preview`: the debounced render and its badges. `view`: the pane. `scenes`: fixtures.
//!
//! The window calls [`open_selected`] when the selection changes (`MainState::select_snippet`),
//! [`sync`] from `MainState::sync`, [`poll`] from the window's poll, and [`flush`] from every
//! path that hides `main` or quits (`MainState::commit_drafts`) and when it loses focus.

pub mod autosave;
pub mod behavior;
pub mod content;
pub mod header;
pub mod model;
pub mod preview;
pub mod scenes;
pub mod triggers;
pub mod view;

use rune::{col, Cx, El, Key, KeyEvent, Tx};

use crate::main::library;
use crate::main::shortcut_meta::LocalShortcut;
use crate::main::state::MainPane;
use crate::shell::MainState;
use crate::theme::Theme;

pub use autosave::{flush, SaveStatus, AUTOSAVE_DELAY};
pub use model::{Session, State};

/// `/` inserts a variable once the editor content exists. Listed beside that field.
pub(crate) const INSERT_VARIABLE: LocalShortcut = LocalShortcut {
    id: "editor-variable",
    chord: "/",
    label: "Insert a variable",
};

/// Undo and save stay with the editor. The content field does not run Undo yet.
pub(crate) const EDITOR_SHORTCUTS: &[LocalShortcut] = &[
    LocalShortcut {
        id: "editor-undo",
        chord: "mod+z",
        label: "Undo",
    },
    LocalShortcut {
        id: "editor-save",
        chord: "mod+s",
        label: "Save now (auto-saves anyway)",
    },
];

/// Escape from the editor field, next to [`escape_to_list`].
pub(crate) const BACK_TO_LIST: LocalShortcut = LocalShortcut {
    id: "editor-back",
    chord: "escape",
    label: "Back to the list",
};

pub const CONTENT_KEY: &str = "editor-content";
pub const NAME_KEY: &str = "editor-name";
pub const TRIGGER_KEY: &str = "editor-trigger";
pub const PANE_KEY: &str = "editor-pane-slot";

/// Escape in an editor field returns to the list (`main-app.tsx:146-156`).
pub(crate) fn escape_to_list(st: &mut MainState, ev: &KeyEvent, tx: &mut Tx) -> bool {
    let bare = !ev.mods.cmd && !ev.mods.ctrl && !ev.mods.alt && !ev.mods.shift;
    if ev.key == Key::Escape && bare && !st.is_composing() && !st.frame.any_modal_open() {
        library::actions::focus_list(st, tx);
        return true;
    }
    false
}

/// ⌘S, exactly.
fn is_save(ev: &KeyEvent) -> bool {
    ev.is_command('s') && !ev.mods.shift && !ev.mods.alt && !ev.mods.ctrl
}

/// Keys every editor text field shares, from its `on_input_key` (Rune gives a focused field's
/// keys to the field, not to the pane): ⌘S writes now; Escape returns to the list. Nothing
/// happens mid-composition.
pub(crate) fn field_key(s: &mut MainState, ev: &KeyEvent, tx: &mut Tx) -> bool {
    if s.is_composing() {
        return false;
    }
    if is_save(ev) {
        flush(s);
        return true;
    }
    escape_to_list(s, ev, tx)
}

/// `⌘E`: focus the open snippet's content.
pub fn edit_content(s: &mut MainState, tx: &mut Tx) {
    if s.frame.selected_snippet.is_some() {
        s.blur_all_inputs();
        s.frame.focus.move_to(MainPane::Editor);
        s.editor.focus_content(tx);
    }
}

/// `main` gained or lost key focus: losing it writes the draft (Tauri's `window` blur).
pub fn window_focus(s: &mut MainState, focused: bool) {
    if !focused {
        flush(s);
    }
}

/// Shows the selected snippet, writing the one that was open first. Nothing shows outside the
/// library (Tauri unmounts the editor there).
pub fn open_selected(s: &mut MainState) -> bool {
    let want = s
        .frame
        .selected_snippet
        .filter(|_| s.frame.view.in_library());
    if s.editor.open_id() == want {
        return false;
    }
    if s.editor.session.is_some() {
        flush(s);
    }
    let record = want.and_then(|id| {
        s.model
            .library
            .snippets
            .iter()
            .find(|sn| sn.id == id)
            .cloned()
    });
    match record {
        Some(record) => {
            s.editor.load(&record);
            s.editor.preview.opened(record.id, s.frame.clock_secs);
        }
        None => s.editor.close(),
    }
    true
}

/// Applies an edit to the draft: autosave restarts, the preview follows content edits.
pub(crate) fn edit_draft(s: &mut MainState, edit: impl FnOnce(&mut kudzu_core::SnippetRecord)) {
    let now = s.frame.clock_secs;
    let Some(session) = s.editor.session.as_mut() else {
        return;
    };
    let before = (
        session.draft.content.clone(),
        session.draft.formatting.clone(),
    );
    if !session.edit(edit) {
        return;
    }
    let content_changed = before
        != (
            session.draft.content.clone(),
            session.draft.formatting.clone(),
        );
    let draft = session.draft.clone();
    s.editor.autosave.edited(now);
    if content_changed {
        s.editor.preview.edited(now);
    }
    s.editor.header.observe(&draft, now);
    s.editor.rules.observe(&draft);
}

/// The name field changed.
pub(crate) fn name_changed(s: &mut MainState, _: &mut Tx) {
    let name = s.editor.name.text.clone();
    edit_draft(s, |d| d.name = name);
}

/// The content field changed (called by [`content`]).
pub(crate) fn content_changed(s: &mut MainState, _: &mut Tx) {
    let source = s.editor.content.source().to_owned();
    edit_draft(s, |d| d.content = source);
}

/// Copies the fields into the draft if an edit reached them without a change callback.
pub(crate) fn pull_fields(s: &mut MainState) -> bool {
    let Some(d) = s.editor.draft() else {
        return false;
    };
    let (name, source) = (
        s.editor.name.text.clone(),
        s.editor.content.source().to_owned(),
    );
    if d.name == name && d.content == source {
        return false;
    }
    edit_draft(s, |d| {
        d.name = name;
        d.content = source;
    });
    true
}

/// Watches the editor reads besides the library: saves and conflicts.
fn subscribe(s: &mut MainState) {
    if !s.editor.subscriptions.is_empty() {
        return;
    }
    let (a, b) = (s.editor.wake.clone(), s.editor.wake.clone());
    s.editor.subscriptions = vec![
        s.svc.library.saved().subscribe(move || a()),
        s.svc.library.conflicts().subscribe(move || b()),
    ];
}

/// One pull: the selection, an outside change, motion that follows the draft, app icons.
pub fn sync(s: &mut MainState) -> bool {
    subscribe(s);
    let mut changed = open_selected(s);
    let now = s.frame.clock_secs;
    if let Some(id) = s.editor.open_id() {
        let live = s
            .model
            .library
            .snippets
            .iter()
            .find(|sn| sn.id == id)
            .cloned();
        if let (Some(live), Some(session)) = (live, s.editor.session.as_mut()) {
            let content = session.draft.content.clone();
            if session.outside(&live) == model::Outside::Taken {
                s.editor.reload(&live);
                if live.content != content {
                    s.editor.preview.edited(now);
                }
                changed = true;
            }
        }
    }
    if let Some(d) = s.editor.draft().cloned() {
        changed |= s.editor.header.observe(&d, now);
        changed |= s.editor.rules.observe(&d);
    }
    behavior::request_icons(s);
    changed |= s.editor.rules.icons.poll();
    changed
}

/// Timers and tasks: autosave, the preview, the trigger field's focus and motion, the star and
/// the copy. Every motion asks for a frame at its deadline and ends there.
pub fn poll(s: &mut MainState, tx: &mut Tx) -> bool {
    let now = s.frame.clock_secs;
    let reduced = s.editor.reduced.get();
    let mut changed = triggers::sync_focus(s, tx, reduced);
    changed |= autosave::poll(s);
    changed |= preview::poll(s);
    changed |= s.editor.trigger.poll(now, tx);
    let (star, copied) = s.editor.header.poll(now, tx);
    changed |= star;
    if let Some(result) = copied {
        header::copied(s, result);
        changed = true;
    }
    for due in [
        s.editor.autosave.due(),
        s.editor.preview.due(),
        s.editor.preview.cue.deadline(),
    ]
    .into_iter()
    .flatten()
    {
        tx.wake_at(due);
    }
    changed
}

/// The editor pane beside the list. ⌘S writes now from anywhere inside it.
pub fn view(
    s: &MainState,
    cx: &Cx<MainState>,
    theme: &Theme,
    pane_focused: bool,
    overlays: &mut Vec<El>,
) -> El {
    s.editor.reduced.set(cx.is_reduced_motion());
    let mut pane = col()
        .key(PANE_KEY)
        .grow(1.0)
        .min_w(0.0)
        .h_full()
        .min_h(0.0)
        // A focused button or switch inside the pane; text fields use `field_key`.
        .on_key(|st: &mut MainState, ev: &KeyEvent, _| {
            if is_save(ev) && !st.is_composing() {
                flush(st);
                return true;
            }
            false
        })
        .child(view::view(s, cx, theme, overlays));
    if pane_focused {
        pane = pane.focus_selected(true);
    }
    pane
}
