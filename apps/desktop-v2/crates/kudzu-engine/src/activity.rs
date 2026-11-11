//! Turns what the engine does into [`ActivityEvent`]s for the host ("Why didn't this expand?").
//!
//! The [`Recorder`] sits between the engine and its host. The worker opens an *attempt* when a
//! matched trigger, palette insert or form submission starts, and closes it when that job ends.
//! In between, the insertion code is left alone: its existing `expanded` and `form_requested`
//! host callbacks pass through the recorder, which is how an attempt learns it expanded or is
//! waiting on a form. Guards that know a more specific reason call [`Recorder::resolve`].
//!
//! Events name a snippet, its stored trigger and the app. They never carry typed or inserted
//! text; see `kudzu_core::activity` for the full privacy contract.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use kudzu_core::activity::{ActivityApp, ActivityEvent, ActivityOutcome, ActivitySource};
use uuid::Uuid;

use crate::{lock, AppInfo, EngineHost, EngineStatus, ExpansionEvent, FormRequest};

/// What an attempt is about. Holds no text beyond the snippet's stored trigger.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Subject {
    pub source: ActivitySource,
    pub snippet_id: Uuid,
    /// The trigger as stored; `None` for palette inserts.
    pub trigger: Option<String>,
    pub app: Option<ActivityApp>,
}

impl Subject {
    fn event(self, outcome: ActivityOutcome) -> ActivityEvent {
        ActivityEvent::attempt(self.source, self.snippet_id, self.trigger, self.app, outcome)
    }
}

/// An [`AppInfo`] as the activity log stores it.
pub(crate) fn activity_app(app: &AppInfo) -> ActivityApp {
    ActivityApp { bundle_id: app.bundle_id.clone(), name: app.name.clone(), path: app.path.clone() }
}

#[derive(Debug)]
struct Attempt {
    subject: Subject,
    outcome: Option<ActivityOutcome>,
    /// A form was requested: the attempt continues when it is submitted or cancelled.
    awaiting_form: bool,
}

#[derive(Debug, Default)]
struct State {
    attempt: Option<Attempt>,
    /// The expansion a Backspace would undo.
    last_expansion: Option<Subject>,
    /// The form on screen, by token.
    forms: HashMap<u64, Subject>,
}

/// Records attempts and forwards every host callback. Host callbacks are made with no lock held.
pub(crate) struct Recorder {
    host: Arc<dyn EngineHost>,
    state: Mutex<State>,
}

impl Recorder {
    pub fn new(host: Arc<dyn EngineHost>) -> Self {
        Self { host, state: Mutex::new(State::default()) }
    }

    /// Reports `outcome` for `subject` right away (a trigger that matched but never started).
    pub fn note(&self, subject: Subject, outcome: ActivityOutcome) {
        self.host.activity(subject.event(outcome));
    }

    /// Reports an engine health change.
    pub fn health(&self, outcome: ActivityOutcome) {
        self.host.activity(ActivityEvent::health(outcome));
    }

    /// Opens an attempt; `None` (nothing to report, e.g. an undo) just closes the previous one.
    pub fn begin(&self, subject: Option<Subject>) {
        let mut state = lock(&self.state);
        if subject.is_some() {
            // A new trigger fired, so the previous expansion can no longer be undone.
            state.last_expansion = None;
        }
        state.attempt = subject.map(|subject| Attempt { subject, outcome: None, awaiting_form: false });
    }

    /// Opens an attempt for the submitted form `token`, if that form is the one on screen.
    pub fn begin_form(&self, token: u64) {
        let mut state = lock(&self.state);
        let subject = state.forms.remove(&token).map(|subject| Subject { source: ActivitySource::Form, ..subject });
        if subject.is_some() {
            state.last_expansion = None;
        }
        state.attempt = subject.map(|subject| Attempt { subject, outcome: None, awaiting_form: false });
    }

    /// Records why the open attempt ended. The first reason wins, except that a completed
    /// expansion (seen through [`EngineHost::expanded`]) always does.
    pub fn resolve(&self, outcome: ActivityOutcome) {
        if let Some(attempt) = lock(&self.state).attempt.as_mut() {
            attempt.outcome.get_or_insert(outcome);
        }
    }

    /// Closes the open attempt and reports it. `unresolved` supplies the outcome when nothing
    /// along the way did; it runs with no lock held.
    pub fn finish(&self, unresolved: impl FnOnce() -> ActivityOutcome) {
        let Some(attempt) = lock(&self.state).attempt.take() else { return };
        if attempt.awaiting_form {
            return;
        }
        let outcome = attempt.outcome.unwrap_or_else(unresolved);
        self.host.activity(attempt.subject.event(outcome));
    }

    /// Backspace undid the last expansion.
    pub fn undone(&self) {
        let last = lock(&self.state).last_expansion.take();
        if let Some(subject) = last {
            self.host.activity(subject.event(ActivityOutcome::Undone));
        }
    }

    /// The form `token` was dismissed without inserting.
    pub fn form_cancelled(&self, token: u64) {
        let form = lock(&self.state).forms.remove(&token);
        if let Some(subject) = form {
            self.host.activity(Subject { source: ActivitySource::Form, ..subject }.event(ActivityOutcome::FormCancelled));
        }
    }
}

impl EngineHost for Recorder {
    fn status_changed(&self, status: EngineStatus) {
        self.host.status_changed(status);
    }

    fn expanded(&self, event: ExpansionEvent) {
        {
            let mut state = lock(&self.state);
            if let Some(attempt) = state.attempt.as_mut() {
                attempt.outcome = Some(ActivityOutcome::Expanded);
                state.last_expansion = Some(attempt.subject.clone());
            }
        }
        self.host.expanded(event);
    }

    fn form_requested(&self, request: FormRequest) {
        {
            let mut state = lock(&self.state);
            if let Some(attempt) = state.attempt.as_mut() {
                attempt.awaiting_form = true;
                let subject = attempt.subject.clone();
                // The engine shows one form at a time; a new one replaces whatever was pending.
                state.forms.clear();
                state.forms.insert(request.token, subject);
            }
        }
        self.host.form_requested(request);
    }

    fn clipboard_history(&self, index: usize, current: Option<&str>) -> Option<String> {
        self.host.clipboard_history(index, current)
    }

    fn activity(&self, event: ActivityEvent) {
        self.host.activity(event);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kudzu_core::activity::InsertFailure;

    #[derive(Default)]
    struct Host {
        events: Mutex<Vec<ActivityEvent>>,
        expansions: Mutex<Vec<ExpansionEvent>>,
        forms: Mutex<Vec<u64>>,
    }

    impl EngineHost for Host {
        fn status_changed(&self, _status: EngineStatus) {}
        fn expanded(&self, event: ExpansionEvent) {
            self.expansions.lock().unwrap().push(event);
        }
        fn form_requested(&self, request: FormRequest) {
            self.forms.lock().unwrap().push(request.token);
        }
        fn activity(&self, event: ActivityEvent) {
            self.events.lock().unwrap().push(event);
        }
    }

    fn setup() -> (Arc<Host>, Recorder) {
        let host = Arc::new(Host::default());
        (host.clone(), Recorder::new(host))
    }

    fn typed(id: Uuid) -> Subject {
        let app = ActivityApp { bundle_id: "com.apple.mail".into(), name: "Mail".into(), path: None };
        Subject { source: ActivitySource::Typed, snippet_id: id, trigger: Some(";sig".into()), app: Some(app) }
    }

    fn outcomes(host: &Host) -> Vec<ActivityOutcome> {
        host.events.lock().unwrap().iter().map(|e| e.outcome.clone()).collect()
    }

    fn form(token: u64, id: Uuid) -> FormRequest {
        FormRequest { token, snippet_id: id, snippet_name: "s".into(), fields: vec![], caret: None, target_app: None, selection: None }
    }

    fn unreachable_fallback() -> ActivityOutcome {
        panic!("the attempt was already resolved")
    }

    #[test]
    fn an_expansion_is_reported_when_its_job_ends_and_still_reaches_the_host() {
        let (host, recorder) = setup();
        let id = Uuid::new_v4();
        recorder.begin(Some(typed(id)));
        recorder.expanded(ExpansionEvent { snippet_id: id, chars_saved: 8 });
        assert!(outcomes(&host).is_empty());
        recorder.finish(unreachable_fallback);
        assert_eq!(outcomes(&host), [ActivityOutcome::Expanded]);
        assert_eq!(host.expansions.lock().unwrap().len(), 1);
        let events = host.events.lock().unwrap();
        assert_eq!((events[0].snippet_id, events[0].trigger.as_deref(), events[0].source), (Some(id), Some(";sig"), Some(ActivitySource::Typed)));
        assert_eq!(events[0].app.as_ref().unwrap().name, "Mail");
    }

    #[test]
    fn an_attempt_nothing_resolved_falls_back() {
        let (host, recorder) = setup();
        recorder.begin(Some(typed(Uuid::new_v4())));
        recorder.finish(|| ActivityOutcome::Interrupted);
        assert_eq!(outcomes(&host), [ActivityOutcome::Interrupted]);
        // Closed: finishing again reports nothing.
        recorder.finish(unreachable_fallback);
        assert_eq!(outcomes(&host).len(), 1);
    }

    #[test]
    fn the_first_reason_wins_but_an_expansion_overrides() {
        let (host, recorder) = setup();
        let id = Uuid::new_v4();
        recorder.begin(Some(typed(id)));
        recorder.resolve(ActivityOutcome::Paused);
        recorder.resolve(ActivityOutcome::InsertionFailed { error: InsertFailure::NotAllowed });
        recorder.finish(unreachable_fallback);
        recorder.begin(Some(typed(id)));
        recorder.resolve(ActivityOutcome::Interrupted);
        recorder.expanded(ExpansionEvent { snippet_id: id, chars_saved: 0 });
        recorder.finish(unreachable_fallback);
        assert_eq!(outcomes(&host), [ActivityOutcome::Paused, ActivityOutcome::Expanded]);
    }

    #[test]
    fn resolving_without_an_attempt_reports_nothing() {
        let (host, recorder) = setup();
        recorder.resolve(ActivityOutcome::Paused);
        recorder.begin(None);
        recorder.finish(unreachable_fallback);
        recorder.expanded(ExpansionEvent { snippet_id: Uuid::nil(), chars_saved: 0 });
        assert!(outcomes(&host).is_empty());
        assert_eq!(host.expansions.lock().unwrap().len(), 1);
    }

    #[test]
    fn backspace_undoes_only_the_latest_expansion_once() {
        let (host, recorder) = setup();
        let id = Uuid::new_v4();
        recorder.undone();
        recorder.begin(Some(typed(id)));
        recorder.expanded(ExpansionEvent { snippet_id: id, chars_saved: 0 });
        recorder.finish(unreachable_fallback);
        // The undo runs as its own job with nothing to report up front.
        recorder.begin(None);
        recorder.undone();
        recorder.finish(unreachable_fallback);
        recorder.undone();
        assert_eq!(outcomes(&host), [ActivityOutcome::Expanded, ActivityOutcome::Undone]);
        assert_eq!(host.events.lock().unwrap()[1].snippet_id, Some(id));
    }

    #[test]
    fn a_later_trigger_makes_the_earlier_expansion_final() {
        let (host, recorder) = setup();
        let id = Uuid::new_v4();
        recorder.begin(Some(typed(id)));
        recorder.expanded(ExpansionEvent { snippet_id: id, chars_saved: 0 });
        recorder.finish(unreachable_fallback);
        recorder.begin(Some(typed(Uuid::new_v4())));
        recorder.finish(|| ActivityOutcome::Interrupted);
        recorder.undone();
        assert_eq!(outcomes(&host), [ActivityOutcome::Expanded, ActivityOutcome::Interrupted]);
    }

    #[test]
    fn a_form_defers_the_report_until_it_is_submitted() {
        let (host, recorder) = setup();
        let id = Uuid::new_v4();
        recorder.begin(Some(typed(id)));
        recorder.form_requested(form(7, id));
        recorder.finish(unreachable_fallback);
        assert!(outcomes(&host).is_empty());
        assert_eq!(*host.forms.lock().unwrap(), [7]);

        recorder.begin_form(7);
        recorder.expanded(ExpansionEvent { snippet_id: id, chars_saved: 3 });
        recorder.finish(unreachable_fallback);
        let events = host.events.lock().unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].outcome, ActivityOutcome::Expanded);
        // Still names the stored trigger that opened the form.
        assert_eq!((events[0].source, events[0].trigger.as_deref()), (Some(ActivitySource::Form), Some(";sig")));
    }

    #[test]
    fn a_cancelled_form_is_reported_once() {
        let (host, recorder) = setup();
        let id = Uuid::new_v4();
        recorder.begin(Some(Subject { source: ActivitySource::QuickSearch, trigger: None, ..typed(id) }));
        recorder.form_requested(form(3, id));
        recorder.finish(unreachable_fallback);
        recorder.form_cancelled(99);
        recorder.form_cancelled(3);
        recorder.form_cancelled(3);
        assert_eq!(outcomes(&host), [ActivityOutcome::FormCancelled]);
        assert_eq!(host.events.lock().unwrap()[0].source, Some(ActivitySource::Form));
        // Submitting a form that is gone opens no attempt.
        recorder.begin_form(3);
        recorder.finish(unreachable_fallback);
        assert_eq!(outcomes(&host).len(), 1);
    }

    #[test]
    fn a_new_form_replaces_the_pending_one() {
        let (host, recorder) = setup();
        let id = Uuid::new_v4();
        for token in [1, 2] {
            recorder.begin(Some(typed(id)));
            recorder.form_requested(form(token, id));
            recorder.finish(unreachable_fallback);
        }
        recorder.form_cancelled(1);
        assert!(outcomes(&host).is_empty());
        recorder.form_cancelled(2);
        assert_eq!(outcomes(&host), [ActivityOutcome::FormCancelled]);
    }

    #[test]
    fn notes_and_health_are_reported_immediately() {
        let (host, recorder) = setup();
        recorder.note(typed(Uuid::nil()), ActivityOutcome::SnippetOff);
        recorder.health(ActivityOutcome::ListenerLost);
        let events = host.events.lock().unwrap();
        assert_eq!(events[0].outcome, ActivityOutcome::SnippetOff);
        assert_eq!((events[1].snippet_id, events[1].source), (None, None));
    }
}
