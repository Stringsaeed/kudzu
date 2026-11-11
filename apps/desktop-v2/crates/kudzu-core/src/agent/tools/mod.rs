//! The tools an agent can call, their JSON schemas, and what they do.
//!
//! - `snippets`: list, get, expand, groups, create and update.
//! - `clipboard`: clipboard history search and items.

mod clipboard;
mod snippets;

use std::collections::{HashMap, HashSet};

use chrono::Utc;
use serde_json::{json, Map, Value};
use uuid::Uuid;

use super::{Access, Host};
use crate::secrets::{self, Redacted};
use crate::template;
use crate::{CaseMode, FireMode, GroupRecord, KudzuLibrary, SnippetRecord};

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

/// Longest text returned for one clipboard item.
pub(super) const MAX_ITEM_CHARS: usize = 50_000;
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
            "Renders a snippet as if the user typed its trigger now: dates, nested snippets and the clipboard are filled in. Fill-in fields take `values`; missing ones use their defaults. Key presses and pauses ({{key}}, {{delay}}) are left out of the text and listed in `skippedActions`; {{selection}} is empty.",
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
pub(super) fn call(host: &dyn Host, params: &Value) -> Result<Value, (i64, String)> {
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
        return Err((-32602, format!("Unknown tool: {name}")));
    }
    let empty = Map::new();
    let args = match params.get("arguments") {
        None | Some(Value::Null) => &empty,
        Some(Value::Object(m)) => m,
        Some(_) => return Err((-32602, "Tool arguments must be an object".into())),
    };
    Ok(match run(host, name, args) {
        Ok(value) => json!({
            "content": [{ "type": "text", "text": serde_json::to_string_pretty(&value).unwrap_or_default() }],
            "structuredContent": value,
            "isError": false,
        }),
        Err(message) => {
            json!({ "content": [{ "type": "text", "text": message }], "isError": true })
        }
    })
}

pub(super) type Args = Map<String, Value>;

fn run(host: &dyn Host, name: &str, args: &Args) -> Result<Value, String> {
    let access = host.access()?;
    if WRITE.contains(&name) && !access.write {
        return Err(
            "Creating and editing snippets is turned off in Kudzu (Settings ▸ AI agents).".into(),
        );
    }
    if CLIPBOARD.contains(&name) && !access.clipboard {
        return Err("Clipboard history isn't shared with agents. The user can turn it on in Kudzu (Settings ▸ AI agents).".into());
    }
    let now = Utc::now();
    match name {
        "list_snippets" => list_snippets(&host.library()?, args, &access),
        "get_snippet" => get_snippet(&host.library()?, args),
        "expand_snippet" => {
            expand_snippet(&host.library()?, args, host.render_env(), &|n, current| {
                older_copy(host, &access, n, current)
            })
        }
        "list_groups" => Ok(list_groups(&host.library()?)),
        "create_snippet" => host.edit_library(&mut |lib| create_snippet(lib, args, now)),
        "update_snippet" => host.edit_library(&mut |lib| update_snippet(lib, args, now)),
        "list_clipboard" => list_clipboard(host.clipboard()?, args),
        "get_clipboard_item" => get_clipboard_item(host, args),
        _ => Err(format!("Unknown tool: {name}")),
    }
}

// MARK: Arguments

pub(super) fn string<'a>(args: &'a Args, key: &str) -> Result<Option<&'a str>, String> {
    match args.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s)) => Ok(Some(s.as_str())),
        Some(_) => Err(format!("`{key}` must be a string.")),
    }
}

pub(super) fn boolean(args: &Args, key: &str) -> Result<Option<bool>, String> {
    match args.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Bool(b)) => Ok(Some(*b)),
        Some(_) => Err(format!("`{key}` must be true or false.")),
    }
}

pub(super) fn count(args: &Args, key: &str, default: usize, max: usize) -> Result<usize, String> {
    match args.get(key) {
        None | Some(Value::Null) => Ok(default),
        Some(v) => v
            .as_u64()
            .map(|n| (n as usize).min(max))
            .ok_or_else(|| format!("`{key}` must be a whole number.")),
    }
}

pub(super) fn triggers(args: &Args) -> Result<Option<Vec<String>>, String> {
    match args.get("triggers") {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s)) => Ok(Some(vec![s.clone()])),
        Some(Value::Array(items)) => items
            .iter()
            .map(|v| {
                v.as_str()
                    .map(String::from)
                    .ok_or_else(|| "`triggers` must be a list of strings.".to_string())
            })
            .collect::<Result<_, _>>()
            .map(Some),
        Some(_) => Err("`triggers` must be a list of strings.".into()),
    }
}

pub(super) fn fire_mode(args: &Args) -> Result<Option<FireMode>, String> {
    Ok(match string(args, "fireMode")? {
        None => None,
        Some("immediate") => Some(FireMode::Immediate),
        Some("wholeWord") => Some(FireMode::WholeWord),
        Some(other) => {
            return Err(format!(
                "Unknown fireMode “{other}”: use immediate or wholeWord."
            ))
        }
    })
}

pub(super) fn case_mode(args: &Args) -> Result<Option<CaseMode>, String> {
    Ok(match string(args, "caseMode")? {
        None => None,
        Some("propagate") => Some(CaseMode::Propagate),
        Some("exact") => Some(CaseMode::Exact),
        Some(other) => {
            return Err(format!(
                "Unknown caseMode “{other}”: use propagate or exact."
            ))
        }
    })
}

pub(super) fn parse_id(raw: &str, what: &str) -> Result<Uuid, String> {
    Uuid::parse_str(raw.trim())
        .map_err(|_| format!("“{raw}” isn't a {what} id. Ids come from the list tools."))
}

// MARK: Library helpers

pub(super) fn group_path(groups: &[GroupRecord], id: Option<Uuid>) -> Option<String> {
    let mut names = Vec::new();
    let mut seen = HashSet::new();
    let mut cur = id;
    while let Some(g) = cur.and_then(|id| groups.iter().find(|g| g.id == id)) {
        if !seen.insert(g.id) {
            break;
        }
        names.push(g.name.as_str());
        cur = g.parent_id;
    }
    names.reverse();
    (!names.is_empty()).then(|| names.join("/"))
}

/// A group by id, path ("Work/Email") or (unambiguous) name.
pub(super) fn find_group(groups: &[GroupRecord], query: &str) -> Result<Option<Uuid>, String> {
    let q = query.trim();
    if let Ok(id) = Uuid::parse_str(q) {
        return groups
            .iter()
            .find(|g| g.id == id)
            .map(|g| Some(g.id))
            .ok_or_else(|| format!("No group has the id {q}."));
    }
    let wanted = q.trim_matches('/').to_lowercase();
    if let Some(g) = groups
        .iter()
        .find(|g| group_path(groups, Some(g.id)).is_some_and(|p| p.to_lowercase() == wanted))
    {
        return Ok(Some(g.id));
    }
    let named: Vec<&GroupRecord> = groups
        .iter()
        .filter(|g| g.name.to_lowercase() == wanted)
        .collect();
    match named.as_slice() {
        [] => Ok(None),
        [g] => Ok(Some(g.id)),
        _ => Err(format!(
            "More than one group is called “{q}”. Use its path or id from list_groups."
        )),
    }
}

/// Finds the group, creating missing path components at the end of their parent.
pub(super) fn ensure_group(lib: &mut KudzuLibrary, query: &str) -> Result<Uuid, String> {
    if let Some(id) = find_group(&lib.groups, query)? {
        return Ok(id);
    }
    let mut parent: Option<Uuid> = None;
    for name in query.split('/').map(str::trim).filter(|n| !n.is_empty()) {
        let existing = lib
            .groups
            .iter()
            .find(|g| g.parent_id == parent && g.name.to_lowercase() == name.to_lowercase())
            .map(|g| g.id);
        parent = Some(match existing {
            Some(id) => id,
            None => {
                let sort_index = lib
                    .groups
                    .iter()
                    .filter(|g| g.parent_id == parent)
                    .map(|g| g.sort_index + 1)
                    .max()
                    .unwrap_or(0);
                let g = GroupRecord {
                    parent_id: parent,
                    sort_index,
                    ..GroupRecord::new(name)
                };
                let id = g.id;
                lib.groups.push(g);
                id
            }
        });
    }
    parent.ok_or_else(|| "The group name is empty.".to_string())
}

pub(super) fn descendants(groups: &[GroupRecord], root: Uuid) -> HashSet<Uuid> {
    let mut set = HashSet::from([root]);
    loop {
        let before = set.len();
        for g in groups {
            if g.parent_id.is_some_and(|p| set.contains(&p)) {
                set.insert(g.id);
            }
        }
        if set.len() == before {
            return set;
        }
    }
}

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
