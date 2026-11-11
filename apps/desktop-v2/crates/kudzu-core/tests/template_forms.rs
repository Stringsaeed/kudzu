//! Recursive form fields and form memory (ported from FormTests.swift).

use std::collections::{HashMap, HashSet};

use kudzu_core::template::{self, FormFieldKind, FormMemory};
use uuid::Uuid;

fn names(fields: &[template::FormFieldSpec]) -> Vec<&str> {
    fields.iter().map(|f| f.name.as_str()).collect()
}

#[test]
fn recursive_form_order_and_first_definition() {
    let id = Uuid::new_v4();
    let source = format!(
        "{{{{field name=a}}}}{{{{snippet id=\"{id}\"}}}}{{{{choice name=b options=\"wrong\"}}}}{{{{field name=c}}}}"
    );
    let fields = template::form_fields(&source, &|nested| {
        (nested == id).then(|| {
            "{{choice name=b options=\"one|two\"}}{{field name=a multiline=true}}".to_string()
        })
    });
    assert_eq!(names(&fields), ["a", "b", "c"]);
    assert_eq!(fields[0].kind, FormFieldKind::Text);
    assert_eq!(fields[1].options, ["one", "two"]);
}

#[test]
fn recursive_forms_stop_cycles_and_depth() {
    let ids: Vec<Uuid> = (0..10).map(|_| Uuid::new_v4()).collect();
    let sources: HashMap<Uuid, String> = ids
        .iter()
        .enumerate()
        .map(|(i, id)| {
            (
                *id,
                format!(
                    "{{{{field name=f{i}}}}}{{{{snippet id=\"{}\"}}}}",
                    ids[(i + 1) % ids.len()]
                ),
            )
        })
        .collect();
    let fields = template::form_fields(&sources[&ids[0]], &|id| sources.get(&id).cloned());
    let expected: Vec<String> = (0..=8).map(|i| format!("f{i}")).collect();
    assert_eq!(
        names(&fields),
        expected.iter().map(String::as_str).collect::<Vec<_>>()
    );
    let first = ids[0];
    let once = template::form_fields(&format!("{{{{snippet id=\"{first}\"}}}}"), &|_| {
        Some(format!(
            "{{{{field name=once}}}}{{{{snippet id=\"{first}\"}}}}"
        ))
    });
    assert_eq!(names(&once), ["once"]);
}

#[test]
fn form_memory_validates_and_round_trips() {
    let id = Uuid::new_v4();
    let fields = template::analyze(&template::parse(
        "{{field name=text default=hello}}{{choice name=pick options=\"a|b\" default=b}}{{choice name=first options=\"x|y\"}}{{field name=empty}}",
    ))
    .form_fields;
    let mut memory = FormMemory::default();
    let expected: HashMap<String, String> = [
        ("text", "hello"),
        ("pick", "b"),
        ("first", "x"),
        ("empty", ""),
    ]
    .map(|(k, v)| (k.into(), v.into()))
    .into();
    assert_eq!(memory.initial_values(id, &fields), expected);
    memory.record(
        [("text", ""), ("pick", "removed"), ("first", "y")]
            .map(|(k, v)| (k.into(), v.into()))
            .into(),
        id,
    );
    let decoded: FormMemory =
        serde_json::from_str(&serde_json::to_string(&memory).unwrap()).unwrap();
    let expected: HashMap<String, String> =
        [("text", ""), ("pick", "b"), ("first", "y"), ("empty", "")]
            .map(|(k, v)| (k.into(), v.into()))
            .into();
    assert_eq!(decoded.initial_values(id, &fields), expected);
    memory.retain_snippets(&HashSet::from([Uuid::new_v4()]));
    assert_eq!(memory.initial_values(id, &fields)["text"], "hello");
}

#[test]
fn forms_ignore_missing_references_and_escaped_tags() {
    let source = format!(
        "\\{{{{field name=escaped}}}}{{{{snippet id=\"{}\"}}}}{{{{field name=visible}}}}",
        Uuid::new_v4()
    );
    assert_eq!(
        names(&template::form_fields(&source, &|_| None)),
        ["visible"]
    );
}

#[test]
fn form_memory_rejects_obsolete_choice_defaults() {
    let id = Uuid::new_v4();
    let fields = template::analyze(&template::parse(
        "{{choice name=pick options=\"a|b\" default=obsolete}}{{choice name=empty options=\"\"}}",
    ))
    .form_fields;
    let mut memory = FormMemory::default();
    memory.record(
        [("pick".to_string(), "also-obsolete".to_string())].into(),
        id,
    );
    assert_eq!(memory.initial_values(id, &fields)["pick"], "a");
    assert_eq!(memory.initial_values(id, &fields)["empty"], "");
}

#[test]
fn form_memory_is_bounded() {
    let mut memory = FormMemory::default();
    let mut ids: Vec<Uuid> = (0..FormMemory::CAPACITY).map(|_| Uuid::new_v4()).collect();
    for id in &ids {
        memory.record(HashMap::new(), *id);
    }
    ids.sort();
    let newcomer = Uuid::new_v4();
    memory.record(HashMap::new(), newcomer);
    assert_eq!(memory.values.len(), FormMemory::CAPACITY);
    assert!(memory.values.contains_key(&newcomer));
    assert!(!memory.values.contains_key(&ids[0]) || newcomer < ids[0]);
}
