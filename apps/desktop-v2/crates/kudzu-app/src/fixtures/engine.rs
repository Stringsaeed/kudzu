//! Controllable engine and callback collector. All callbacks run outside fake locks.
use crate::{
    error::PortError,
    ports::{EngineEvents, EngineLibrary, Expansion, ExpansionEngine, FormRequest},
};
use kudzu_core::{
    activity::ActivityEvent,
    system::{AppInfo, EngineStatus, PermissionStatus, TapMode},
};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};
use uuid::Uuid;
#[derive(Clone)]
pub struct InsertRequest {
    pub snippet: Uuid,
    pub target: Option<AppInfo>,
    pub selection: Option<String>,
}
pub struct EngineState {
    pub running: bool,
    pub status: EngineStatus,
    pub suppressed: bool,
    pub library: EngineLibrary,
    pub reloads: usize,
    pub insertions: Vec<InsertRequest>,
    pub submissions: Vec<(u64, HashMap<String, String>)>,
    pub cancelled: Vec<u64>,
    pub focus: Vec<(u64, bool)>,
    pub failure: Option<PortError>,
}
impl Default for EngineState {
    fn default() -> Self {
        Self {
            running: false,
            status: EngineStatus {
                can_post_events: false,
                can_expand: false,
                paused: false,
                tap: TapMode::Off,
                permissions: PermissionStatus::default(),
                secure_input: false,
                secure_input_owner: None,
                summary: "Fixture engine stopped".into(),
            },
            suppressed: false,
            library: EngineLibrary::default(),
            reloads: 0,
            insertions: Vec::new(),
            submissions: Vec::new(),
            cancelled: Vec::new(),
            focus: Vec::new(),
            failure: None,
        }
    }
}
#[derive(Default)]
pub struct FakeEngine {
    pub state: Mutex<EngineState>,
    events: Mutex<Option<Arc<dyn EngineEvents>>>,
}
impl FakeEngine {
    pub fn events(&self) -> Option<Arc<dyn EngineEvents>> {
        self.events.lock().unwrap().clone()
    }
    pub fn emit_form(&self, request: FormRequest) {
        if let Some(events) = self.events() {
            events.form_requested(request);
        }
    }
    pub fn emit_expansion(&self, expansion: Expansion) {
        if let Some(events) = self.events() {
            events.expanded(expansion);
        }
    }
    pub fn emit_status(&self, status: EngineStatus) {
        self.state.lock().unwrap().status = status.clone();
        if let Some(events) = self.events() {
            events.status_changed(status);
        }
    }
}
impl ExpansionEngine for FakeEngine {
    fn start(&self, events: Arc<dyn EngineEvents>) -> Result<(), PortError> {
        let mut state = self.state.lock().unwrap();
        if let Some(e) = state.failure {
            return Err(e);
        }
        state.running = true;
        *self.events.lock().unwrap() = Some(events);
        Ok(())
    }
    fn stop(&self) {
        self.state.lock().unwrap().running = false;
        self.events.lock().unwrap().take();
    }
    fn reload(&self, library: EngineLibrary) {
        let mut state = self.state.lock().unwrap();
        state.library = library;
        state.reloads += 1;
    }
    fn status(&self) -> EngineStatus {
        self.state.lock().unwrap().status.clone()
    }
    fn set_paused(&self, paused: bool) {
        let mut status = self.status();
        status.paused = paused;
        status.can_expand =
            !paused && status.can_post_events && !status.secure_input && status.tap != TapMode::Off;
        self.emit_status(status);
    }
    fn set_suppressed(&self, suppressed: bool) {
        self.state.lock().unwrap().suppressed = suppressed;
    }
    fn insert(
        &self,
        snippet: Uuid,
        target: Option<AppInfo>,
        selection: Option<String>,
    ) -> Result<(), PortError> {
        let mut state = self.state.lock().unwrap();
        if let Some(e) = state.failure {
            return Err(e);
        }
        state.insertions.push(InsertRequest {
            snippet,
            target,
            selection,
        });
        Ok(())
    }
    fn submit_form(&self, token: u64, values: HashMap<String, String>) -> Result<(), PortError> {
        let mut state = self.state.lock().unwrap();
        if let Some(e) = state.failure {
            return Err(e);
        }
        state.submissions.push((token, values));
        Ok(())
    }
    fn cancel_form(&self, token: u64) {
        self.state.lock().unwrap().cancelled.push(token);
    }
    fn panel_focused(&self, token: u64, focused: bool) {
        self.state.lock().unwrap().focus.push((token, focused));
    }
}
#[derive(Default)]
pub struct FakeEngineEvents {
    pub statuses: Mutex<Vec<EngineStatus>>,
    pub expansions: Mutex<Vec<Expansion>>,
    pub forms: Mutex<Vec<FormRequest>>,
    pub activity: Mutex<Vec<ActivityEvent>>,
    pub history: Mutex<Vec<String>>,
}
impl EngineEvents for FakeEngineEvents {
    fn status_changed(&self, value: EngineStatus) {
        self.statuses.lock().unwrap().push(value);
    }
    fn expanded(&self, value: Expansion) {
        self.expansions.lock().unwrap().push(value);
    }
    fn form_requested(&self, value: FormRequest) {
        self.forms.lock().unwrap().push(value);
    }
    fn activity(&self, value: ActivityEvent) {
        self.activity.lock().unwrap().push(value);
    }
    fn clipboard_history(&self, index: usize, _: Option<&str>) -> Option<String> {
        self.history
            .lock()
            .unwrap()
            .get(index.checked_sub(1)?)
            .cloned()
    }
}
