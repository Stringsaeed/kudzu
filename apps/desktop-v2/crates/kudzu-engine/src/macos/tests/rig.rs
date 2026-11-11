//! A started engine on the fake desktop, and a host that records what the engine reports.
//!
//! [`Rig`] methods wait for the engine's worker to go idle after every event, like a user who
//! types slowly. Anything that must happen *during* an insertion is scripted with
//! [`Desk::at`] instead, so no test depends on thread timing.

use std::collections::HashMap;
use std::ops::Deref;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use kudzu_core::activity::{ActivityEvent, ActivityOutcome};
use kudzu_core::matching::MatchingConfig;
use kudzu_core::{FireMode, InsertionMethod, SnippetRecord};
use unicode_segmentation::UnicodeSegmentation;

use super::desk::{Desk, Ev, Out};
use super::fakes::{FakeBoard, FakeEvents, FakeObserve};
use crate::macos::pasteboard::Pasteboard;
use crate::macos::Platform;
use crate::{
    lock, Engine, EngineHost, EngineLibrary, EngineStatus, EventStamp, ExpansionEvent, FormRequest,
    TriggerLost,
};

/// Records every host callback.
#[derive(Default)]
pub(super) struct Host {
    pub statuses: Mutex<Vec<EngineStatus>>,
    pub expanded: Mutex<Vec<ExpansionEvent>>,
    pub forms: Mutex<Vec<FormRequest>>,
    pub activity: Mutex<Vec<ActivityEvent>>,
    pub lost: Mutex<Vec<TriggerLost>>,
}

impl EngineHost for Host {
    fn status_changed(&self, status: EngineStatus) {
        lock(&self.statuses).push(status);
    }

    fn expanded(&self, event: ExpansionEvent) {
        lock(&self.expanded).push(event);
    }

    fn form_requested(&self, request: FormRequest) {
        lock(&self.forms).push(request);
    }

    fn activity(&self, event: ActivityEvent) {
        lock(&self.activity).push(event);
    }

    fn trigger_lost(&self, lost: TriggerLost) {
        lock(&self.lost).push(lost);
    }
}

impl Host {
    pub fn outcomes(&self) -> Vec<ActivityOutcome> {
        lock(&self.activity)
            .iter()
            .map(|e| e.outcome.clone())
            .collect()
    }

    pub fn expansions(&self) -> usize {
        lock(&self.expanded).len()
    }

    /// The token of the form the engine asked for last.
    pub fn form_token(&self) -> u64 {
        lock(&self.forms)
            .last()
            .expect("a form was requested")
            .token
    }

    pub fn last_status(&self) -> EngineStatus {
        lock(&self.statuses).last().cloned().expect("a status")
    }
}

pub(super) fn form_values(values: &[(&str, &str)]) -> HashMap<String, String> {
    values
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

/// An immediate snippet with the default (automatic) insertion.
pub(super) fn snippet(trigger: &str, content: &str) -> SnippetRecord {
    SnippetRecord::new(trigger, vec![trigger.into()], content)
}

/// The same, always pasted.
pub(super) fn pasted(trigger: &str, content: &str) -> SnippetRecord {
    SnippetRecord {
        insertion: Some(InsertionMethod::Paste),
        ..snippet(trigger, content)
    }
}

/// A whole-word snippet: it fires on the separator typed after the trigger.
pub(super) fn whole_word(trigger: &str, content: &str) -> SnippetRecord {
    SnippetRecord {
        fire_mode: FireMode::WholeWord,
        ..snippet(trigger, content)
    }
}

pub(super) fn library(snippets: &[SnippetRecord]) -> EngineLibrary {
    EngineLibrary {
        snippets: snippets.to_vec(),
        groups: vec![],
        config: MatchingConfig::default(),
        locale: "en_US".into(),
    }
}

/// `n` Backspaces.
pub(super) fn backspaces(n: usize) -> Vec<Out> {
    vec![Out::Backspace; n]
}

/// Posted events as log entries.
pub(super) fn outs(outs: Vec<Out>) -> Vec<Ev> {
    outs.into_iter().map(Ev::Out).collect()
}

pub(super) fn ms(n: u64) -> Duration {
    Duration::from_millis(n)
}

pub(super) fn lefts(n: usize) -> Vec<Out> {
    vec![Out::Left; n]
}

pub(super) struct Rig {
    pub desk: Desk,
    pub engine: Arc<Engine>,
    pub host: Arc<Host>,
    pub pasteboard: Arc<Pasteboard>,
}

impl Rig {
    /// A started engine with `snippets`, Accessibility and Input Monitoring granted, the editor
    /// frontmost with an empty document.
    pub fn new(snippets: &[SnippetRecord]) -> Rig {
        Rig::with(snippets, |_| {})
    }

    /// The same, after `setup` adjusts the desktop. The log starts empty after `start`.
    pub fn with(snippets: &[SnippetRecord], setup: impl FnOnce(&Desk)) -> Rig {
        let rig = Rig::unstarted(snippets, setup);
        rig.engine.start();
        rig.desk.clear_log();
        rig
    }

    /// An engine that hasn't started (no listener yet), after `setup`.
    pub fn unstarted(snippets: &[SnippetRecord], setup: impl FnOnce(&Desk)) -> Rig {
        let desk = Desk::new();
        setup(&desk);
        let pasteboard = Arc::new(Pasteboard::new(Box::new(FakeBoard(desk.clone()))));
        let platform = Platform {
            events: Arc::new(FakeEvents(desk.clone())),
            pasteboard: pasteboard.clone(),
            observe: Arc::new(FakeObserve(desk.clone())),
        };
        let host = Arc::new(Host::default());
        let engine = Engine::with_platform(host.clone(), platform);
        desk.attach(&engine);
        engine.reload(library(snippets));
        Rig {
            desk,
            engine,
            host,
            pasteboard,
        }
    }

    /// Waits until every job sent to the engine's worker has finished.
    pub fn settle(&self) {
        let deadline = Instant::now() + Duration::from_secs(10);
        while !self.engine.imp.idle() {
            assert!(Instant::now() < deadline, "the engine never went idle");
            thread::sleep(Duration::from_micros(200));
        }
    }

    /// Types `keys` in the target app, letting the engine finish after every key event.
    pub fn type_(&self, keys: &str) {
        for key in keys.graphemes(true) {
            self.desk.key(key, true, true);
            self.settle();
            self.desk.key(key, false, true);
            self.settle();
        }
    }

    /// Types `keys` wherever focus is (see `panels.rs`), letting the engine finish after every
    /// key event.
    pub fn press(&self, keys: &str) {
        for key in keys.graphemes(true) {
            self.desk.key_at_focus(key, true);
            self.settle();
            self.desk.key_at_focus(key, false);
            self.settle();
        }
    }

    /// Types `keys` into the form panel (the listener sees them; the document doesn't).
    pub fn type_in_panel(&self, keys: &str) {
        self.desk.types_in_panel(keys);
        self.settle();
    }

    /// Submits form `token` from its panel by the last event the user's apps received.
    pub fn submit(&self, token: u64, values: &[(&str, &str)]) {
        self.submit_at(token, values, self.desk.last_stamp());
    }

    /// Submits form `token` by the event stamped `at`.
    pub fn submit_at(&self, token: u64, values: &[(&str, &str)], at: EventStamp) {
        self.engine.submit_form(token, form_values(values), at);
        self.settle();
    }

    /// The id of the loaded snippet with `trigger`.
    pub fn snippet_id(&self, trigger: &str) -> uuid::Uuid {
        let library = self.engine.imp.library();
        let found = library.snippets.values().find(|s| s.triggers == [trigger]);
        found.expect("a snippet with that trigger").id
    }

    /// The loaded snippets, as the app would hand them over again after saving usage counts.
    pub fn loaded(&self) -> Vec<SnippetRecord> {
        let library = self.engine.imp.library();
        library.snippets.values().cloned().collect()
    }

    pub fn reload(&self, snippets: &[SnippetRecord]) {
        self.engine.reload(library(snippets));
    }

    /// Posted events since the last call (and clears the log).
    pub fn take_posted(&self) -> Vec<Out> {
        let posted = self.desk.posted();
        self.desk.clear_log();
        posted
    }
}

impl Deref for Rig {
    type Target = Desk;

    fn deref(&self) -> &Desk {
        &self.desk
    }
}

impl Drop for Rig {
    fn drop(&mut self) {
        if !thread::panicking() {
            assert_eq!(self.desk.unfired(), [], "a scripted action never ran");
        }
    }
}
