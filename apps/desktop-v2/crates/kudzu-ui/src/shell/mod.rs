//! Window coordination: every window's `AppHandle`, and the moves that cross windows.
//! Windows get services from their own bundles (`windows`); `Shell` holds only what
//! coordination needs. Nothing here is global: `compose` builds one `Shell` per process.
mod hotkeys;
pub(crate) mod lifecycle;
mod placeholder;
pub mod routes;
mod settle;
pub mod theme_sync;
pub mod tray;
mod window_list;
pub mod windows;

pub(crate) use lifecycle::closes_windows;
pub use routes::{NewSnippet, Route, SettingsPane};
pub use windows::{build, Launch, MainState, Windows};

use kudzu_app::assembly::{SelectionCapture, Shutdown};
use kudzu_app::expansion::ExpansionService;
use kudzu_app::forms::FormService;
use kudzu_app::updates::{Trigger, UpdateState, Updates};
use rune::AppHandle;
use std::sync::Arc;
use windows::{FormState, PaletteState, PermissionsState, PickerState};

/// The five windows' handles. `Clone + Send + Sync`, like `AppHandle`.
#[derive(Clone)]
pub(crate) struct Handles {
    pub main: AppHandle<MainState>,
    pub palette: AppHandle<PaletteState>,
    pub picker: AppHandle<PickerState>,
    pub form: AppHandle<FormState>,
    pub permissions: AppHandle<PermissionsState>,
}

/// Who the app is, for labels. The resolved identity, never read from the bundle here.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Identity {
    pub app_name: &'static str,
    pub dev: bool,
}

struct Inner {
    handles: Handles,
    identity: Identity,
    /// The real app: native menus, status item and `NSApp.appearance`. False when headless.
    native: bool,
    forms: Arc<FormService>,
    expansion: Arc<ExpansionService>,
    updates: Updates,
    selection: Arc<SelectionCapture>,
    shutdown: Arc<dyn Shutdown>,
    windows: window_list::WindowList,
    registrar: Arc<dyn kudzu_app::ports::HotkeyRegistrar>,
    settle: settle::Settle,
    /// Watch subscriptions that wake windows; dropped with the last window state.
    wakers: std::sync::Mutex<Vec<kudzu_app::watch::Subscription>>,
    /// `main` reported focus. Escape then hides Quick Search without activating the target.
    main_focused: std::sync::atomic::AtomicBool,
}

/// Cheap to clone; every window state holds one.
#[derive(Clone)]
pub struct Shell(Arc<Inner>);

impl Shell {
    pub fn identity(&self) -> Identity {
        self.0.identity
    }

    pub(crate) fn native(&self) -> bool {
        self.0.native
    }

    /// The main window's last focus report. Quick Search reads it when it hides.
    pub(crate) fn set_main_focused(&self, focused: bool) {
        self.0
            .main_focused
            .store(focused, std::sync::atomic::Ordering::Relaxed);
    }

    pub(crate) fn main_focused(&self) -> bool {
        self.0
            .main_focused
            .load(std::sync::atomic::Ordering::Relaxed)
    }

    pub(crate) fn handles(&self) -> &Handles {
        &self.0.handles
    }

    /// Show the main window at `route`, with the Dock icon when the user wants it.
    pub fn open_main(&self, route: Route) {
        self.0.handles.main.update(move |s, _| {
            s.open_route(route);
            s.show();
        });
    }

    /// Show `main` where it was (the tray's Open, a Dock click).
    pub fn show_main(&self) {
        self.0.handles.main.update(|s, _| s.show());
    }

    pub fn toggle_palette(&self) {
        self.0.handles.palette.update(|s, _| s.toggle());
    }

    pub fn toggle_picker(&self) {
        self.0.handles.picker.update(|s, _| s.toggle());
    }

    pub fn open_permissions(&self) {
        self.0.handles.permissions.update(|s, _| s.show());
    }

    /// Open the permission companion with `kind` pre-selected.
    pub fn open_permissions_for(&self, kind: kudzu_app::ports::PermissionKind) {
        self.0
            .handles
            .permissions
            .update(move |s, _| s.show_for(kind));
    }

    /// ⌘Q and the app menu's Close Windows: cancel a pending form (lifting its suppression),
    /// dismiss every panel, commit drafts and hide `main`. The app keeps running.
    pub fn close_windows(&self) {
        match self.0.forms.close_windows() {
            Ok(Some(task)) => self.settle("form cancel", task),
            Ok(None) => {}
            Err(error) => log::warn!("cancelling the pending form failed to start: {error}"),
        }
        let handles = &self.0.handles;
        handles.palette.update(|s, _| s.dismiss(false));
        handles.picker.update(|s, _| s.dismiss());
        handles.form.update(|s, _| s.hide());
        handles.permissions.update(|s, _| s.hide());
        handles.main.update(|s, _| s.hide());
    }

    /// The tray's Quit: one update on `main` commits drafts, shuts down, then quits.
    pub fn quit(&self) {
        self.0.handles.main.update(|s, tx| {
            s.quit();
            tx.quit();
        });
    }

    /// Runs `Kudzu::shutdown` once, however quit starts.
    pub(crate) fn shutdown(&self) {
        let outcome = self.0.shutdown.shutdown();
        if let Err(error) = outcome.durable {
            log::error!("shutdown could not save everything: {error}");
        }
    }

    pub fn toggle_pause(&self) {
        if let Err(error) = self.0.expansion.toggle_paused() {
            log::warn!("pausing failed: {error}");
        }
    }

    /// Captures the selection on a worker, then opens a new snippet, empty or not.
    pub fn new_snippet_from_selection(&self) {
        self.0.handles.main.update(|s, _| s.begin_capture());
    }

    pub(crate) fn selection(&self) -> &SelectionCapture {
        &self.0.selection
    }

    /// The tray's update row: restart into a ready update, else show About and check.
    pub fn update_row(&self) {
        let state = self.0.updates.state().get();
        match *state {
            UpdateState::Ready { .. } => {
                let task = self.0.updates.restart_to_update();
                self.0.settle.add_value("restart to update", task);
            }
            UpdateState::Downloading { .. } | UpdateState::Checking => {
                self.open_main(Route::Settings(SettingsPane::About));
            }
            _ => {
                self.open_main(Route::Settings(SettingsPane::About));
                let task = self.0.updates.check(Trigger::User);
                self.0.settle.add_value("update check", task);
                self.0.handles.main.wake();
            }
        }
    }

    pub(crate) fn registrar(&self) -> &dyn kudzu_app::ports::HotkeyRegistrar {
        &*self.0.registrar
    }

    /// Push Kudzu's live windows to the engine (FRM-4). Call on every open, close or change.
    pub(crate) fn push_windows(&self) {
        self.0.windows.push();
    }

    pub(crate) fn track_window(&self, handle: &rune::WindowHandle) {
        self.0.windows.track(handle);
    }

    pub(crate) fn window(&self, key: &str) -> Option<rune::WindowHandle> {
        self.0.windows.window(key)
    }

    pub(crate) fn set_guide_window(&self, number: Option<u32>) {
        self.0.windows.set_guide(number);
    }

    /// Keep a started task until it finishes, then log a failure. Drained by `main`'s poll.
    pub(crate) fn settle<E: std::fmt::Display + Send + 'static>(
        &self,
        what: &'static str,
        task: kudzu_app::tasks::Task<Result<(), E>>,
    ) {
        self.0.settle.add(what, task);
        self.0.handles.main.wake();
    }

    /// Keep a started task whose value nobody reads (a form submission).
    pub(crate) fn settle_value<T: Send + 'static>(
        &self,
        what: &'static str,
        task: kudzu_app::tasks::Task<T>,
    ) {
        self.0.settle.add_value(what, task);
        self.0.handles.main.wake();
    }

    pub(crate) fn drain_settled(&self) {
        self.0.settle.drain();
    }

    #[cfg(test)]
    pub(crate) fn settling(&self) -> usize {
        self.0.settle.len()
    }
}

#[cfg(test)]
mod tests;
