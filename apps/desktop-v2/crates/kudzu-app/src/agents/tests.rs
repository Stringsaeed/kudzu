use std::cell::RefCell;

use chrono::{TimeZone, Utc};
use serde_json::{json, Value};

use super::*;
use kudzu_core::clipboard::{classify, uti, ClipEntry, History};
use kudzu_core::KudzuLibrary;
use kudzu_core::{FireMode, GroupRecord, SnippetRecord};
use uuid::Uuid;

struct Fake {
    access: Access,
    library: RefCell<KudzuLibrary>,
    clips: Vec<ClipEntry>,
    clipboard: Option<String>,
    saves: RefCell<usize>,
    harness: RefCell<Option<super::test_support::Harness>>,
}

impl Fake {
    fn new() -> Self {
        let mut lib = KudzuLibrary::default();
        let work = GroupRecord::new("Work");
        let mut sig = SnippetRecord::new("Signature", vec![";sig".into()], "Best,\nSaeed");
        sig.group_id = Some(work.id);
        let wifi = SnippetRecord::new(
            "Wi-Fi",
            vec![";wifi".into()],
            "Network: Home\nPassword: Tr0ub4dor&3",
        );
        let greet = SnippetRecord::new(
            "Greeting",
            vec![";hi".into()],
            r#"Hi {{field name="who" default="there"}}, {{choice name="tone" options="thanks|cheers"}}!"#,
        );
        lib.groups.push(work);
        lib.snippets = vec![sig, wifi, greet];
        for snippet in &mut lib.snippets {
            snippet.created_at = Utc.with_ymd_and_hms(2026, 11, 8, 9, 0, 0).unwrap();
            snippet.updated_at = snippet.created_at;
        }
        Self {
            access: Access {
                clipboard: true,
                write: true,
                trigger_prefix: ";".into(),
            },
            library: RefCell::new(lib),
            clips: vec![
                clip("meeting notes for monday", 1),
                clip(concat!("ghp", "_wWPw5k4aXcaT4fNP0UcnZwJUVFk6LO0pINUx"), 2),
                clip("P@ssw0rd!", 3),
            ],
            clipboard: None,
            saves: RefCell::new(0),
            harness: RefCell::new(None),
        }
    }

    fn snippet(&self, trigger: &str) -> SnippetRecord {
        self.library
            .borrow()
            .snippets
            .iter()
            .find(|s| s.triggers.iter().any(|t| t == trigger))
            .cloned()
            .unwrap()
    }
}

fn clip(text: &str, minute: u32) -> ClipEntry {
    let items = vec![vec![(
        uti::PLAIN_TEXT.to_string(),
        text.as_bytes().to_vec(),
    )]];
    let c = classify(&items, None).unwrap();
    let mut h = History::new();
    h.ingest(
        &c,
        None,
        None,
        Utc.with_ymd_and_hms(2026, 11, 8, 9, minute, 0).unwrap(),
    )
    .entry()
    .clone()
}

trait Rpc {
    fn handle(&self, version: &str, message: &str) -> Option<String>;
}
struct Offline;
impl Rpc for Offline {
    fn handle(&self, version: &str, message: &str) -> Option<String> {
        super::handle(None, version, message)
    }
}
impl Rpc for super::service::ServiceHost {
    fn handle(&self, version: &str, message: &str) -> Option<String> {
        super::handle(Some(self), version, message)
    }
}
impl Rpc for Fake {
    fn handle(&self, version: &str, message: &str) -> Option<String> {
        let mut slot = self.harness.borrow_mut();
        let h = slot.get_or_insert_with(|| {
            super::test_support::Harness::new(self.library.borrow().clone(), self.clips.clone())
        });
        h.service
            .host
            .library
            .transact(|lib| {
                *lib = self.library.borrow().clone();
                Ok(())
            })
            .unwrap();
        h.settings
            .replace(std::sync::Arc::new(crate::settings::Settings {
                agent_access: true,
                agent_write: self.access.write,
                agent_clipboard: self.access.clipboard,
                default_trigger_prefix: self.access.trigger_prefix.clone(),
                ..Default::default()
            }))
            .notification
            .notify();
        std::sync::Arc::get_mut(&mut h.service.host)
            .unwrap()
            .render_override = Some(RenderEnv {
            now: chrono::FixedOffset::east_opt(0)
                .unwrap()
                .with_ymd_and_hms(2026, 11, 8, 12, 0, 0)
                .unwrap(),
            locale: "en_US".into(),
            clipboard: self.clipboard.clone(),
        });
        let before = h.service.host.library.snapshot().0;
        let result = super::handle(Some(h.service.host.as_ref()), version, message);
        let (after, lib) = h.service.host.library.snapshot();
        if after != before {
            *self.library.borrow_mut() = (*lib).clone();
            *self.saves.borrow_mut() += 1;
        }
        result
    }
}

fn rpc(host: &dyn Rpc, message: Value) -> Value {
    let out = host.handle("1.2.3", &message.to_string()).expect("a reply");
    serde_json::from_str(&out).unwrap()
}

/// Calls a tool and returns (structured result or error text, is_error).
fn tool(host: &dyn Rpc, name: &str, args: Value) -> (Value, bool) {
    let reply = rpc(
        host,
        json!({ "jsonrpc": "2.0", "id": 7, "method": "tools/call", "params": { "name": name, "arguments": args } }),
    );
    let result = &reply["result"];
    let is_error = result["isError"].as_bool().unwrap();
    if is_error {
        (result["content"][0]["text"].clone(), true)
    } else {
        (result["structuredContent"].clone(), false)
    }
}

fn names(tools: &Value) -> Vec<String> {
    tools["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap().to_string())
        .collect()
}

#[test]
fn initialize_negotiates_the_protocol() {
    let host = Fake::new();
    let r = rpc(
        &host,
        json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": { "protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": { "name": "t", "version": "1" } } }),
    );
    assert_eq!(r["id"], 1);
    assert_eq!(r["result"]["protocolVersion"], "2025-06-18");
    assert_eq!(r["result"]["serverInfo"]["version"], "1.2.3");
    assert!(r["result"]["capabilities"]["tools"].is_object());
    let r = rpc(
        &host,
        json!({ "jsonrpc": "2.0", "id": 2, "method": "initialize", "params": { "protocolVersion": "1999-01-01" } }),
    );
    assert_eq!(r["result"]["protocolVersion"], PROTOCOL_VERSIONS[0]);
}

#[test]
fn serves_the_skill_as_a_resource() {
    for host in [&Fake::new() as &dyn Rpc, &Offline] {
        let list = rpc(
            host,
            json!({ "jsonrpc": "2.0", "id": 1, "method": "resources/list" }),
        );
        assert_eq!(list["result"]["resources"][0]["uri"], SKILL_URI);
        let read = rpc(
            host,
            json!({ "jsonrpc": "2.0", "id": 2, "method": "resources/read", "params": { "uri": SKILL_URI } }),
        );
        assert_eq!(read["result"]["contents"][0]["text"], SKILL);
        assert!(SKILL.starts_with("---\nname: kudzu"));
        let missing = rpc(
            host,
            json!({ "jsonrpc": "2.0", "id": 3, "method": "resources/read", "params": { "uri": "kudzu://nope" } }),
        );
        assert_eq!(missing["error"]["code"], -32002);
    }
}

#[test]
fn notifications_and_responses_get_no_reply() {
    let host = Fake::new();
    for msg in [
        json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }),
        json!({ "jsonrpc": "2.0", "method": "tools/list" }),
        json!({ "jsonrpc": "2.0", "id": 4, "result": {} }),
    ] {
        let line = msg.to_string();
        assert!(host.handle("1", &line).is_none(), "{line}");
        assert!(!expects_reply(&line), "{line}");
    }
    for line in [
        "{nope",
        "[1,2]",
        r#"{"jsonrpc":"2.0","id":3,"method":"nope"}"#,
    ] {
        assert!(expects_reply(line));
        let r: Value = serde_json::from_str(&host.handle("1", line).unwrap()).unwrap();
        assert!(r["error"]["code"].as_i64().unwrap() < 0, "{line}");
    }
    let r = rpc(
        &host,
        json!({ "jsonrpc": "2.0", "id": "p", "method": "ping" }),
    );
    assert_eq!(r, json!({ "jsonrpc": "2.0", "id": "p", "result": {} }));
}

#[test]
fn tools_follow_access() {
    let mut host = Fake::new();
    let list = json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/list" });
    assert_eq!(names(&rpc(&host, list.clone())).len(), 8);
    host.access = Access {
        clipboard: false,
        write: false,
        trigger_prefix: ";".into(),
    };
    assert_eq!(
        names(&rpc(&host, list.clone())),
        [
            "list_snippets",
            "get_snippet",
            "expand_snippet",
            "list_groups"
        ]
    );
    let (msg, err) = tool(&host, "list_clipboard", json!({}));
    assert!(err && msg.as_str().unwrap().contains("Settings"));
    let (_, err) = tool(
        &host,
        "create_snippet",
        json!({ "name": "x", "content": "y", "triggers": [";y"] }),
    );
    assert!(err);
    assert_eq!(*host.saves.borrow(), 0);
    // An unknown tool is a protocol error.
    let r = rpc(
        &host,
        json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": { "name": "rm_rf" } }),
    );
    assert_eq!(r["error"]["code"], -32602);
}

#[test]
fn offline_lists_tools_and_explains_calls() {
    let r = rpc(
        &Offline,
        json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/list" }),
    );
    assert_eq!(names(&r).len(), 8);
    let (msg, err) = tool(&Offline, "list_snippets", json!({}));
    assert!(err);
    assert_eq!(msg, OFFLINE_MESSAGE);
}

#[test]
fn lists_and_searches_snippets_without_leaking_secrets() {
    let host = Fake::new();
    let (all, _) = tool(&host, "list_snippets", json!({}));
    assert_eq!(all["total"], 3);
    assert_eq!(all["triggerPrefix"], ";");
    let wifi = all["snippets"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["name"] == "Wi-Fi")
        .unwrap();
    assert_eq!(
        wifi["preview"],
        "Network: Home Password: [redacted: password]"
    );
    assert_eq!(wifi["redacted"], json!(["password"]));
    assert!(!all.to_string().contains("Tr0ub4dor"));

    // Searching for the secret finds nothing.
    let (found, _) = tool(&host, "list_snippets", json!({ "query": "Tr0ub4dor" }));
    assert_eq!(found["total"], 0);
    let (found, _) = tool(&host, "list_snippets", json!({ "query": "best" }));
    assert_eq!(found["snippets"][0]["triggers"], json!([";sig"]));
    let (found, _) = tool(&host, "list_snippets", json!({ "group": "work" }));
    assert_eq!(found["total"], 1);
    assert_eq!(found["snippets"][0]["group"], "Work");
}

#[test]
fn reads_a_snippet_by_trigger() {
    let host = Fake::new();
    let (s, err) = tool(&host, "get_snippet", json!({ "trigger": ";HI" }));
    assert!(!err, "{s}");
    assert_eq!(s["name"], "Greeting");
    assert_eq!(s["form"], true);
    assert_eq!(s["fields"].as_array().unwrap().len(), 2);
    let (s, _) = tool(&host, "get_snippet", json!({ "trigger": ";wifi" }));
    assert_eq!(
        s["content"],
        "Network: Home\nPassword: [redacted: password]"
    );
    assert!(s["note"].is_string());
    let (msg, err) = tool(&host, "get_snippet", json!({ "id": "nope" }));
    assert!(err && msg.as_str().unwrap().contains("isn't a snippet id"));
}

#[test]
fn expands_with_values_defaults_and_a_scrubbed_clipboard() {
    let mut host = Fake::new();
    let (r, _) = tool(&host, "expand_snippet", json!({ "trigger": ";hi" }));
    assert_eq!(r["text"], "Hi there, thanks!");
    let (r, _) = tool(
        &host,
        "expand_snippet",
        json!({ "trigger": ";hi", "values": { "who": "Ana", "tone": "cheers", "extra": "1" } }),
    );
    assert_eq!(r["text"], "Hi Ana, cheers!");
    assert_eq!(r["unknownFields"], json!(["extra"]));
    let (msg, err) = tool(
        &host,
        "expand_snippet",
        json!({ "trigger": ";hi", "values": { "tone": "rude" } }),
    );
    assert!(err && msg.as_str().unwrap().contains("thanks, cheers"));

    // A password on the clipboard is scrubbed before it's embedded in other text.
    let id = {
        let mut lib = host.library.borrow_mut();
        let s = SnippetRecord::new("Paste", vec![";p".into()], "Here: {{clipboard}} ok");
        let id = s.id;
        lib.snippets.push(s);
        id
    };
    host.clipboard = Some("P@ssw0rd!".into());
    let (r, _) = tool(&host, "expand_snippet", json!({ "id": id.to_string() }));
    assert_eq!(r["text"], "Here: [redacted: password] ok");

    // So is a secret in a nested snippet.
    let wifi = host.snippet(";wifi").id;
    let id = {
        let mut lib = host.library.borrow_mut();
        let s = SnippetRecord::new(
            "Nest",
            vec![";n".into()],
            format!(r#"Guest info: {{{{snippet id="{wifi}"}}}}"#),
        );
        let id = s.id;
        lib.snippets.push(s);
        id
    };
    let (r, _) = tool(&host, "expand_snippet", json!({ "id": id.to_string() }));
    assert_eq!(
        r["text"],
        "Guest info: Network: Home\nPassword: [redacted: password]"
    );
    assert!(r["note"].is_string());
}

#[test]
fn creates_snippets_and_groups() {
    let host = Fake::new();
    let (r, err) = tool(
        &host,
        "create_snippet",
        json!({ "name": "Address", "content": "1 Main St", "triggers": [" ;addr "], "group": "Personal/Home", "fireMode": "wholeWord" }),
    );
    assert!(!err, "{r}");
    assert_eq!(r["created"]["triggers"], json!([";addr"]));
    assert_eq!(r["created"]["group"], "Personal/Home");
    let s = host.snippet(";addr");
    assert_eq!(s.fire_mode, FireMode::WholeWord);
    assert_eq!(host.library.borrow().groups.len(), 3);
    assert_eq!(
        host.library.borrow().snippets[0].id,
        s.id,
        "new snippets go first"
    );

    // Existing groups are reused by name.
    let (r, _) = tool(
        &host,
        "create_snippet",
        json!({ "name": "Phone", "content": "555", "triggers": [";ph"], "group": "home" }),
    );
    assert_eq!(r["created"]["group"], "Personal/Home");
    assert_eq!(host.library.borrow().groups.len(), 3);
}

#[test]
fn rejects_bad_creates_without_saving() {
    let host = Fake::new();
    for (args, needle) in [
        (
            json!({ "name": "Dup", "content": "x", "triggers": [";SIG"] }),
            "already the trigger of “Signature”",
        ),
        (
            json!({ "name": "No trigger", "content": "x", "triggers": [] }),
            "at least one trigger",
        ),
        (
            json!({ "name": "Blank", "content": "x", "triggers": ["   "] }),
            "only spaces",
        ),
        (
            json!({ "name": "", "content": "x", "triggers": [";a"] }),
            "name",
        ),
        (
            json!({ "name": "Marker", "content": "pw: [redacted: password]", "triggers": [";m"] }),
            "marker",
        ),
        (
            json!({ "name": "Mode", "content": "x", "triggers": [";q"], "fireMode": "later" }),
            "fireMode",
        ),
    ] {
        let (msg, err) = tool(&host, "create_snippet", args.clone());
        assert!(err, "{args}");
        assert!(
            msg.as_str().unwrap().contains(needle),
            "{msg} lacks {needle}"
        );
    }
    assert_eq!(*host.saves.borrow(), 0);
    assert_eq!(host.library.borrow().snippets.len(), 3);
}

#[test]
fn warns_about_shadowed_triggers() {
    let host = Fake::new();
    let (r, _) = tool(
        &host,
        "create_snippet",
        json!({ "name": "Sign off", "content": "Cheers", "triggers": [";si"] }),
    );
    assert!(r["warnings"][0].as_str().unwrap().contains(";sig"), "{r}");
}

#[test]
fn updates_only_what_is_passed_and_protects_secrets() {
    let host = Fake::new();
    let sig = host.snippet(";sig");
    let (r, err) = tool(
        &host,
        "update_snippet",
        json!({ "id": sig.id.to_string(), "content": "Thanks,\nSaeed", "favorite": true, "group": null }),
    );
    assert!(!err, "{r}");
    let after = host.snippet(";sig");
    assert_eq!(after.content, "Thanks,\nSaeed");
    assert!(after.is_favorite);
    assert_eq!(after.group_id, None);
    assert_eq!(after.name, "Signature");
    assert!(after.updated_at >= sig.updated_at);

    // A snippet with a secret keeps its content, but other fields can change.
    let wifi = host.snippet(";wifi");
    let (msg, err) = tool(
        &host,
        "update_snippet",
        json!({ "id": wifi.id.to_string(), "content": "Network: Home\nPassword: [redacted: password]" }),
    );
    assert!(err && msg.as_str().unwrap().contains("only be changed in Kudzu"));
    let (_, err) = tool(
        &host,
        "update_snippet",
        json!({ "id": wifi.id.to_string(), "triggers": [";wf"] }),
    );
    assert!(!err);
    assert_eq!(host.snippet(";wf").content, wifi.content);

    // Formatted snippets keep their content too.
    {
        let mut lib = host.library.borrow_mut();
        let s = lib.snippets.iter_mut().find(|s| s.id == sig.id).unwrap();
        s.formatting = Some(kudzu_core::formatting::Formatting {
            source: s.content.clone(),
            spans: vec![],
        });
    }
    let (msg, err) = tool(
        &host,
        "update_snippet",
        json!({ "id": sig.id.to_string(), "content": "new" }),
    );
    assert!(err && msg.as_str().unwrap().contains("formatted"));

    // Taking another snippet's trigger fails.
    let (msg, err) = tool(
        &host,
        "update_snippet",
        json!({ "id": sig.id.to_string(), "triggers": [";hi"] }),
    );
    assert!(err && msg.as_str().unwrap().contains("already the trigger"));
}

#[test]
fn expand_leaves_out_key_presses_and_guards_clipboard_history() {
    let mut host = Fake::new();
    let add = |host: &Fake, content: &str| {
        let s = SnippetRecord::new("New", vec![], content);
        let id = s.id;
        host.library.borrow_mut().snippets.push(s);
        id.to_string()
    };
    // Keys and pauses are dropped from the text and listed; the selection is empty.
    let id = add(
        &host,
        "né{{key name=tab count=2}}[{{selection}}]{{delay ms=300}}{{key name=return}}",
    );
    let (r, _) = tool(&host, "expand_snippet", json!({ "id": id }));
    assert_eq!(r["text"], "né[]");
    assert_eq!(
        r["skippedActions"],
        json!([
            { "afterCharacters": 2, "action": "press tab ×2" },
            { "afterCharacters": 4, "action": "wait 300 ms" },
            { "afterCharacters": 4, "action": "press return" },
        ])
    );
    assert!(r["actionsNote"].is_string());
    let (r, _) = tool(&host, "expand_snippet", json!({ "trigger": ";sig" }));
    assert!(r.get("skippedActions").is_none() && r.get("actionsNote").is_none());

    // UUIDs and random picks are real values.
    let id = add(&host, r#"{{uuid}} {{random options="a|b"}}"#);
    let (r, _) = tool(&host, "expand_snippet", json!({ "id": id }));
    let text = r["text"].as_str().unwrap();
    let (uuid, pick) = text.split_once(' ').unwrap();
    assert!(Uuid::parse_str(uuid).is_ok_and(|u| !u.is_nil()));
    assert!(pick == "a" || pick == "b");

    // History, newest first: a password, an API key, then the notes. Nothing recorded is on the
    // clipboard now, so `history=1` is the newest entry. Secrets render empty.
    let id = add(&host, "[{{clipboard history=1}}][{{clipboard history=2}}][{{clipboard history=3}}][{{clipboard history=4}}]");
    let (r, _) = tool(&host, "expand_snippet", json!({ "id": id }));
    assert_eq!(r["text"], "[][][meeting notes for monday][]");
    // When the newest entry is what's on the clipboard, it is skipped.
    host.clipboard = Some("P@ssw0rd!".into());
    let (r, _) = tool(&host, "expand_snippet", json!({ "id": id }));
    assert_eq!(r["text"], "[][meeting notes for monday][][]");
    // Without clipboard access the history isn't read at all.
    host.access.clipboard = false;
    let (r, _) = tool(&host, "expand_snippet", json!({ "id": id }));
    assert_eq!(r["text"], "[][][][]");
}

#[test]
fn clipboard_withholds_secrets() {
    let host = Fake::new();
    let (r, _) = tool(&host, "list_clipboard", json!({}));
    let items = r["items"].as_array().unwrap();
    assert_eq!(items.len(), 3);
    // Newest first.
    assert_eq!(items[0]["text"], Value::Null);
    assert!(items[0]["withheld"].as_str().unwrap().contains("password"));
    assert!(
        items[0].get("length").is_none() && items[0].get("lines").is_none(),
        "withheld items don't reveal their length"
    );
    assert_eq!(items[1]["text"], Value::Null);
    assert_eq!(items[1]["redacted"], json!(["API key"]));
    assert_eq!(items[2]["text"], "meeting notes for monday");
    assert!(!r.to_string().contains("P@ssw0rd") && !r.to_string().contains("ghp_"));

    let (r, _) = tool(&host, "list_clipboard", json!({ "query": "ghp_" }));
    assert_eq!(r["total"], 0);
    let (r, _) = tool(&host, "list_clipboard", json!({ "query": "monday" }));
    assert_eq!(r["total"], 1);

    let id = host.clips[1].id.to_string();
    let (r, _) = tool(&host, "get_clipboard_item", json!({ "id": id }));
    assert_eq!(r["text"], Value::Null);
    let id = host.clips[0].id.to_string();
    let (r, _) = tool(&host, "get_clipboard_item", json!({ "id": id }));
    assert_eq!(r["text"], "meeting notes for monday");
}

#[test]
fn clipboard_follows_the_users_corrections() {
    let mut host = Fake::new();
    host.clips[2].secret_override = Some(false); // "P@ssw0rd!" is fine to share
    host.clips[0].secret_override = Some(true); // hide the meeting notes
    let (r, _) = tool(&host, "list_clipboard", json!({}));
    let items = r["items"].as_array().unwrap();
    assert_eq!(items[0]["text"], "P@ssw0rd!");
    assert!(items[0].get("redacted").is_none());
    assert_eq!(items[2]["text"], Value::Null);
    assert!(items[2]["withheld"].as_str().unwrap().contains("marked"));
    let (r, _) = tool(&host, "list_clipboard", json!({ "query": "monday" }));
    assert_eq!(r["total"], 0);
}

#[test]
fn field_defaults_and_options_are_redacted_too() {
    let host = Fake::new();
    let token = concat!("ghp", "_wWPw5k4aXcaT4fNP0UcnZwJUVFk6LO0pINUx");
    {
        let mut lib = host.library.borrow_mut();
        let content = format!(
            r#"Key: {{{{field name="key" default="{token}"}}}} via {{{{choice name="env" options="dev|{token}"}}}}"#
        );
        lib.snippets
            .push(SnippetRecord::new("Deploy", vec![";dep".into()], content));
    }
    for tool_name in ["get_snippet", "expand_snippet", "list_snippets"] {
        let args = if tool_name == "list_snippets" {
            json!({})
        } else {
            json!({ "trigger": ";dep" })
        };
        let (r, err) = tool(&host, tool_name, args);
        assert!(!err, "{r}");
        assert!(
            !r.to_string().contains("wWPw5k4a"),
            "{tool_name} leaked a secret: {r}"
        );
    }
}

#[test]
fn socket_lifecycle_access_and_setup_are_isolated() {
    use super::test_support::Harness;
    use std::{
        fs,
        io::{BufRead, BufReader, Write},
        os::unix::{
            fs::PermissionsExt,
            net::{UnixListener, UnixStream},
        },
        time::Duration,
    };
    let h = Harness::new(KudzuLibrary::default(), vec![]);
    let path = &h.service.config.socket;
    // A stale endpoint is replaced.
    drop(UnixListener::bind(path).unwrap());
    h.service.apply().unwrap();
    assert_eq!(
        fs::metadata(path).unwrap().permissions().mode() & 0o777,
        0o600
    );
    let other = AgentsService::new(
        h.service.config.clone(),
        h.service.host.library.clone(),
        h.service.host.clipboard.clone(),
        h.settings.watch(),
        std::sync::Arc::new(crate::fixtures::clock::FakeClock::default()),
        std::sync::Arc::new(crate::fixtures::ids::FixedIds::new(200)),
    );
    assert!(other.apply().is_err());
    let mut client = UnixStream::connect(path).unwrap();
    client
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let mut reader = BufReader::new(client.try_clone().unwrap());
    writeln!(client, "{{\"id\":1,\"method\":\"ping\"}}").unwrap();
    let mut line = String::new();
    reader.read_line(&mut line).unwrap();
    assert!(line.contains("result"));
    h.settings
        .replace(std::sync::Arc::new(crate::settings::Settings {
            agent_access: false,
            ..Default::default()
        }))
        .notification
        .notify();
    writeln!(client, "{{\"id\":2,\"method\":\"ping\"}}").unwrap();
    line.clear();
    assert_eq!(reader.read_line(&mut line).unwrap(), 0);
    h.service.apply().unwrap();
    assert!(!path.exists());
    assert!(!h.service.status().get().running);
    assert_eq!(h.service.setup().skill, SkillState::Missing);
    let setup = h.service.setup();
    assert!(setup
        .claude_command
        .starts_with("claude mcp add --scope user kudzu-next -- '"));
    let config: Value = serde_json::from_str(&setup.config_json).unwrap();
    assert_eq!(config["mcpServers"]["kudzu-next"]["args"], json!(["--mcp"]));
    let installed = h.service.install_skill().unwrap();
    assert!(installed.starts_with(&h.dir));
    assert_eq!(h.service.setup().skill, SkillState::Current);
    fs::write(installed, "old version").unwrap();
    assert_eq!(h.service.setup().skill, SkillState::Outdated);
    h.service.install_skill().unwrap();
    assert_eq!(h.service.setup().skill, SkillState::Current);
}

#[test]
fn service_gates_are_independent_and_recording_is_required() {
    use super::test_support::Harness;
    for (write, clipboard, recording) in [
        (false, true, true),
        (true, false, true),
        (true, true, false),
    ] {
        let h = Harness::new(KudzuLibrary::default(), vec![clip("notes", 1)]);
        h.settings
            .replace(std::sync::Arc::new(crate::settings::Settings {
                agent_access: true,
                agent_write: write,
                agent_clipboard: clipboard,
                clipboard_enabled: recording,
                ..Default::default()
            }))
            .notification
            .notify();
        let (_, err) = tool(
            h.service.host.as_ref(),
            "create_snippet",
            json!({"name":"Note","content":"hello","triggers":[";note"]}),
        );
        assert_eq!(err, !write);
        let (_, err) = tool(h.service.host.as_ref(), "list_clipboard", json!({}));
        assert_eq!(err, !(clipboard && recording));
    }
}

#[test]
fn racing_agent_and_ui_edits_preserve_both_and_collision_rolls_back_group() {
    let h = super::test_support::Harness::new(KudzuLibrary::default(), vec![]);
    std::thread::scope(|scope| {
        let host = h.service.host.clone();
        scope.spawn(move || {
            let (_, err) = tool(
                host.as_ref(),
                "create_snippet",
                json!({"name":"Agent","content":"a","triggers":[";a"],"group":"Agent/Group"}),
            );
            assert!(!err);
        });
        h.service
            .host
            .library
            .transact(|lib| {
                lib.snippets
                    .push(SnippetRecord::new("UI", vec![";ui".into()], "ui"));
                Ok(())
            })
            .unwrap();
    });
    let before = h.service.host.library.snapshot();
    assert_eq!(before.1.snippets.len(), 2);
    let (_, err) = tool(
        h.service.host.as_ref(),
        "create_snippet",
        json!({"name":"Collision","content":"b","triggers":[";UI"],"group":"Unwanted/Group"}),
    );
    assert!(err);
    assert_eq!(h.service.host.library.snapshot(), before);
    let id = before
        .1
        .snippets
        .iter()
        .find(|s| s.name == "Agent")
        .unwrap()
        .id;
    let (_, err) = tool(
        h.service.host.as_ref(),
        "update_snippet",
        json!({"id":id,"triggers":[";ui"],"group":"Unwanted/Group"}),
    );
    assert!(err);
    assert_eq!(h.service.host.library.snapshot(), before);
}

#[test]
fn secrets_never_reach_results_errors_or_logging_calls() {
    let token = concat!("ghp", "_wWPw5k4aXcaT4fNP0UcnZwJUVFk6LO0pINUx");
    let host = Fake::new();
    for (name, args) in [
        ("get_snippet", json!({"id":token})),
        ("update_snippet", json!({"id":token})),
        ("get_clipboard_item", json!({"id":token})),
    ] {
        let result = rpc(
            &host,
            json!({"id":1,"method":"tools/call","params":{"name":name,"arguments":args}}),
        );
        assert!(!result.to_string().contains("wWPw5k4a"));
    }
    // This adapter has no logging sink at all, including transport failures.
    for source in [
        include_str!("mod.rs"),
        include_str!("service.rs"),
        include_str!("socket.rs"),
        include_str!("bridge.rs"),
        include_str!("setup.rs"),
        include_str!("tools/mod.rs"),
        include_str!("tools/snippets.rs"),
        include_str!("tools/clipboard.rs"),
    ] {
        assert!(!source.contains("log::"));
    }
}

#[test]
fn replays_client_protocol_transcript() {
    let h = super::test_support::Harness::new(KudzuLibrary::default(), vec![]);
    let input = include_str!("fixtures/session.jsonl");
    h.service.apply().unwrap();
    let mut output = vec![];
    bridge::run(
        std::io::Cursor::new(input),
        &mut output,
        &h.service.config.socket,
        "1.2.3",
    )
    .unwrap();
    let replies: Vec<Value> = String::from_utf8(output)
        .unwrap()
        .lines()
        .map(|s| serde_json::from_str(s).unwrap())
        .collect();
    assert_eq!(replies.len(), 8);
    assert_eq!(replies[3]["result"]["isError"], false);
    assert_eq!(replies[4]["result"], json!({}));
    assert_eq!(replies[5]["result"]["resources"][0]["uri"], SKILL_URI);
    assert_eq!(replies[6]["result"]["contents"][0]["text"], SKILL);
    assert_eq!(
        replies[7]["result"]["structuredContent"]["content"],
        "Transcript text"
    );
    assert_eq!(replies[0]["result"]["protocolVersion"], "2025-06-18");
    assert_eq!(replies[1]["result"]["tools"].as_array().unwrap().len(), 8);
    assert_eq!(replies[2]["result"]["structuredContent"]["total"], 0);
}

#[test]
fn tool_clipboard_limits_remain_independent_of_service_caps() {
    let clips = (0..130)
        .map(|i| {
            let mut entry = clip(&format!("ordinary note {i}"), 1);
            entry.id = Uuid::new_v4();
            entry
        })
        .collect();
    let h = super::test_support::Harness::new(KudzuLibrary::default(), clips);
    let (result, err) = tool(
        h.service.host.as_ref(),
        "list_clipboard",
        json!({"limit":900}),
    );
    assert!(!err);
    assert_eq!(result["returned"], 100);
    let (result, err) = tool(h.service.host.as_ref(), "list_clipboard", json!({}));
    assert!(!err);
    assert_eq!(result["returned"], 20);
    let (result, err) = tool(
        h.service.host.as_ref(),
        "list_clipboard",
        json!({"limit":1}),
    );
    assert!(!err);
    assert_eq!(result["returned"], 1);
    let text = "ordinary word ".repeat(6000);
    let mut entry = clip(&text, 1);
    entry.text = Some(text);
    entry.secret_override = Some(false); // Explicitly share this oversized fixture without a backing blob.
    let id = entry.id;
    let h = super::test_support::Harness::new(KudzuLibrary::default(), vec![entry]);
    let (result, err) = tool(
        h.service.host.as_ref(),
        "get_clipboard_item",
        json!({"id":id}),
    );
    assert!(!err);
    assert!(result["text"].as_str().unwrap().chars().count() <= 50_000);
    assert_eq!(result["truncated"], true);
}

#[test]
fn snippet_metadata_and_unknown_field_names_match_tauri() {
    let token = concat!("ghp", "_wWPw5k4aXcaT4fNP0UcnZwJUVFk6LO0pINUx");
    let host = Fake::new();
    host.library.borrow_mut().snippets[0].name = token.into();
    for (name, args) in [
        ("list_snippets", json!({})),
        (
            "expand_snippet",
            json!({"trigger":";hi","values":{token:"unused"}}),
        ),
    ] {
        let (output, err) = tool(&host, name, args);
        assert!(!err);
        assert!(output.to_string().contains("wWPw5k4a"));
    }
}

#[test]
fn tauri_clean_template_fields_survive_get_update_round_trip() {
    let host = Fake::new();
    let body = r#"Hello {{field name="Code" default="P@ssw0rd!"}}"#;
    assert!(kudzu_core::secrets::redact(body).is_clean());
    let s = SnippetRecord::new("Round trip", vec![";round".into()], body);
    let id = s.id;
    host.library.borrow_mut().snippets.push(s);
    let (result, err) = tool(&host, "get_snippet", json!({"id":id}));
    assert!(!err);
    assert_eq!(result["fields"][0]["defaultValue"], "P@ssw0rd!");
    let (_, err) = tool(
        &host,
        "update_snippet",
        json!({"id":id,"content":result["content"]}),
    );
    assert!(!err);
    assert_eq!(host.snippet(";round").content, body);
}
#[test]
fn protocol_errors_include_bounded_escaped_names_and_valid_kinds() {
    let host = Fake::new();
    let name = "unknown\n\"method";
    let reply = rpc(&host, json!({"id":1,"method":name}));
    assert!(reply["error"]["message"]
        .as_str()
        .unwrap()
        .contains("unknown\\n\\\"method"));
    let reply = rpc(
        &host,
        json!({"id":1,"method":"tools/call","params":{"name":"z".repeat(500)}}),
    );
    let message = reply["error"]["message"].as_str().unwrap();
    assert!(message.contains(&"z".repeat(64)));
    assert!(message.len() < 200);
    let (message, err) = tool(&host, "list_clipboard", json!({"kind":"nope"}));
    assert!(err);
    assert!(message
        .as_str()
        .unwrap()
        .contains("text, code, link, color, image, file or other"));
}

#[test]
fn agent_records_use_the_injected_identity_and_clock() {
    let h = super::test_support::Harness::new(KudzuLibrary::default(), vec![]);
    let (value, err) = tool(
        h.service.host.as_ref(),
        "create_snippet",
        json!({"name":"Clock","content":"ordinary text","triggers":[";clock"],"group":"One/Two"}),
    );
    assert!(!err);
    assert_eq!(value["created"]["id"], Uuid::from_u128(100).to_string());
    let lib = h.service.host.library.snapshot().1;
    assert_eq!(lib.groups[0].id, Uuid::from_u128(101));
    assert_eq!(lib.groups[1].id, Uuid::from_u128(102));
    assert_eq!(lib.snippets[0].created_at.timestamp(), 1_794_398_400);
    let (value, err) = tool(
        h.service.host.as_ref(),
        "update_snippet",
        json!({"id":Uuid::from_u128(100),"name":"Clock updated"}),
    );
    assert!(!err, "{value}");
    assert_eq!(
        h.service.host.library.snapshot().1.snippets[0]
            .updated_at
            .timestamp(),
        1_794_398_400
    );
}
#[test]
fn fixture_directories_are_absolute_and_under_the_workspace_target() {
    let dir = super::test_support::directory();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .unwrap()
        .join("target");
    let absolute = dir.is_absolute();
    let inside = dir.starts_with(root);
    std::fs::remove_dir_all(dir).unwrap();
    assert!(absolute && inside);
}
#[test]
fn installed_skill_is_private_atomic_and_replaces_symlink() {
    use std::os::unix::fs::{symlink, PermissionsExt};
    let h = super::test_support::Harness::new(KudzuLibrary::default(), vec![]);
    let path = h.service.setup().skill_path;
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let target = h.dir.join("original");
    std::fs::write(&target, "original").unwrap();
    symlink(&target, &path).unwrap();
    h.service.install_skill().unwrap();
    assert!(!std::fs::symlink_metadata(&path)
        .unwrap()
        .file_type()
        .is_symlink());
    assert_eq!(std::fs::read_to_string(target).unwrap(), "original");
    assert_eq!(
        std::fs::metadata(path).unwrap().permissions().mode() & 0o777,
        0o600
    );
}

#[test]
fn fake_reuses_one_service_across_messages() {
    let host = Fake::new();
    rpc(&host, json!({"id":1,"method":"ping"}));
    let before = host.harness.borrow().as_ref().unwrap().dir.clone();
    rpc(&host, json!({"id":2,"method":"tools/list"}));
    assert_eq!(host.harness.borrow().as_ref().unwrap().dir, before);
}
