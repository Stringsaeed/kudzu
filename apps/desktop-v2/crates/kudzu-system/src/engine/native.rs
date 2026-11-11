//! The native engine's command surface, as the adapter uses it. A seam only so the adapter's
//! forwarding can be tested without a live event tap; `System` is the one real implementation.
use kudzu_core::system::{AppInfo, EngineStatus};
use kudzu_engine::{Engine, EngineLibrary, KudzuWindow, PanelFocus, Submission};
use std::{collections::HashMap, sync::Arc};
use uuid::Uuid;

pub(super) trait Native: Send + Sync {
    fn start(&self);
    fn stop(&self);
    fn reload(&self, library: EngineLibrary);
    fn status(&self) -> EngineStatus;
    fn set_paused(&self, paused: bool);
    fn set_suppressed(&self, suppressed: bool);
    fn insert(&self, snippet: Uuid, target: Option<AppInfo>, selection: Option<String>);
    /// Whether the native engine accepted the token (it is that engine's pending form).
    fn submit_form(
        &self,
        token: u64,
        values: HashMap<String, String>,
        submission: Submission,
    ) -> bool;
    fn cancel_form(&self, token: u64);
    fn form_panel_focused(&self, token: u64, focus: PanelFocus);
    fn set_kudzu_windows(&self, windows: &[KudzuWindow]);
}

pub(super) struct System(pub Arc<Engine>);

impl Native for System {
    fn start(&self) {
        self.0.start();
    }
    fn stop(&self) {
        self.0.stop();
    }
    fn reload(&self, library: EngineLibrary) {
        self.0.reload(library);
    }
    fn status(&self) -> EngineStatus {
        self.0.status()
    }
    fn set_paused(&self, paused: bool) {
        self.0.set_paused(paused);
    }
    fn set_suppressed(&self, suppressed: bool) {
        self.0.set_suppressed(suppressed);
    }
    fn insert(&self, snippet: Uuid, target: Option<AppInfo>, selection: Option<String>) {
        self.0
            .insert_snippet_with_selection(snippet, target, selection);
    }
    fn submit_form(
        &self,
        token: u64,
        values: HashMap<String, String>,
        submission: Submission,
    ) -> bool {
        self.0.submit_form(token, values, submission)
    }
    fn cancel_form(&self, token: u64) {
        self.0.cancel_form(token);
    }
    fn form_panel_focused(&self, token: u64, focus: PanelFocus) {
        self.0.form_panel_focused(token, focus);
    }
    fn set_kudzu_windows(&self, windows: &[KudzuWindow]) {
        self.0.set_kudzu_windows(windows);
    }
}
