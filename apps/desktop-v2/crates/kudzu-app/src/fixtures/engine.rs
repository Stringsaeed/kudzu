//! Controllable engine and callback collector. All callbacks run outside fake locks.
use crate::{
    error::PortError,
    ports::engine::AdmissionError,
    ports::{
        EngineEvents, EngineLibrary, Expansion, ExpansionEngine, FormChannel, FormRequest,
        TriggerLost,
    },
    ports::{EngineInstance, EngineInstances, EventStamp, FormToken},
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
    pub instance: EngineInstance,
    pub cancel_attempts: Vec<FormToken>,
    pub running: bool,
    pub status: EngineStatus,
    pub suppressed: bool,
    pub library: EngineLibrary,
    pub reloads: usize,
    pub insertions: Vec<InsertRequest>,
    pub submissions: Vec<(FormToken, HashMap<String, String>, EventStamp)>,
    pub cancelled: Vec<FormToken>,
    pub focus: Vec<(FormToken, bool, Option<EventStamp>)>,
    pub failure: Option<PortError>,
    pub admission_error: Option<AdmissionError>,
    pub interrupted_forms: std::collections::HashSet<FormToken>,
}
impl Default for EngineState {
    fn default() -> Self {
        Self {
            instance: EngineInstance::default(),
            cancel_attempts: Vec::new(),
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
            admission_error: None,
            interrupted_forms: Default::default(),
        }
    }
}
pub struct FakeEngine {
    instances: EngineInstances,
    pub state: Mutex<EngineState>,
    events: Mutex<Option<Arc<dyn EngineEvents>>>,
    pub before_submit: Mutex<Option<Arc<dyn Fn() + Send + Sync>>>,
}
impl Default for FakeEngine {
    fn default() -> Self {
        let instances = EngineInstances::default();
        let state = EngineState {
            instance: instances.next_engine(),
            ..Default::default()
        };
        Self {
            instances,
            state: Mutex::new(state),
            events: Mutex::new(None),
            before_submit: Mutex::new(None),
        }
    }
}
impl FakeEngine {
    pub fn form_token(&self, native: u64) -> FormToken {
        self.state.lock().unwrap().instance.form_token(native)
    }
    /// Simulates replacing the native Engine object. Caller must start it and resync services.
    pub fn restart_instance(&self) {
        self.stop();
        let mut state = self.state.lock().unwrap();
        state.instance = self.instances.next_engine();
        state.library = EngineLibrary::default();
        state.status.paused = false;
        state.suppressed = false;
        state.interrupted_forms.clear();
    }
    /// Simulates a key-down using the reported focus. Native timestamp ordering and click
    /// hit-testing are engine protocol tests; this fake records window stamps unchanged.
    /// Suppression must not hide an interruption signal (D9).
    pub fn input_outside_form(&self, token: FormToken) {
        let mut state = self.state.lock().unwrap();
        let focused = state
            .focus
            .iter()
            .rev()
            .find(|(t, _, _)| *t == token)
            .is_some_and(|(_, focused, _)| *focused);
        if !focused {
            state.interrupted_forms.insert(token);
        }
    }
    pub fn events(&self) -> Option<Arc<dyn EngineEvents>> {
        self.events.lock().unwrap().clone()
    }
    pub fn emit_activity(&self, event: ActivityEvent) {
        if let Some(events) = self.events() {
            events.activity(event);
        }
    }
    pub fn emit_trigger_lost(&self, lost: TriggerLost) {
        if let Some(events) = self.events() {
            events.trigger_lost(lost);
        }
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
    ) -> Result<(), AdmissionError> {
        let mut state = self.state.lock().unwrap();
        admit(&state)?;
        state.insertions.push(InsertRequest {
            snippet,
            target,
            selection,
        });
        Ok(())
    }
}
fn admit(state: &EngineState) -> Result<(), AdmissionError> {
    if !state.running {
        return Err(AdmissionError::NoEngine);
    }
    state.admission_error.map_or(Ok(()), Err)
}
impl FormChannel for FakeEngine {
    fn submit_form(
        &self,
        token: FormToken,
        values: HashMap<String, String>,
        at: EventStamp,
    ) -> Result<(), AdmissionError> {
        {
            admit(&self.state.lock().unwrap())?;
        }
        let observer = self.before_submit.lock().unwrap().clone();
        if let Some(observer) = observer {
            observer();
        }
        let mut state = self.state.lock().unwrap();
        admit(&state)?;
        if state.instance.native_token(token).is_none() {
            return Err(AdmissionError::StaleToken);
        }
        state.submissions.push((token, values, at));
        Ok(())
    }
    fn cancel_form(&self, token: FormToken) {
        let mut state = self.state.lock().unwrap();
        state.cancel_attempts.push(token);
        if state.instance.native_token(token).is_some() {
            state.cancelled.push(token);
        }
    }
    fn panel_focused(&self, token: FormToken, focused: bool, after: Option<EventStamp>) {
        let mut state = self.state.lock().unwrap();
        if state.instance.native_token(token).is_some() {
            state.focus.push((token, focused, after));
        }
    }
}
impl FakeEngine {
    /// Resolve a previously admitted request separately, as a native worker would.
    pub fn resolve_insert(&self, index: usize) {
        use kudzu_core::activity::{
            ActivityOutcome as A, ActivitySource, InsertFailure, Permission, RuleLevel,
        };
        let (request, outcome) = {
            let state = self.state.lock().unwrap();
            let request = state.insertions[index].clone();
            let outcome = if state.status.paused {
                A::Paused
            } else if state.status.secure_input {
                A::SecureInput { owner: None }
            } else if !state.status.can_post_events {
                A::MissingPermission {
                    permission: Permission::Accessibility,
                }
            } else if request.target.as_ref().is_none() {
                A::InsertionFailed {
                    error: InsertFailure::TargetGone,
                }
            } else if request.target.as_ref().is_some_and(|a| {
                state
                    .library
                    .config
                    .denied_bundle_ids
                    .contains(&a.bundle_id)
            }) {
                A::AppRule {
                    level: RuleLevel::AppSetting,
                    group_id: None,
                }
            } else {
                A::Expanded
            };
            (request, outcome)
        };
        self.emit_activity(ActivityEvent::attempt(
            ActivitySource::QuickSearch,
            request.snippet,
            None,
            None,
            outcome,
        ));
    }
    pub fn resolve_form(&self, token: FormToken, snippet_id: Uuid) {
        use kudzu_core::activity::{ActivityOutcome, ActivitySource};
        let interrupted = self.state.lock().unwrap().interrupted_forms.remove(&token);
        let outcome = if interrupted {
            ActivityOutcome::Interrupted
        } else {
            ActivityOutcome::Expanded
        };
        self.emit_activity(ActivityEvent::attempt(
            ActivitySource::Form,
            snippet_id,
            None,
            None,
            outcome,
        ));
    }
}

#[derive(Default)]
pub struct FakeEngineEvents {
    pub statuses: Mutex<Vec<EngineStatus>>,
    pub expansions: Mutex<Vec<Expansion>>,
    pub forms: Mutex<Vec<FormRequest>>,
    pub activity: Mutex<Vec<ActivityEvent>>,
    pub lost_triggers: Mutex<Vec<TriggerLost>>,
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
    fn trigger_lost(&self, lost: TriggerLost) {
        self.lost_triggers.lock().unwrap().push(lost);
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
