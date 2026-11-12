//! The main window: frame, navigation, shortcuts, toasts, and one module per screen
//! (`PLAN.md` S41). `shell/windows/main.rs` holds only the services, `MainState` and the window
//! spec; everything a screen does lives in that screen's module, so parallel steps edit their
//! own files:
//!
//! - Frame (S41): `mod`, `state`, `nav` (views, selection, routes, pane focus), `keys` (key
//!   routing, the action dispatcher, input guards), `frame`, `toolbar`, `toasts`, `scenes`.
//! - `sidebar/**` (S42): the view, `State`, and `sidebar/actions.rs` (groups, pause).
//! - `command_menu/` and `cheatsheet.rs` (S43).
//! - `library/**` (S50): the list, `State`, and `library/actions.rs` (snippet actions, delete
//!   with Undo, list focus).
//! - `editor/**` (S51–S54), `group_settings/**` (S55), `clipboard/**` (S58), `settings/**`
//!   (S61–S63), `insights/**` (S64), `onboarding/**` (S65), `update_toast.rs` (S63).
//!
//! Each screen's `State` is built with `Default` and answers `is_composing`, `is_editing_text`
//! and `blur_inputs` for the guards in `keys`. Scenes go in [`scenes::SCENES`], one per line.
//! Tests: `tests/mod.rs` is the rig, and each area adds its own file beside it.

pub mod cheatsheet;
pub mod clipboard;
pub mod command_menu;
pub mod editor;
pub mod frame;
pub mod group_settings;
pub mod insights;
pub mod keys;
pub mod library;
pub mod nav;
pub mod onboarding;
pub mod scenes;
pub mod settings;
pub(crate) mod shortcut_meta;
pub mod sidebar;
pub mod state;
pub mod toasts;
pub mod toolbar;
pub mod update_toast;

#[cfg(test)]
mod tests;

pub use frame::{view, CONTENT_GUTTER, SIDEBAR_W};
pub use keys::{handle_key, run};
pub use state::{
    settings_pane_label, ClipFilter, LibraryFilter, MainFrame, MainPane, MainView, NavHistory,
    LIBRARY_PANES, MAX_NAV_HISTORY, SINGLE_PANE, TWO_PANES,
};
pub use toasts::{
    ToastAction, ToastSpec, Toasts, DEFAULT_TOAST_SECS, UNDO_TOAST_SECS, UPDATE_TOAST_SECS,
};
pub use toolbar::{collapse_crumbs, compute_crumbs, Crumb, VisibleCrumb, MAX_VISIBLE_CRUMBS};
