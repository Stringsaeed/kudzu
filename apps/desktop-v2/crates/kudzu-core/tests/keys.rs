//! Fixture tests for key labels and the shortcut recorder against `keys.json`.

use std::path::PathBuf;

use kudzu_core::keys::{
    accelerator_from_event, accelerator_labels, key_labels, matches, spoken_shortcut, KeyEvent,
};
use serde::Deserialize;

fn load<T: for<'de> Deserialize<'de>>(name: &str) -> T {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/ts-parity")
        .join(name);
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

#[derive(Deserialize)]
struct KeysFixture {
    labels: Vec<LabelCase>,
    recorder: Vec<RecorderCase>,
}

#[derive(Deserialize)]
struct LabelCase {
    input: String,
    keys: Vec<String>,
    accelerator: Vec<String>,
}

#[derive(Deserialize)]
struct RecorderCase {
    event: FixtureEvent,
    result: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FixtureEvent {
    key: String,
    code: String,
    #[serde(default)]
    ctrl_key: bool,
    #[serde(default)]
    alt_key: bool,
    #[serde(default)]
    shift_key: bool,
    #[serde(default)]
    meta_key: bool,
}

impl From<&FixtureEvent> for KeyEvent {
    fn from(e: &FixtureEvent) -> Self {
        KeyEvent {
            key: e.key.clone(),
            code: e.code.clone(),
            ctrl: e.ctrl_key,
            alt: e.alt_key,
            shift: e.shift_key,
            meta: e.meta_key,
        }
    }
}

#[test]
fn labels_match_every_keys_fixture() {
    let fixture: KeysFixture = load("keys.json");
    assert_eq!(fixture.labels.len(), 5);
    for case in &fixture.labels {
        assert_eq!(
            key_labels(&case.input),
            case.keys,
            "key_labels({})",
            case.input
        );
        assert_eq!(
            accelerator_labels(&case.input),
            case.accelerator,
            "accelerator_labels({})",
            case.input
        );
    }
}

#[test]
fn recorder_matches_every_keys_fixture() {
    let fixture: KeysFixture = load("keys.json");
    assert_eq!(fixture.recorder.len(), 8);
    for case in &fixture.recorder {
        let event = KeyEvent::from(&case.event);
        assert_eq!(
            accelerator_from_event(&event),
            case.result,
            "accelerator_from_event({:?})",
            case.event.key
        );
    }
}

#[test]
fn spoken_labels_use_mac_words() {
    assert_eq!(spoken_shortcut("mod+enter"), "Command Return");
    assert_eq!(spoken_shortcut("mod+shift+p"), "Command Shift P");
    assert_eq!(spoken_shortcut("alt+up"), "Option Up Arrow");
}

#[test]
fn shift_plus_function_key_is_accepted() {
    let event = KeyEvent {
        key: "F5".into(),
        code: "F5".into(),
        ctrl: false,
        alt: false,
        shift: true,
        meta: false,
    };
    assert_eq!(accelerator_from_event(&event).as_deref(), Some("Shift+F5"));
}

#[test]
fn combo_matches_uses_physical_codes() {
    let cmd_k = KeyEvent {
        key: "k".into(),
        code: "KeyK".into(),
        ctrl: false,
        alt: false,
        shift: false,
        meta: true,
    };
    assert!(matches(&cmd_k, "mod+k"));
    let plain_k = KeyEvent {
        meta: false,
        ..cmd_k.clone()
    };
    assert!(!matches(&plain_k, "mod+k"));
    let space = KeyEvent {
        key: " ".into(),
        code: "Space".into(),
        ctrl: true,
        alt: true,
        shift: false,
        meta: false,
    };
    assert!(matches(&space, "ctrl+alt+space"));
}
