//! Focus numbering, stamps, outcomes, placement and submit paths.
use super::model::{unique_fields, Panel};
use super::outcomes::{self, Kind};
use crate::shell::windows::{parts, Launch};
use crate::shell::Identity;
use kudzu_app::assembly::{Config, Host, Kudzu};
use kudzu_app::expansion::{InsertionOutcome, OutcomeNotice};
use kudzu_app::fixtures::Fixtures;
use kudzu_app::forms::FormRequest;
use kudzu_app::paths::Paths;
use kudzu_app::ports::engine::AdmissionError;
use kudzu_app::ports::{FormToken, HotkeyRegistrar};
use kudzu_app::settings::Variant;
use kudzu_core::activity::ActivitySource;
use kudzu_core::system::Rect;
use kudzu_core::template::{FormFieldKind, FormFieldSpec};
use rune::{Key, KeyEvent, Mods, Tx};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use uuid::Uuid;

fn field(name: &str, kind: FormFieldKind, options: &[&str]) -> FormFieldSpec {
    FormFieldSpec {
        name: name.into(),
        kind,
        options: options.iter().map(|o| (*o).into()).collect(),
        default_value: None,
    }
}

#[test]
fn unique_fields_keep_first_definition_and_share_a_name() {
    let fields = unique_fields(&[
        field("Name", FormFieldKind::Text, &[]),
        field("When", FormFieldKind::Choice, &["Mon", "Tue"]),
        field("Name", FormFieldKind::Multiline, &[]),
        field("Notes", FormFieldKind::Multiline, &[]),
    ]);
    let names: Vec<_> = fields.iter().map(|f| f.name.as_str()).collect();
    assert_eq!(names, ["Name", "When", "Notes"]);
    assert_eq!(fields[0].kind, FormFieldKind::Text);
}

fn notice(
    outcome: InsertionOutcome,
    snippet: Option<Uuid>,
    source: Option<ActivitySource>,
) -> OutcomeNotice {
    OutcomeNotice {
        sequence: 3,
        snippet_id: snippet,
        source,
        outcome,
    }
}

#[test]
fn outcomes_ignore_engine_stopped_and_other_snippets() {
    let snippet = Uuid::nil();
    let stopped = notice(InsertionOutcome::EngineStopped, None, None);
    assert_eq!(outcomes::classify(&stopped, Some(snippet)), Kind::Ignore);
    outcomes::acknowledge(&stopped);

    let ours = notice(
        InsertionOutcome::Expanded,
        Some(snippet),
        Some(ActivitySource::Form),
    );
    assert_eq!(outcomes::classify(&ours, Some(snippet)), Kind::Ours);

    let other = notice(
        InsertionOutcome::Expanded,
        Some(Uuid::from_u128(1)),
        Some(ActivitySource::Form),
    );
    assert_eq!(outcomes::classify(&other, Some(snippet)), Kind::Ignore);

    assert!(outcomes::next(Some(&ours), 2).is_some());
    assert!(outcomes::next(Some(&ours), 3).is_none());

    let lost = notice(
        InsertionOutcome::TriggerLost,
        Some(snippet),
        Some(ActivitySource::Form),
    );
    assert_eq!(outcomes::classify(&lost, Some(snippet)), Kind::Ours);
}

fn request(fields: Vec<FormFieldSpec>, initial: HashMap<String, String>) -> FormRequest {
    FormRequest {
        token: FormToken::default(),
        snippet_id: Uuid::nil(),
        snippet_name: "Demo".into(),
        fields,
        initial_values: initial,
        target: None,
        caret: None,
        selection: None,
    }
}

#[test]
fn the_first_text_field_is_focused_and_remembered_values_fill_it() {
    let mut initial = HashMap::new();
    initial.insert("Name".into(), "Ada".into());
    let panel = Panel::from_request(&request(
        vec![
            field("Name", FormFieldKind::Text, &[]),
            field("When", FormFieldKind::Choice, &["Mon"]),
        ],
        initial,
    ));
    assert!(panel.edits["Name"].focused);
    assert_eq!(panel.edits["Name"].text, "Ada");
    assert_eq!(panel.values["Name"], "Ada");
    assert!(!panel.edits["When"].focused);
}

#[test]
fn a_first_choice_field_is_not_a_text_focus() {
    let panel = Panel::from_request(&request(
        vec![field("When", FormFieldKind::Choice, &["Mon", "Tue"])],
        HashMap::new(),
    ));
    assert!(!panel.edits["When"].focused);
}

struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        let path = PathBuf::from("/tmp").join(format!("kudzu-form-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

const LAUNCH: Launch = Launch {
    identity: Identity {
        app_name: "Kudzu Next",
        dev: true,
    },
    native: false,
    hidden: false,
};

struct Rig {
    _scratch: Scratch,
    fixtures: Fixtures,
    kudzu: Arc<Kudzu>,
    p: crate::shell::windows::Parts,
}

fn rig() -> Rig {
    let scratch = Scratch::new();
    let fixtures = Fixtures::new().unwrap();
    let kudzu = Kudzu::new(Config {
        paths: Paths {
            data: scratch.0.clone(),
            logs: scratch.0.join("logs"),
        },
        host: Host::fixtures(&fixtures, scratch.0.clone()),
        variant: Variant::Dev,
        version: "0.7.0".into(),
        app_name: "Kudzu Next".into(),
        bundle_id: "sh.saeed.kudzu.next".into(),
        os_major: 15,
        mcp_name: "kudzu-next".into(),
        executable: scratch.0.join("Kudzu"),
        agent_args: vec!["--mcp".into()],
        bundled_skill_dir: None,
        production: false,
        cloud_and_updates: false,
        debounce: Duration::from_millis(50),
        retry: Duration::from_millis(50),
    })
    .unwrap();
    let registrar: Option<Arc<dyn HotkeyRegistrar>> = None;
    let p = parts(&kudzu, LAUNCH, registrar);
    Rig {
        _scratch: scratch,
        fixtures,
        kudzu,
        p,
    }
}

fn wait_for(mut ready: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while !ready() {
        assert!(Instant::now() < deadline, "condition was not met");
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn key(key: Key, cmd: bool) -> KeyEvent {
    KeyEvent {
        key,
        mods: Mods {
            cmd,
            ..Mods::default()
        },
    }
}

fn emit_form(r: &Rig, native: u64, fields: Vec<FormFieldSpec>) -> FormToken {
    let snippet = r.kudzu.library.watch().get().snippets[0].id;
    let token = r.fixtures.engine.form_token(native);
    r.fixtures.engine.emit_form(kudzu_app::ports::FormRequest {
        token,
        snippet_id: snippet,
        snippet_name: "Form".into(),
        fields,
        target: None,
        caret: None,
        selection: None,
    });
    wait_for(|| {
        r.kudzu
            .forms
            .pending()
            .get()
            .as_ref()
            .as_ref()
            .is_some_and(|request| request.token == token)
    });
    token
}

fn submitted(r: &Rig) -> usize {
    r.fixtures.engine.state.lock().unwrap().submissions.len()
}

#[test]
fn double_insert_submits_once() {
    let mut r = rig();
    emit_form(&r, 1, vec![]);
    assert!(r.p.form.sync());
    r.p.form.submit();
    r.p.form.submit();
    wait_for(|| submitted(&r) == 1);
    r.p.form.sync();
    assert_eq!(submitted(&r), 1);
}

#[test]
fn seq_restarts_at_one_on_the_next_form() {
    let mut r = rig();
    let first = emit_form(&r, 1, vec![]);
    assert!(r.p.form.sync());
    r.p.form.focused(true);
    assert_eq!(r.p.form.model.focus_seq, 1);
    r.p.form.cancel();
    wait_for(|| r.kudzu.forms.pending().get().is_none());
    r.p.form.sync();
    let second = emit_form(&r, 2, vec![]);
    assert!(r.p.form.sync());
    r.p.form.focused(true);
    let focus = r.fixtures.engine.state.lock().unwrap().focus.clone();
    let last = focus.last().expect("focus");
    assert_eq!(last.0, second);
    assert_eq!(last.1.seq, 1);
    assert!(last.1.key);
    assert_ne!(first, second);
}

#[test]
fn a_replaced_form_reports_key_at_seq_one() {
    let mut r = rig();
    let first = emit_form(&r, 1, vec![]);
    assert!(r.p.form.sync());
    r.p.form.focused(true);
    let second = emit_form(&r, 2, vec![]);
    assert!(r.p.form.sync());
    let focus = r.fixtures.engine.state.lock().unwrap().focus.clone();
    let last = focus.last().expect("replacement focus");
    assert_eq!(last.0, second);
    assert_eq!(last.1.seq, 1);
    assert!(last.1.key);
    assert_ne!(first, second);
}

#[test]
fn escape_cancels_through_on_key_and_return_submits_as_the_default_button() {
    let mut r = rig();
    emit_form(&r, 1, vec![]);
    assert!(r.p.form.sync());
    let mut tx = Tx::default();
    assert!(r.p.form.key(&key(Key::Escape, false), &mut tx));
    wait_for(|| r.kudzu.forms.pending().get().is_none());
    r.p.form.sync();
    assert!(r.p.form.model.request.is_none());

    emit_form(&r, 2, vec![]);
    assert!(r.p.form.sync());
    r.p.form.submit();
    wait_for(|| submitted(&r) == 1);
}

#[test]
fn command_return_submits_a_multiline_field() {
    let mut r = rig();
    emit_form(&r, 1, vec![field("Notes", FormFieldKind::Multiline, &[])]);
    assert!(r.p.form.sync());
    assert!(r
        .p
        .form
        .model
        .panel
        .as_ref()
        .is_some_and(|panel| panel.focused_multiline()));
    let mut tx = Tx::default();
    assert!(r.p.form.key(&key(Key::Enter, true), &mut tx));
    wait_for(|| submitted(&r) == 1);
}

#[test]
fn an_admission_error_is_kept_after_submit() {
    let mut r = rig();
    emit_form(&r, 1, vec![]);
    assert!(r.p.form.sync());
    r.fixtures.engine.state.lock().unwrap().admission_error = Some(AdmissionError::Busy);
    r.p.form.submit();
    wait_for(|| {
        r.p.shell.drain_settled();
        r.p.form.submit_admission.lock().unwrap().is_some()
    });
    assert_eq!(
        *r.p.form.submit_admission.lock().unwrap(),
        Some(Err(AdmissionError::Busy))
    );
    assert_eq!(submitted(&r), 0);
}

#[test]
fn the_form_is_placed_before_it_is_shown() {
    let mut r = rig();
    let mut request = super::scenes::demo_request();
    request.caret = Some(Rect {
        x: 400.0,
        y: 200.0,
        width: 2.0,
        height: 18.0,
    });
    r.p.form.play_scene(request);
    assert!(r.p.form.model.placed_before_show);
    assert_eq!(r.p.form.model.place_count, 1);
    assert_eq!(r.p.form.model.origin, Some((376.0, 224.0)));
}

#[test]
fn a_drag_then_a_height_change_keeps_the_origin() {
    let mut r = rig();
    let mut request = super::scenes::demo_request();
    request.caret = Some(Rect {
        x: 400.0,
        y: 200.0,
        width: 2.0,
        height: 18.0,
    });
    r.p.form.play_scene(request);
    r.p.form.model.origin = Some((40.0, 50.0));
    let mut tx = Tx::default();
    r.p.form.fit_height(240.0, &mut tx);
    assert_eq!(r.p.form.model.origin, Some((40.0, 50.0)));
    assert_eq!(r.p.form.model.place_count, 1);
}

#[test]
fn return_replaces_the_selected_range() {
    let mut r = rig();
    r.p.form.play_scene(request(
        vec![field("Notes", FormFieldKind::Multiline, &[])],
        HashMap::new(),
    ));
    let edit = r.p.form.edit("Notes");
    edit.text = "ab".into();
    edit.anchor = 0;
    edit.focus = 2;
    r.p.form.break_line("Notes");
    let edit = r.p.form.edit_snapshot("Notes");
    assert_eq!(edit.text, "\n");
    assert_eq!(edit.focus, 1);
    assert_eq!(edit.anchor, 1);
}
