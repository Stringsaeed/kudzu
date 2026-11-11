use std::cell::RefCell;

use chrono::{TimeZone, Utc};
use serde_json::{json, Value};

use super::*;
use crate::clipboard::{classify, uti, History};
use crate::{FireMode, GroupRecord, SnippetRecord};

struct Fake {
    access: Access,
    library: RefCell<KudzuLibrary>,
    clips: Vec<ClipEntry>,
    clipboard: Option<String>,
    saves: RefCell<usize>,
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

impl Host for Fake {
    fn access(&self) -> Result<Access, String> {
        Ok(self.access.clone())
    }
    fn library(&self) -> Result<KudzuLibrary, String> {
        Ok(self.library.borrow().clone())
    }
    fn edit_library(&self, edit: Edit<'_>) -> Result<Value, String> {
        let mut copy = self.library.borrow().clone();
        let v = edit(&mut copy)?;
        *self.library.borrow_mut() = copy;
        *self.saves.borrow_mut() += 1;
        Ok(v)
    }
    fn clipboard(&self) -> Result<Vec<ClipEntry>, String> {
        Ok(self.clips.clone())
    }
    fn clipboard_text(&self, id: Uuid) -> Result<Option<String>, String> {
        Ok(self
            .clips
            .iter()
            .find(|c| c.id == id)
            .and_then(|c| c.text.clone()))
    }
    fn render_env(&self) -> RenderEnv {
        RenderEnv {
            now: chrono::FixedOffset::east_opt(0)
                .unwrap()
                .with_ymd_and_hms(2026, 11, 8, 12, 0, 0)
                .unwrap(),
            locale: "en_US".into(),
            clipboard: self.clipboard.clone(),
        }
    }
}

fn rpc(host: &dyn Host, message: Value) -> Value {
    let out = handle(host, "1.2.3", &message.to_string()).expect("a reply");
    serde_json::from_str(&out).unwrap()
}

/// Calls a tool and returns (structured result or error text, is_error).
fn tool(host: &dyn Host, name: &str, args: Value) -> (Value, bool) {
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
    for host in [&Fake::new() as &dyn Host, &Offline] {
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
        assert!(handle(&host, "1", &line).is_none(), "{line}");
        assert!(!expects_reply(&line), "{line}");
    }
    for line in [
        "{nope",
        "[1,2]",
        r#"{"jsonrpc":"2.0","id":3,"method":"nope"}"#,
    ] {
        assert!(expects_reply(line));
        let r: Value = serde_json::from_str(&handle(&host, "1", line).unwrap()).unwrap();
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
        s.formatting = Some(crate::formatting::Formatting {
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
