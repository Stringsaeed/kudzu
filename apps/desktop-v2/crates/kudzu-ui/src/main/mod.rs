//! Main window frame, navigation, breadcrumbs, window shortcuts, toast host, and pre-declared
//! screen module boundaries (`PLAN.md` S41).
//!
//! Later Wave 9–11 steps fill the screen and modal modules declared below (`sidebar`, `library`,
//! `editor`, `clipboard`, `settings`, `insights`, `onboarding`, `command_menu`, `cheatsheet`,
//! `group_settings`, `update_toast`) and append scene entries to [`scenes::SCENES`] without
//! colliding on `main/mod.rs` or `MainState`.

pub mod cheatsheet;
pub mod clipboard;
pub mod command_menu;
pub mod editor;
pub mod frame;
pub mod group_settings;
pub mod insights;
pub mod keys;
pub mod library;
pub mod onboarding;
pub mod scenes;
pub mod settings;
pub mod sidebar;
pub mod state;
pub mod toasts;
pub mod toolbar;
pub mod update_toast;

#[cfg(test)]
mod tests;

pub use frame::{view, CONTENT_GUTTER, SIDEBAR_W};
pub use keys::{apply_focus_after_action, handle_key};
pub use state::{
    settings_pane_label, ClipFilter, LibraryFilter, MainFrame, MainPane, MainView, NavHistory,
    LIBRARY_PANES, MAX_NAV_HISTORY, SINGLE_PANE, TWO_PANES,
};
pub use toasts::{
    ToastAction, ToastSpec, Toasts, DEFAULT_TOAST_SECS, UNDO_TOAST_SECS, UPDATE_TOAST_SECS,
};
pub use toolbar::{collapse_crumbs, compute_crumbs, Crumb, VisibleCrumb, MAX_VISIBLE_CRUMBS};
