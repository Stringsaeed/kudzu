//! What every `ExpansionEngine` + `FormChannel` + `KudzuWindows` implementation promises:
//! admission only (D20), forwarding to the native engine, token namespacing, and detaching
//! the event sink on stop. The fake and the macOS adapter both run these.
//!
//! A harness exposes the port and, when it can, the native side behind it: something that
//! reports as the native engine would and records what reached it. The real macOS engine
//! needs Accessibility and a live event tap for that, so its harness has no native side and
//! runs only [`without_native`].
use crate::{
    fixtures::engine::FakeEngineEvents,
    ports::{
        engine::AdmissionError, EngineLibrary, EventStamp, Expansion, ExpansionEngine, FormChannel,
        FormSubmission, FormToken, InputEvent, KudzuWindow, KudzuWindows, PanelFocus, TriggerLost,
    },
};
use kudzu_core::{
    activity::{ActivityEvent, ActivityOutcome},
    system::{AppInfo, EngineStatus},
};
use std::{collections::HashMap, sync::Arc};
use uuid::Uuid;

/// What reached the native engine, in native terms (raw form tokens).
#[derive(Clone, Default, PartialEq)]
pub struct NativeLog {
    pub inserts: Vec<(Uuid, Option<AppInfo>, Option<String>)>,
    pub submissions: Vec<(u64, HashMap<String, String>, FormSubmission)>,
    pub cancels: Vec<u64>,
    pub focus: Vec<(u64, PanelFocus)>,
    pub windows: Vec<Vec<KudzuWindow>>,
    /// Snippet count and locale of every reload, in order.
    pub reloads: Vec<(usize, String)>,
}

/// The native engine behind a port, driven by the contract.
pub trait NativeSide {
    /// The native engine asks for a form under its own token `native` (≥ 1).
    fn request_form(&self, native: u64, snippet: Uuid, target: Option<AppInfo>);
    fn report_status(&self, status: EngineStatus);
    fn report_expansion(&self, expansion: Expansion);
    fn report_activity(&self, event: ActivityEvent);
    fn report_trigger_lost(&self, lost: TriggerLost);
    /// The native engine asks the host for clipboard history entry `index`.
    fn ask_history(&self, index: usize, current: Option<&str>) -> Option<String>;
    fn log(&self) -> NativeLog;
}

pub trait EngineHarness {
    fn engine(&self) -> &dyn ExpansionEngine;
    fn forms(&self) -> &dyn FormChannel;
    fn windows(&self) -> &dyn KudzuWindows;
    /// None when the native side cannot be driven without permissions (the real engine).
    fn native(&self) -> Option<&dyn NativeSide>;
    /// A token qualified by an engine instance other than this one's.
    fn foreign_token(&self, native: u64) -> FormToken;
    /// A token qualified by this engine's own instance. Only harnesses that never start may
    /// need it; the others learn tokens from forwarded form requests.
    fn own_token(&self, native: u64) -> FormToken;
}

fn submission(stamp: u64) -> FormSubmission {
    FormSubmission {
        event: Some(InputEvent {
            stamp: EventStamp::from_cg_event_timestamp(stamp),
            key_code: Some(36),
        }),
        focus_seq: 2,
    }
}
fn focus(key: bool, seq: u64) -> PanelFocus {
    PanelFocus {
        key,
        seq,
        after: Some(InputEvent {
            stamp: EventStamp::from_cg_event_timestamp(40 + seq),
            key_code: None,
        }),
    }
}
fn target() -> AppInfo {
    AppInfo {
        bundle_id: "com.example.target".into(),
        name: "Target".into(),
        pid: Some(4242),
        path: None,
    }
}
fn windows() -> Vec<KudzuWindow> {
    vec![
        KudzuWindow {
            number: 31,
            passes_clicks_through: false,
        },
        KudzuWindow {
            number: 32,
            passes_clicks_through: true,
        },
    ]
}

/// Every case that needs neither a started engine nor a driven native side.
pub fn without_native(h: &dyn EngineHarness) {
    let snippet = Uuid::from_u128(0x5135);
    assert_eq!(
        h.engine().insert(snippet, Some(target()), None),
        Err(AdmissionError::NoEngine),
        "nothing is admitted before start"
    );
    assert_eq!(
        h.forms()
            .submit_form(h.own_token(1), HashMap::new(), submission(10)),
        Err(AdmissionError::NoEngine)
    );
    h.forms().cancel_form(h.foreign_token(1));
    h.forms().panel_focused(h.foreign_token(1), focus(true, 1));
    h.forms().cancel_form(h.own_token(1));
    h.engine().set_paused(true);
    assert!(h.engine().status().paused);
    h.engine().set_paused(false);
    assert!(!h.engine().status().paused);
    h.engine().set_suppressed(true);
    h.engine().set_suppressed(false);
    h.engine().reload(EngineLibrary {
        locale: "en_GB".into(),
        ..Default::default()
    });
    h.windows().set_kudzu_windows(&windows());
    h.windows().set_kudzu_windows(&[]);
}

/// Forwarding, token namespacing and the stop/start lifecycle, through the native side.
pub fn with_native(h: &dyn EngineHarness) {
    let native = h.native().expect("this case drives the native side");
    let events = Arc::new(FakeEngineEvents::default());
    h.engine().start(events.clone()).unwrap();
    let snippet = Uuid::from_u128(0x5135);

    // Reload and windows reach the native engine as given.
    h.engine().reload(EngineLibrary {
        locale: "en_GB".into(),
        ..Default::default()
    });
    assert_eq!(native.log().reloads.last(), Some(&(0, "en_GB".to_string())));
    h.windows().set_kudzu_windows(&windows());
    assert_eq!(native.log().windows.last(), Some(&windows()));

    // Insert is admission only and reaches the native engine once.
    let selection = Some("picked".to_string());
    assert_eq!(
        h.engine()
            .insert(snippet, Some(target()), selection.clone()),
        Ok(())
    );
    assert_eq!(native.log().inserts, [(snippet, Some(target()), selection)]);

    // A form request arrives qualified; its token maps back to the native one.
    native.request_form(7, snippet, Some(target()));
    let request = events.forms.lock().unwrap().last().cloned().expect("form");
    assert_eq!(request.snippet_id, snippet);
    assert_eq!(request.target, Some(target()));
    let token = request.token;
    assert_ne!(
        token,
        h.foreign_token(7),
        "tokens carry their engine instance"
    );
    h.forms().panel_focused(token, focus(true, 1));
    h.forms().panel_focused(token, focus(false, 2));
    let values = HashMap::from([("who".to_string(), "Ada".to_string())]);
    assert_eq!(
        h.forms().submit_form(token, values.clone(), submission(99)),
        Ok(())
    );
    h.forms().cancel_form(token);
    let log = native.log();
    assert_eq!(log.focus, [(7, focus(true, 1)), (7, focus(false, 2))]);
    assert_eq!(log.submissions, [(7, values, submission(99))]);
    assert_eq!(log.cancels, [7]);

    // A token from another engine instance never reaches this one.
    assert_eq!(
        h.forms()
            .submit_form(h.foreign_token(7), HashMap::new(), submission(100)),
        Err(AdmissionError::StaleToken)
    );
    h.forms().cancel_form(h.foreign_token(7));
    h.forms().panel_focused(h.foreign_token(7), focus(true, 3));
    let after = native.log();
    assert_eq!((after.submissions.len(), after.cancels.len()), (1, 1));
    assert_eq!(after.focus.len(), 2);

    // Reports are forwarded typed.
    let expansion = Expansion {
        snippet_id: snippet,
        chars_saved: 9,
    };
    native.report_expansion(expansion);
    assert_eq!(*events.expansions.lock().unwrap(), [expansion]);
    let lost = TriggerLost {
        snippet_id: snippet,
        trigger: ";sig".into(),
    };
    native.report_trigger_lost(lost.clone());
    assert!(*events.lost_triggers.lock().unwrap() == [lost]);
    native.report_activity(ActivityEvent::health(ActivityOutcome::EngineStopped));
    assert_eq!(events.activity.lock().unwrap().len(), 1);
    let mut status = h.engine().status();
    status.summary = "contract".into();
    native.report_status(status.clone());
    assert_eq!(events.statuses.lock().unwrap().last(), Some(&status));
    events.history.lock().unwrap().push("older".into());
    assert_eq!(native.ask_history(1, None).as_deref(), Some("older"));
    assert_eq!(native.ask_history(2, None), None);

    // Stop detaches the sink and refuses admission; the same engine restarts in its namespace.
    h.engine().stop();
    let forms_before = events.forms.lock().unwrap().len();
    native.request_form(8, snippet, None);
    native.report_expansion(expansion);
    assert_eq!(events.forms.lock().unwrap().len(), forms_before);
    assert_eq!(events.expansions.lock().unwrap().len(), 1);
    assert_eq!(native.ask_history(1, None), None, "no host, no history");
    assert_eq!(
        h.engine().insert(snippet, None, None),
        Err(AdmissionError::NoEngine)
    );
    assert_eq!(
        h.forms()
            .submit_form(token, HashMap::new(), submission(101)),
        Err(AdmissionError::NoEngine)
    );
    h.engine().start(events.clone()).unwrap();
    native.request_form(9, snippet, None);
    let again = events.forms.lock().unwrap().last().unwrap().token;
    assert_eq!(again.instance(), token.instance());
    assert_eq!(
        h.forms()
            .submit_form(again, HashMap::new(), submission(102)),
        Ok(())
    );
    assert_eq!(native.log().submissions.last().unwrap().0, 9);
    h.engine().stop();
}
