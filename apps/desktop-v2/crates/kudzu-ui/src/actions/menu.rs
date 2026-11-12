//! How [`super::main::MainAction`] appears in the command menu and the cheat sheet.
//!
//! The registry owns the chord. This module owns the menu's section, order and the
//! label that names the current selection or toggle. Group rows are not actions.

use kudzu_app::settings::Theme;
use kudzu_core::SnippetRecord;

use super::main::MainAction;
use super::Action;
use crate::shell::MainState;
use crate::theme::icons;

/// Tauri's command-menu groups, after the snippet hits (`command-menu.tsx`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum MenuSection {
    SelectedSnippet,
    Actions,
    GoTo,
}

impl MenuSection {
    pub fn title(self) -> &'static str {
        match self {
            MenuSection::SelectedSnippet => "Selected snippet",
            MenuSection::Actions => "Actions",
            MenuSection::GoTo => "Go to",
        }
    }
}

/// `None` for shortcuts that are not command-menu rows.
pub fn menu_section(action: MainAction) -> Option<MenuSection> {
    Some(match action {
        MainAction::ToggleFavorite
        | MainAction::ToggleEnabled
        | MainAction::DuplicateSnippet
        | MainAction::DeleteSnippet => MenuSection::SelectedSnippet,
        MainAction::NewSnippet
        | MainAction::NewGroup
        | MainAction::TogglePause
        | MainAction::ImportSnippets
        | MainAction::ExportLibrary
        | MainAction::ToggleAppearance
        | MainAction::OpenShortcuts
        | MainAction::ToggleClipboardHistory => MenuSection::Actions,
        MainAction::ViewAll
        | MainAction::ViewFavorites
        | MainAction::ViewRecent
        | MainAction::ViewInsights
        | MainAction::ViewClipboard
        | MainAction::OpenSettingsClipboard
        | MainAction::OpenSettings
        | MainAction::OpenSettingsActivity
        | MainAction::OpenSettingsApps
        | MainAction::OpenSettingsData => MenuSection::GoTo,
        MainAction::CommandMenu
        | MainAction::FocusFilter
        | MainAction::FocusList
        | MainAction::EditContent
        | MainAction::Back
        | MainAction::Forward
        | MainAction::ToggleSidebar => return None,
    })
}

/// Order inside [`menu_section`]. Group rows sort after these.
pub fn menu_order(action: MainAction) -> u16 {
    match action {
        MainAction::ToggleFavorite => 0,
        MainAction::ToggleEnabled => 1,
        MainAction::DuplicateSnippet => 2,
        MainAction::DeleteSnippet => 3,
        MainAction::NewSnippet => 0,
        MainAction::NewGroup => 1,
        MainAction::TogglePause => 2,
        MainAction::ImportSnippets => 3,
        MainAction::ExportLibrary => 4,
        MainAction::ToggleAppearance => 5,
        MainAction::OpenShortcuts => 6,
        MainAction::ToggleClipboardHistory => 7,
        MainAction::ViewAll => 0,
        MainAction::ViewFavorites => 1,
        MainAction::ViewRecent => 2,
        MainAction::ViewInsights => 3,
        MainAction::ViewClipboard => 4,
        MainAction::OpenSettingsClipboard => 5,
        MainAction::OpenSettings => 6,
        MainAction::OpenSettingsActivity => 7,
        MainAction::OpenSettingsApps => 8,
        MainAction::OpenSettingsData => 9,
        MainAction::CommandMenu
        | MainAction::FocusFilter
        | MainAction::FocusList
        | MainAction::EditContent
        | MainAction::Back
        | MainAction::Forward
        | MainAction::ToggleSidebar => 0,
    }
}

/// The row's name: the selection in curly quotes, or the live pause and theme wording.
pub fn menu_label(action: MainAction, s: &MainState) -> String {
    let settings = s.svc.settings.watch().get();
    match action {
        MainAction::ToggleFavorite => {
            let name = snippet_name(selected(s));
            if selected(s).is_some_and(|snippet| snippet.is_favorite) {
                format!("Unfavorite “{name}”")
            } else {
                format!("Favorite “{name}”")
            }
        }
        MainAction::ToggleEnabled => {
            let name = snippet_name(selected(s));
            if selected(s).is_none_or(|snippet| snippet.is_enabled) {
                format!("Turn off “{name}”")
            } else {
                format!("Turn on “{name}”")
            }
        }
        MainAction::DuplicateSnippet => format!("Duplicate “{}”", snippet_name(selected(s))),
        MainAction::DeleteSnippet => format!("Delete “{}”", snippet_name(selected(s))),
        MainAction::TogglePause => {
            if settings.paused {
                "Resume expanding".to_string()
            } else {
                "Pause expanding".to_string()
            }
        }
        MainAction::ToggleAppearance => {
            if settings.theme == Theme::Dark {
                "Switch to light appearance".to_string()
            } else {
                "Switch to dark appearance".to_string()
            }
        }
        MainAction::ToggleClipboardHistory => {
            if settings.clipboard_enabled {
                "Pause clipboard history".to_string()
            } else {
                "Resume clipboard history".to_string()
            }
        }
        other => other.label().to_string(),
    }
}

/// The chord the menu and the cheat sheet draw. `mod+comma` is shown as `mod+,`.
pub fn shown_chord(action: MainAction) -> String {
    match action {
        MainAction::OpenSettings => "mod+,".to_string(),
        MainAction::OpenShortcuts => "mod+/".to_string(),
        other => other.chord().to_string(),
    }
}

pub fn menu_icon(action: MainAction, s: &MainState) -> &'static rune::Icon {
    let settings = s.svc.settings.watch().get();
    let paused = settings.paused;
    let dark = settings.theme == Theme::Dark;
    let clipboard_on = settings.clipboard_enabled;
    match action {
        MainAction::CommandMenu | MainAction::FocusFilter | MainAction::FocusList => &icons::SEARCH,
        MainAction::NewSnippet => &icons::PLUS,
        MainAction::NewGroup => &icons::FOLDER_PLUS,
        MainAction::EditContent => &icons::PENCIL,
        MainAction::DuplicateSnippet => &icons::COPY,
        MainAction::ToggleFavorite | MainAction::ViewFavorites => &icons::STAR,
        MainAction::ToggleEnabled => &icons::CIRCLE_OFF,
        MainAction::DeleteSnippet => &icons::TRASH_2,
        MainAction::TogglePause => {
            if paused {
                &icons::PLAY
            } else {
                &icons::PAUSE
            }
        }
        MainAction::ImportSnippets => &icons::DOWNLOAD,
        MainAction::ExportLibrary => &icons::UPLOAD,
        MainAction::ToggleAppearance => {
            if dark {
                &icons::SUN
            } else {
                &icons::MOON
            }
        }
        MainAction::ToggleClipboardHistory => {
            if clipboard_on {
                &icons::PAUSE
            } else {
                &icons::PLAY
            }
        }
        MainAction::ViewAll => &icons::LAYERS,
        MainAction::ViewRecent => &icons::CLOCK_3,
        MainAction::ViewInsights => &icons::BAR_CHART_3,
        MainAction::ViewClipboard => &icons::CLIPBOARD_LIST,
        MainAction::OpenSettings
        | MainAction::OpenSettingsClipboard
        | MainAction::OpenSettingsApps
        | MainAction::OpenSettingsData => &icons::SETTINGS,
        MainAction::OpenSettingsActivity => &icons::CIRCLE_HELP,
        MainAction::OpenShortcuts => &icons::KEYBOARD,
        MainAction::Back => &icons::ARROW_LEFT,
        MainAction::Forward => &icons::ARROW_RIGHT,
        MainAction::ToggleSidebar => &icons::PANEL_LEFT,
    }
}

fn selected(s: &MainState) -> Option<&SnippetRecord> {
    let id = s.frame.selected_snippet?;
    s.library_snapshot()
        .snippets
        .iter()
        .find(|snippet| snippet.id == id)
}

fn snippet_name(snippet: Option<&SnippetRecord>) -> String {
    let Some(snippet) = snippet else {
        return "Untitled".to_string();
    };
    if !snippet.name.is_empty() {
        return snippet.name.clone();
    }
    snippet
        .triggers
        .iter()
        .find(|trigger| !trigger.is_empty())
        .cloned()
        .unwrap_or_else(|| "Untitled".to_string())
}
