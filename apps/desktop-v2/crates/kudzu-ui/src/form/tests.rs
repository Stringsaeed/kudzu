//! Focus numbering, stamps, outcomes and unique controls. Window wiring is in `shell::tests`.
use super::model::{unique_fields, Panel};
use super::outcomes::{self, Kind};
use kudzu_app::expansion::{InsertionOutcome, OutcomeNotice};
use kudzu_app::forms::FormRequest;
use kudzu_app::ports::FormToken;
use kudzu_core::activity::ActivitySource;
use kudzu_core::template::{FormFieldKind, FormFieldSpec};
use std::collections::HashMap;
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
}

#[test]
fn a_missing_native_event_is_none() {
    super::events::fake::clear();
    assert!(super::events::fake::current().is_none());
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
