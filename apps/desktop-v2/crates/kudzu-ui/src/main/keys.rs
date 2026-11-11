//! Main window keyboard routing (`LIB-14`, `PLAN.md` S41).
//!
//! Enforces IME composition blocking (`TextEdit::is_composing`), modal Escape dismissal and
//! shortcut suppression, editor-to-list Escape focus return, editable-input shortcut filtering, and
//! [`crate::actions::Action`] dispatch for [`MainAction`].

use rune::{Key, KeyEvent, Keybind, Tx};

use crate::actions::main::{ActionContext, MainAction};
use crate::actions::Action;
use crate::main::clipboard;
use crate::main::library;
use crate::main::state::{MainPane, MainView};
use crate::shell::MainState;

/// Window-level `on_key` handler wired into `AppSpec::on_key` for `main`.
pub fn handle_key(s: &mut MainState, ev: &KeyEvent, tx: &mut Tx) -> bool {
    s.frame.clock_secs = tx.clock();

    // 1. IME guard (`use-hotkeys.ts:19`): never fire window shortcuts mid-composition.
    if s.is_composing() {
        return false;
    }

    // 2. Modal guard: open dialogs/menus own Escape and suppress underlying window shortcuts.
    if s.frame.any_modal_open() {
        if ev.key == Key::Escape && !ev.mods.cmd && !ev.mods.ctrl && !ev.mods.alt {
            return s.frame.close_top_modal();
        }
        if s.frame.command_open
            && !s.frame.shortcuts_open
            && s.frame.group_settings_id.is_none()
            && !s.frame.breadcrumb_overflow_open
            && Keybind::new("mod+k").matches(ev)
        {
            s.frame.command_open = false;
            return true;
        }
        if s.frame.shortcuts_open
            && s.frame.group_settings_id.is_none()
            && !s.frame.breadcrumb_overflow_open
            && Keybind::new("mod+/").matches(ev)
        {
            s.frame.shortcuts_open = false;
            return true;
        }
        return false;
    }

    // 3. Escape inside the editor returns focus to the snippet list (`main-app.tsx:146-156`).
    if ev.key == Key::Escape
        && !ev.mods.cmd
        && !ev.mods.ctrl
        && !ev.mods.alt
        && !ev.mods.shift
        && s.frame.view.in_library()
        && (s.editor.is_editing_text() || s.frame.focus.is_active(MainPane::Editor))
    {
        s.blur_all_inputs();
        tx.blur();
        s.frame.focus.move_to(MainPane::List);
        tx.focus(library::LIST_KEY, false);
        return true;
    }

    // 4. Resolve against `MainAction` with full context guards.
    let ctx = ActionContext {
        area: s.frame.view.area(),
        has_selected_snippet: s.frame.selected_snippet.is_some(),
        can_back: s.frame.history.can_go_back(),
        can_forward: s.frame.history.can_go_forward(),
        in_text_input: s.is_editing_text(),
        modal_open: false,
        composing: false,
    };

    let Some(action) = MainAction::resolve_in_context(ev, ctx) else {
        return false;
    };

    action.run(s, tx);
    true
}

/// Applies Rune `Tx` focus transitions after an action mutates `MainState`.
pub fn apply_focus_after_action(s: &mut MainState, action: MainAction, tx: &mut Tx) {
    match action {
        MainAction::FocusFilter => match s.frame.view {
            MainView::Clipboard(_) => {
                s.blur_all_inputs();
                s.frame.focus.move_to(MainPane::List);
                s.clipboard.focus_search(tx);
            }
            _ => {
                s.blur_all_inputs();
                s.frame.focus.move_to(MainPane::List);
                s.library.focus_filter(tx);
            }
        },
        MainAction::FocusList => {
            s.blur_all_inputs();
            tx.blur();
            s.frame.focus.move_to(MainPane::List);
            match s.frame.view {
                MainView::Clipboard(_) => tx.focus(clipboard::LIST_KEY, false),
                _ => tx.focus(library::LIST_KEY, false),
            }
        }
        MainAction::EditContent | MainAction::NewSnippet => {
            if s.frame.selected_snippet.is_some() {
                s.blur_all_inputs();
                s.frame.focus.move_to(MainPane::Editor);
                s.editor.focus_content(tx);
            }
        }
        MainAction::ViewAll
        | MainAction::ViewFavorites
        | MainAction::ViewRecent
        | MainAction::ViewInsights
        | MainAction::ViewClipboard
        | MainAction::OpenSettings
        | MainAction::Back
        | MainAction::Forward => {
            s.blur_all_inputs();
            tx.blur();
        }
        _ => {}
    }

    if let Some(due) = s.frame.toasts.next_due() {
        tx.wake_at(due);
    }
}
