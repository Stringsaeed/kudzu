//! Engine behaviour that doesn't need a live event tap (never calls `start`).

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use kudzu_core::matching::MatchingConfig;
use kudzu_core::SnippetRecord;
use kudzu_engine::{Engine, EngineHost, EngineLibrary, EngineStatus, ExpansionEvent, FormRequest};

#[derive(Default)]
struct RecordingHost {
    statuses: Mutex<Vec<EngineStatus>>,
}

impl EngineHost for RecordingHost {
    fn status_changed(&self, status: EngineStatus) {
        self.statuses.lock().unwrap().push(status);
    }
    fn expanded(&self, _event: ExpansionEvent) {}
    fn form_requested(&self, _request: FormRequest) {}
}

fn library(snippets: Vec<SnippetRecord>) -> EngineLibrary {
    EngineLibrary {
        snippets,
        groups: vec![],
        config: MatchingConfig::default(),
        locale: "en_US".into(),
    }
}

#[test]
fn renders_nested_snippets_with_form_defaults() {
    let host = Arc::new(RecordingHost::default());
    let engine = Engine::new(host);
    let inner = SnippetRecord::new("inner", vec![], "Regards");
    let outer = SnippetRecord::new(
        "outer",
        vec![";sig".into()],
        format!("Hi {{{{field name=who default=there}}}}, {{{{choice name=tone options=\"warm|cool\"}}}}.{{{{cursor}}}} {{{{snippet id=\"{}\"}}}}", inner.id),
    );
    engine.reload(library(vec![outer.clone(), inner]));
    assert_eq!(
        engine.render_snippet(outer.id, HashMap::new()).as_deref(),
        Some("Hi there, warm. Regards")
    );
    let given = HashMap::from([("who".to_string(), "Sam".to_string())]);
    assert_eq!(
        engine.render_snippet(outer.id, given).as_deref(),
        Some("Hi Sam, warm. Regards")
    );
    assert_eq!(
        engine.render_snippet(uuid::Uuid::new_v4(), HashMap::new()),
        None
    );
}

#[test]
fn reload_replaces_the_library() {
    let engine = Engine::new(Arc::new(RecordingHost::default()));
    let mut snippet = SnippetRecord::new("s", vec!["s".into()], "one");
    engine.reload(library(vec![snippet.clone()]));
    // Identical reloads are no-ops; changed content is picked up.
    engine.reload(library(vec![snippet.clone()]));
    assert_eq!(
        engine.render_snippet(snippet.id, HashMap::new()).as_deref(),
        Some("one")
    );
    snippet.content = "two".into();
    engine.reload(library(vec![snippet.clone()]));
    assert_eq!(
        engine.render_snippet(snippet.id, HashMap::new()).as_deref(),
        Some("two")
    );
}

#[test]
fn pause_is_reported_once_per_change() {
    let host = Arc::new(RecordingHost::default());
    let engine = Engine::new(host.clone());
    engine.set_paused(true);
    engine.set_paused(true);
    assert!(engine.status().paused);
    assert_eq!(engine.status().summary, "Paused");
    engine.set_paused(false);
    let statuses = host.statuses.lock().unwrap();
    let paused: Vec<bool> = statuses.iter().map(|s| s.paused).collect();
    assert_eq!(paused, vec![true, false]);
}

struct HistoryHost;

impl EngineHost for HistoryHost {
    fn status_changed(&self, _status: EngineStatus) {}
    fn expanded(&self, _event: ExpansionEvent) {}
    fn form_requested(&self, _request: FormRequest) {}
    fn clipboard_history(&self, index: usize, _current: Option<&str>) -> Option<String> {
        (index == 1).then(|| "older".to_string())
    }
}

#[test]
fn renders_selection_history_and_keeps_key_presses_out_of_the_text() {
    use kudzu_core::template::{ActionKind, Key, RenderAction};
    let engine = Engine::new(Arc::new(HistoryHost));
    let snippet = SnippetRecord::new(
        "form",
        vec![";f".into()],
        "{{selection}}{{key name=tab count=2}}{{clipboard history=1}}/{{clipboard history=2}}{{delay ms=40}}{{key name=return}}",
    );
    engine.reload(library(vec![snippet.clone()]));
    // "Copy" and typed triggers have no selection: the tag renders empty.
    assert_eq!(
        engine.render_snippet(snippet.id, HashMap::new()).as_deref(),
        Some("older/")
    );
    let result = engine
        .render_snippet_with_selection(snippet.id, HashMap::new(), Some("picked".into()))
        .unwrap();
    assert_eq!(result.text, "pickedolder/");
    assert_eq!(
        result.actions,
        [
            RenderAction {
                offset: 6,
                action: ActionKind::Key {
                    key: Key::Tab,
                    count: 2
                }
            },
            RenderAction {
                offset: 12,
                action: ActionKind::Pause { ms: 40 }
            },
            RenderAction {
                offset: 12,
                action: ActionKind::Key {
                    key: Key::Return,
                    count: 1
                }
            },
        ]
    );
    // A host without a clipboard history renders history items empty.
    let plain = Engine::new(Arc::new(RecordingHost::default()));
    plain.reload(library(vec![snippet.clone()]));
    assert_eq!(
        plain.render_snippet(snippet.id, HashMap::new()).as_deref(),
        Some("/")
    );
}
