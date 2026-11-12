//! Main window keyboard routing (`LIB-14`, `PLAN.md` S41).
//!
//! Enforces IME composition blocking (`TextEdit::is_composing`), modal Escape dismissal and
//! shortcut suppression, editor-to-list Escape focus return, editable-input shortcut filtering, and
//! [`crate::actions::Action`] dispatch for [`MainAction`], and the input guards every screen's
//! `State` feeds (`is_composing`, `is_editing_text`, `blur_inputs`).

use rune::{Key, KeyEvent, Keybind, Tx};

use crate::actions::main::{ActionContext, MainAction};
use crate::actions::Action;
use crate::main::state::{ClipFilter, LibraryFilter, MainPane, MainView};
use crate::main::{
    cheatsheet, clipboard, command_menu, editor, group_settings, library, nav, sidebar,
};
use crate::shell::{MainState, NewSnippet, SettingsPane};

/// Window-level `on_key` handler wired into `AppSpec::on_key` for `main`.
pub fn handle_key(s: &mut MainState, ev: &KeyEvent, tx: &mut Tx) -> bool {
    s.frame.clock_secs = tx.clock();

    // 1. IME guard (`use-hotkeys.ts:19`): never fire window shortcuts mid-composition.
    if s.is_composing() {
        return false;
    }

    // 2. Modal guard: open dialogs/menus own Escape and suppress underlying window shortcuts.
    if s.frame.any_modal_open() {
        let command_top = s.frame.command_open
            && !s.frame.shortcuts_open
            && s.frame.group_settings_id.is_none()
            && !s.frame.breadcrumb_overflow_open;
        if ev.key == Key::Escape && !ev.mods.cmd && !ev.mods.ctrl && !ev.mods.alt {
            return s.dismiss_top_overlay();
        }
        if command_top && Keybind::new("mod+k").matches(ev) {
            command_menu::close(s);
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
        if command_top {
            return command_menu::on_key(s, ev, tx);
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
        library::actions::focus_list(s, tx);
        return true;
    }

    // 3b. The clipboard screen's keys come before the frame's, so its ⌘⌫ deletes clips.
    if matches!(s.frame.view, MainView::Clipboard(_)) && clipboard::handle_key(s, ev, tx) {
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

/// Runs a [`MainAction`] from a shortcut or the command menu (`LIB-14`). Each arm calls the
/// module that owns the action, which also moves focus.
pub fn run(s: &mut MainState, action: MainAction, tx: &mut Tx) {
    s.frame.clock_secs = tx.clock();
    match action {
        MainAction::CommandMenu => command_menu::open(s, tx),
        MainAction::NewSnippet => {
            let _ = library::actions::new_snippet(s, NewSnippet::default());
        }
        MainAction::NewGroup => sidebar::actions::new_group(s),
        MainAction::FocusFilter => match s.frame.view {
            MainView::Clipboard(_) => clipboard::focus_search(s, tx),
            _ => library::actions::focus_filter(s, tx),
        },
        MainAction::FocusList => match s.frame.view {
            MainView::Clipboard(_) => clipboard::focus_list(s, tx),
            _ => library::actions::focus_list(s, tx),
        },
        MainAction::EditContent => editor::edit_content(s, tx),
        MainAction::DuplicateSnippet => library::actions::duplicate_selected(s),
        MainAction::ToggleFavorite => library::actions::toggle_favorite(s),
        MainAction::ToggleEnabled => library::actions::toggle_enabled(s),
        MainAction::DeleteSnippet => library::actions::delete_selected(s, tx),
        MainAction::TogglePause => sidebar::actions::toggle_pause(s),
        MainAction::ViewAll => nav::view(s, MainView::Library(LibraryFilter::All), tx),
        MainAction::ViewFavorites => {
            nav::view(s, MainView::Library(LibraryFilter::Favorites), tx);
        }
        MainAction::ViewRecent => nav::view(s, MainView::Library(LibraryFilter::Recent), tx),
        MainAction::ViewInsights => nav::view(s, MainView::Insights, tx),
        MainAction::ViewClipboard => nav::view(s, MainView::Clipboard(ClipFilter::All), tx),
        MainAction::OpenSettings => nav::open_settings(s, tx),
        MainAction::OpenShortcuts => cheatsheet::open(s),
        MainAction::Back => nav::back(s, tx),
        MainAction::Forward => nav::forward(s, tx),
        MainAction::ToggleSidebar => s.toggle_sidebar(),
        MainAction::ImportSnippets => command_menu::import_snippets(s, tx),
        MainAction::ExportLibrary => command_menu::export_library(s, tx),
        MainAction::ToggleAppearance => command_menu::toggle_appearance(s),
        MainAction::ToggleClipboardHistory => command_menu::toggle_clipboard(s),
        MainAction::OpenSettingsClipboard => {
            nav::view(s, MainView::Settings(SettingsPane::Clipboard), tx);
        }
        MainAction::OpenSettingsActivity => {
            nav::view(s, MainView::Settings(SettingsPane::Activity), tx);
        }
        MainAction::OpenSettingsApps => nav::view(s, MainView::Settings(SettingsPane::Apps), tx),
        MainAction::OpenSettingsData => nav::view(s, MainView::Settings(SettingsPane::Data), tx),
    }
    if let Some(due) = s.frame.toasts.next_due() {
        tx.wake_at(due);
    }
}

impl MainState {
    /// True while any screen's text field has an IME composition open.
    pub fn is_composing(&self) -> bool {
        self.sidebar.is_composing()
            || self.library.is_composing()
            || self.editor.is_composing()
            || self.clipboard.is_composing()
            || self.settings.is_composing()
            || self.insights.is_composing()
            || self.onboarding.is_composing()
            || self.command_menu.query.is_composing()
            || self.group_settings.is_composing()
    }

    /// True while any screen's editable text field holds keyboard focus.
    pub fn is_editing_text(&self) -> bool {
        self.sidebar.is_editing_text()
            || self.library.is_editing_text()
            || self.editor.is_editing_text()
            || self.clipboard.is_editing_text()
            || self.settings.is_editing_text()
            || self.insights.is_editing_text()
            || self.onboarding.is_editing_text()
            || self.group_settings.is_editing_text()
    }

    /// Dismisses the overlay painted last. The group-settings app picker, then
    /// group settings, then the cheat sheet, then the command menu (which clears
    /// its query), then the breadcrumb menu.
    pub fn dismiss_top_overlay(&mut self) -> bool {
        if self.group_settings.picker_open() {
            group_settings::close_picker(self);
            return true;
        }
        if self.frame.group_settings_id.is_some() {
            self.frame.group_settings_id = None;
            return true;
        }
        if self.frame.shortcuts_open {
            self.frame.shortcuts_open = false;
            return true;
        }
        if self.frame.command_open {
            command_menu::close(self);
            return true;
        }
        if self.frame.breadcrumb_overflow_open {
            self.frame.breadcrumb_overflow_open = false;
            return true;
        }
        false
    }

    /// Clears every screen's text-field focus flag before pane focus moves.
    pub fn blur_all_inputs(&mut self) {
        self.sidebar.blur_inputs();
        self.library.blur_inputs();
        self.editor.blur_inputs();
        self.clipboard.blur_inputs();
        self.settings.blur_inputs();
        self.insights.blur_inputs();
        self.onboarding.blur_inputs();
        self.command_menu.query.focused = false;
        self.group_settings.blur_inputs();
    }
}
