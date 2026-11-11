//! The tools an agent can call, their JSON schemas, and what they do.
//!
//! - `snippets`: list, get, expand, groups, create and update.
//! - `clipboard`: clipboard history search and items.

mod args;
mod clipboard;
mod groups;
mod snippets;
use args::*;
use groups::*;

use std::collections::HashMap;

use chrono::Utc;
use serde_json::{json, Map, Value};
use uuid::Uuid;

use super::{service::ServiceHost, Access};
use kudzu_core::secrets::{self, Redacted};
use kudzu_core::template;
use kudzu_core::{KudzuLibrary, SnippetRecord};

use clipboard::{get_clipboard_item, list_clipboard};
use snippets::{
    create_snippet, expand_snippet, get_snippet, list_groups, list_snippets, older_copy,
    update_snippet,
};

const READ: [&str; 4] = [
    "list_snippets",
    "get_snippet",
    "expand_snippet",
    "list_groups",
];
const WRITE: [&str; 2] = ["create_snippet", "update_snippet"];
const CLIPBOARD: [&str; 2] = ["list_clipboard", "get_clipboard_item"];

/// Characters of a snippet or clipboard item shown in a list row.
pub(super) const PREVIEW_CHARS: usize = 160;

/// The tools `access` allows, as MCP tool definitions.
pub fn definitions(access: &Access) -> Vec<Value> {
    let prefix = &access.trigger_prefix;
    let mut tools = vec![
        tool(
            "list_snippets",
            "List snippets",
            "Lists the user's snippets, or searches them by name, trigger and content. Returns ids, triggers, a short preview and how often each was used.",
            json!({
                "query": { "type": "string", "description": "Words to search for in names, triggers and content." },
                "group": { "type": "string", "description": "Only snippets in this group (id, name or path like \"Work/Email\"), including its subgroups." },
                "favoritesOnly": { "type": "boolean" },
                "limit": { "type": "integer", "minimum": 1, "maximum": 200, "default": 50 },
                "offset": { "type": "integer", "minimum": 0, "default": 0 },
            }),
            &[],
            true,
        ),
        tool(
            "get_snippet",
            "Read a snippet",
            "Returns one snippet's full template, triggers, settings and fill-in fields. Pass its id or one of its triggers.",
            json!({
                "id": { "type": "string", "description": "Snippet id from list_snippets." },
                "trigger": { "type": "string", "description": "One of the snippet's triggers, such as \";sig\"." },
            }),
            &[],
            true,
        ),
        tool(
            "expand_snippet",
            "Expand a snippet",
            "Renders a snippet as if the user typed its trigger now: dates, nested snippets and the clipboard are filled in. {{clipboard}} is blank unless clipboard recording and agent sharing are on and the newest recorded entry matches the live clipboard. Fill-in fields take `values`; missing ones use their defaults. Key presses and pauses ({{key}}, {{delay}}) are left out of the text and listed in `skippedActions`; {{selection}} is empty.",
            json!({
                "id": { "type": "string" },
                "trigger": { "type": "string" },
                "values": { "type": "object", "additionalProperties": { "type": "string" }, "description": "Fill-in field values by field name." },
            }),
            &[],
            true,
        ),
        tool(
            "list_groups",
            "List groups",
            "Lists the folders snippets are organized in, with their paths and snippet counts.",
            json!({}),
            &[],
            true,
        ),
    ];
    if access.write {
        tools.push(tool(
            "create_snippet",
            "Create a snippet",
            &format!(
                "Adds a snippet to the user's library. It works in every app right away. Triggers conventionally start with \"{prefix}\" (for example \"{prefix}addr\") and must not already be in use. Only create snippets the user asked for."
            ),
            json!({
                "name": { "type": "string", "description": "A short descriptive name." },
                "content": { "type": "string", "description": "The template: text plus optional {{tags}} (see the server instructions)." },
                "triggers": { "type": "array", "items": { "type": "string" }, "minItems": 1, "description": "What the user types to expand it." },
                "group": { "type": "string", "description": "Group id, name or path (\"Work/Email\"). Missing groups are created." },
                "fireMode": { "type": "string", "enum": ["immediate", "wholeWord"], "description": "immediate (default) expands as soon as the trigger is typed; wholeWord waits for a space or punctuation." },
                "caseMode": { "type": "string", "enum": ["propagate", "exact"], "description": "propagate (default) matches ;Sig and ;SIG and adapts the output's case; exact matches only as written." },
                "favorite": { "type": "boolean" },
            }),
            &["name", "content", "triggers"],
            false,
        ));
        tools.push(tool(
            "update_snippet",
            "Edit a snippet",
            "Changes an existing snippet. Pass its id and only the fields to change. Content of formatted snippets, and of snippets that hold a password or key, can only be edited in Kudzu.",
            json!({
                "id": { "type": "string" },
                "name": { "type": "string" },
                "content": { "type": "string" },
                "triggers": { "type": "array", "items": { "type": "string" }, "minItems": 1, "description": "Replaces all triggers." },
                "group": { "type": ["string", "null"], "description": "Group id, name or path; null or \"\" moves it out of any group." },
                "enabled": { "type": "boolean" },
                "favorite": { "type": "boolean" },
                "fireMode": { "type": "string", "enum": ["immediate", "wholeWord"] },
                "caseMode": { "type": "string", "enum": ["propagate", "exact"] },
            }),
            &["id"],
            false,
        ));
    }
    if access.clipboard {
        tools.push(tool(
            "list_clipboard",
            "Search clipboard history",
            "Lists what the user copied recently, newest first, with the app it came from. Passwords, keys and tokens are withheld.",
            json!({
                "query": { "type": "string", "description": "Words to search for in the copied text, file paths and app names." },
                "kind": { "type": "string", "enum": ["text", "code", "link", "color", "image", "file", "other"] },
                "pinnedOnly": { "type": "boolean" },
                "limit": { "type": "integer", "minimum": 1, "maximum": 100, "default": 20 },
            }),
            &[],
            true,
        ));
        tools.push(tool(
            "get_clipboard_item",
            "Read a clipboard item",
            "Returns the full text of one clipboard history item (list results are shortened).",
            json!({ "id": { "type": "string" } }),
            &["id"],
            true,
        ));
    }
    tools
}

fn tool(
    name: &str,
    title: &str,
    description: &str,
    properties: Value,
    required: &[&str],
    read_only: bool,
) -> Value {
    let mut schema =
        json!({ "type": "object", "properties": properties, "additionalProperties": false });
    if !required.is_empty() {
        schema["required"] = json!(required);
    }
    let annotations = if read_only {
        json!({ "title": title, "readOnlyHint": true, "openWorldHint": false })
    } else {
        json!({ "title": title, "readOnlyHint": false, "destructiveHint": name == "update_snippet", "idempotentHint": false, "openWorldHint": false })
    };
    json!({ "name": name, "title": title, "description": description, "inputSchema": schema, "annotations": annotations })
}

/// Runs a `tools/call`. Tool failures are results with `isError`; only a malformed call is a
/// protocol error.
pub(super) fn call(host: Option<&ServiceHost>, params: &Value) -> Result<Value, (i64, String)> {
    let name = params
        .get("name")
        .and_then(Value::as_str)
        .ok_or((-32602, "Missing tool name".to_string()))?;
    if !READ
        .iter()
        .chain(&WRITE)
        .chain(&CLIPBOARD)
        .any(|t| *t == name)
    {
        return Err((-32602, format!("Unknown tool: {}", super::input_name(name))));
    }
    let empty = Map::new();
    let args = match params.get("arguments") {
        None | Some(Value::Null) => &empty,
        Some(Value::Object(m)) => m,
        Some(_) => return Err((-32602, "Tool arguments must be an object".into())),
    };
    Ok(
        match host
            .ok_or_else(|| super::OFFLINE_MESSAGE.to_string())
            .and_then(|host| run(host, name, args))
        {
            Ok(value) => {
                json!({
                    "content": [{ "type": "text", "text": serde_json::to_string_pretty(&value).unwrap_or_default() }],
                    "structuredContent": value,
                    "isError": false,
                })
            }
            Err(message) => {
                json!({ "content": [{ "type": "text", "text": secrets::redact(&message).text }], "isError": true })
            }
        },
    )
}

pub(super) type Args = Map<String, Value>;

fn run(host: &ServiceHost, name: &str, args: &Args) -> Result<Value, String> {
    let access = host.access()?;
    if WRITE.contains(&name) && !access.write {
        return Err(
            "Creating and editing snippets is turned off in Kudzu (Settings ▸ AI agents).".into(),
        );
    }
    if CLIPBOARD.contains(&name) && !access.clipboard {
        return Err("Clipboard history isn't shared with agents. The user can turn it on in Kudzu (Settings ▸ AI agents).".into());
    }
    let now = host.clock.now().into();
    match name {
        "list_snippets" => list_snippets(&host.library()?, args, &access),
        "get_snippet" => get_snippet(&host.library()?, args),
        "expand_snippet" => {
            expand_snippet(&host.library()?, args, host.render_env(), &|n, current| {
                older_copy(host, &access, n, current)
            })
        }
        "list_groups" => Ok(list_groups(&host.library()?)),
        "create_snippet" => transact(&host.library, |lib| {
            create_snippet(lib, args, now, host.ids.as_ref())
        }),
        "update_snippet" => transact(&host.library, |lib| {
            update_snippet(lib, args, now, host.ids.as_ref())
        }),
        "list_clipboard" => list_clipboard(host, args),
        "get_clipboard_item" => get_clipboard_item(host, args),
        _ => Err(format!("Unknown tool: {}", super::input_name(name))),
    }
}

// Translate tool validation failures without committing a partial library copy.
fn transact(
    library: &crate::library::LibraryService,
    edit: impl FnOnce(&mut KudzuLibrary) -> Result<Value, String>,
) -> Result<Value, String> {
    let mut rejected = None;
    library
        .transact(|lib| {
            edit(lib).map_err(|message| {
                rejected = Some(message);
                crate::library::LibraryError::TriggerTaken
            })
        })
        .map(|(value, _)| value)
        .map_err(|error| rejected.unwrap_or_else(|| error.to_string()))
}

// MARK: Arguments

// MARK: Library helpers

pub(super) fn uses(lib: &KudzuLibrary) -> HashMap<Uuid, (i64, Option<chrono::DateTime<Utc>>)> {
    let mut map: HashMap<Uuid, (i64, Option<chrono::DateTime<Utc>>)> = HashMap::new();
    for u in &lib.usage {
        let e = map.entry(u.snippet_id).or_default();
        e.0 += u.count;
        e.1 = e.1.max(u.last_used_at);
    }
    map
}

pub(super) fn find_snippet<'a>(
    lib: &'a KudzuLibrary,
    args: &Args,
) -> Result<&'a SnippetRecord, String> {
    if let Some(raw) = string(args, "id")? {
        let id = parse_id(raw, "snippet")?;
        return lib
            .snippets
            .iter()
            .find(|s| s.id == id)
            .ok_or_else(|| format!("No snippet has the id {id}. It may have been deleted."));
    }
    let Some(trigger) = string(args, "trigger")? else {
        return Err("Pass the snippet's `id` or one of its `trigger`s.".into());
    };
    let t = trigger.trim();
    lib.snippets
        .iter()
        .find(|s| s.triggers.iter().any(|x| x == t))
        .or_else(|| {
            lib.snippets.iter().find(|s| {
                s.triggers
                    .iter()
                    .any(|x| x.to_lowercase() == t.to_lowercase())
            })
        })
        .ok_or_else(|| format!("No snippet has the trigger “{t}”."))
}

pub(super) fn labels(r: &Redacted) -> Value {
    json!(r.kinds.iter().map(|k| k.label()).collect::<Vec<_>>())
}

pub(super) fn truncate(text: &str, max: usize) -> (String, bool) {
    match text.char_indices().nth(max) {
        Some((i, _)) => (format!("{}…", &text[..i]), true),
        None => (text.to_string(), false),
    }
}

pub(super) fn one_line(text: &str, max: usize) -> String {
    let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
    truncate(&flat, max).0
}

pub(super) fn has_redaction_marker(text: &str) -> bool {
    text.contains("[redacted:")
}

pub(super) fn summary(
    lib: &KudzuLibrary,
    s: &SnippetRecord,
    usage: &HashMap<Uuid, (i64, Option<chrono::DateTime<Utc>>)>,
) -> Value {
    let r = secrets::redact(&s.content);
    let form = !template::analyze(&template::parse(&s.content))
        .form_fields
        .is_empty();
    let mut v = json!({
        "id": s.id,
        "name": s.name,
        "triggers": s.triggers,
        "group": group_path(&lib.groups, s.group_id),
        "enabled": s.is_enabled,
        "favorite": s.is_favorite,
        "form": form,
        "preview": one_line(&r.text, PREVIEW_CHARS),
        "uses": usage.get(&s.id).map(|u| u.0).unwrap_or(0),
    });
    if !r.is_clean() {
        v["redacted"] = labels(&r);
    }
    v
}
