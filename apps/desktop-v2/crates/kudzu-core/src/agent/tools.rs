//! The tools an agent can call, their JSON schemas, and what they do.

use std::collections::{HashMap, HashSet};

use chrono::Utc;
use serde_json::{json, Map, Value};
use uuid::Uuid;

use super::{Access, Host, RenderEnv, Values};
use crate::clipboard::{ClipEntry, ClipKind, TEXT_EXCERPT_LIMIT};
use crate::matching::{find_conflicts, validate_trigger, ConflictKind};
use crate::secrets::{self, Redacted};
use crate::template::{self, FormFieldKind, FormMemory, RenderContext};
use crate::types::iso8601;
use crate::{CaseMode, FireMode, GroupRecord, KudzuLibrary, SnippetRecord};

const READ: [&str; 4] = ["list_snippets", "get_snippet", "expand_snippet", "list_groups"];
const WRITE: [&str; 2] = ["create_snippet", "update_snippet"];
const CLIPBOARD: [&str; 2] = ["list_clipboard", "get_clipboard_item"];

/// Longest text returned for one clipboard item.
const MAX_ITEM_CHARS: usize = 50_000;
/// Characters of a snippet or clipboard item shown in a list row.
const PREVIEW_CHARS: usize = 160;

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

fn tool(name: &str, title: &str, description: &str, properties: Value, required: &[&str], read_only: bool) -> Value {
    let mut schema = json!({ "type": "object", "properties": properties, "additionalProperties": false });
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
    let name = params.get("name").and_then(Value::as_str).ok_or((-32602, "Missing tool name".to_string()))?;
    if !READ.iter().chain(&WRITE).chain(&CLIPBOARD).any(|t| *t == name) {
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
        Err(message) => json!({ "content": [{ "type": "text", "text": message }], "isError": true }),
    })
}

type Args = Map<String, Value>;

fn run(host: &dyn Host, name: &str, args: &Args) -> Result<Value, String> {
    let access = host.access()?;
    if WRITE.contains(&name) && !access.write {
        return Err("Creating and editing snippets is turned off in Kudzu (Settings ▸ AI agents).".into());
    }
    if CLIPBOARD.contains(&name) && !access.clipboard {
        return Err("Clipboard history isn't shared with agents. The user can turn it on in Kudzu (Settings ▸ AI agents).".into());
    }
    let now = Utc::now();
    match name {
        "list_snippets" => list_snippets(&host.library()?, args, &access),
        "get_snippet" => get_snippet(&host.library()?, args),
        "expand_snippet" => expand_snippet(&host.library()?, args, host.render_env(), &|n, current| older_copy(host, &access, n, current)),
        "list_groups" => Ok(list_groups(&host.library()?)),
        "create_snippet" => host.edit_library(&mut |lib| create_snippet(lib, args, now)),
        "update_snippet" => host.edit_library(&mut |lib| update_snippet(lib, args, now)),
        "list_clipboard" => list_clipboard(host.clipboard()?, args),
        "get_clipboard_item" => get_clipboard_item(host, args),
        _ => Err(format!("Unknown tool: {name}")),
    }
}

// MARK: Arguments

fn string<'a>(args: &'a Args, key: &str) -> Result<Option<&'a str>, String> {
    match args.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s)) => Ok(Some(s.as_str())),
        Some(_) => Err(format!("`{key}` must be a string.")),
    }
}

fn boolean(args: &Args, key: &str) -> Result<Option<bool>, String> {
    match args.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Bool(b)) => Ok(Some(*b)),
        Some(_) => Err(format!("`{key}` must be true or false.")),
    }
}

fn count(args: &Args, key: &str, default: usize, max: usize) -> Result<usize, String> {
    match args.get(key) {
        None | Some(Value::Null) => Ok(default),
        Some(v) => v.as_u64().map(|n| (n as usize).min(max)).ok_or_else(|| format!("`{key}` must be a whole number.")),
    }
}

fn triggers(args: &Args) -> Result<Option<Vec<String>>, String> {
    match args.get("triggers") {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s)) => Ok(Some(vec![s.clone()])),
        Some(Value::Array(items)) => items
            .iter()
            .map(|v| v.as_str().map(String::from).ok_or_else(|| "`triggers` must be a list of strings.".to_string()))
            .collect::<Result<_, _>>()
            .map(Some),
        Some(_) => Err("`triggers` must be a list of strings.".into()),
    }
}

fn fire_mode(args: &Args) -> Result<Option<FireMode>, String> {
    Ok(match string(args, "fireMode")? {
        None => None,
        Some("immediate") => Some(FireMode::Immediate),
        Some("wholeWord") => Some(FireMode::WholeWord),
        Some(other) => return Err(format!("Unknown fireMode “{other}”: use immediate or wholeWord.")),
    })
}

fn case_mode(args: &Args) -> Result<Option<CaseMode>, String> {
    Ok(match string(args, "caseMode")? {
        None => None,
        Some("propagate") => Some(CaseMode::Propagate),
        Some("exact") => Some(CaseMode::Exact),
        Some(other) => return Err(format!("Unknown caseMode “{other}”: use propagate or exact.")),
    })
}

fn parse_id(raw: &str, what: &str) -> Result<Uuid, String> {
    Uuid::parse_str(raw.trim()).map_err(|_| format!("“{raw}” isn't a {what} id. Ids come from the list tools."))
}

// MARK: Library helpers

fn group_path(groups: &[GroupRecord], id: Option<Uuid>) -> Option<String> {
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
fn find_group(groups: &[GroupRecord], query: &str) -> Result<Option<Uuid>, String> {
    let q = query.trim();
    if let Ok(id) = Uuid::parse_str(q) {
        return groups.iter().find(|g| g.id == id).map(|g| Some(g.id)).ok_or_else(|| format!("No group has the id {q}."));
    }
    let wanted = q.trim_matches('/').to_lowercase();
    if let Some(g) = groups.iter().find(|g| group_path(groups, Some(g.id)).is_some_and(|p| p.to_lowercase() == wanted)) {
        return Ok(Some(g.id));
    }
    let named: Vec<&GroupRecord> = groups.iter().filter(|g| g.name.to_lowercase() == wanted).collect();
    match named.as_slice() {
        [] => Ok(None),
        [g] => Ok(Some(g.id)),
        _ => Err(format!("More than one group is called “{q}”. Use its path or id from list_groups.")),
    }
}

/// Finds the group, creating missing path components at the end of their parent.
fn ensure_group(lib: &mut KudzuLibrary, query: &str) -> Result<Uuid, String> {
    if let Some(id) = find_group(&lib.groups, query)? {
        return Ok(id);
    }
    let mut parent: Option<Uuid> = None;
    for name in query.split('/').map(str::trim).filter(|n| !n.is_empty()) {
        let existing = lib.groups.iter().find(|g| g.parent_id == parent && g.name.to_lowercase() == name.to_lowercase()).map(|g| g.id);
        parent = Some(match existing {
            Some(id) => id,
            None => {
                let sort_index = lib.groups.iter().filter(|g| g.parent_id == parent).map(|g| g.sort_index + 1).max().unwrap_or(0);
                let g = GroupRecord { parent_id: parent, sort_index, ..GroupRecord::new(name) };
                let id = g.id;
                lib.groups.push(g);
                id
            }
        });
    }
    parent.ok_or_else(|| "The group name is empty.".to_string())
}

fn descendants(groups: &[GroupRecord], root: Uuid) -> HashSet<Uuid> {
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

fn uses(lib: &KudzuLibrary) -> HashMap<Uuid, (i64, Option<chrono::DateTime<Utc>>)> {
    let mut map: HashMap<Uuid, (i64, Option<chrono::DateTime<Utc>>)> = HashMap::new();
    for u in &lib.usage {
        let e = map.entry(u.snippet_id).or_default();
        e.0 += u.count;
        e.1 = e.1.max(u.last_used_at);
    }
    map
}

fn find_snippet<'a>(lib: &'a KudzuLibrary, args: &Args) -> Result<&'a SnippetRecord, String> {
    if let Some(raw) = string(args, "id")? {
        let id = parse_id(raw, "snippet")?;
        return lib.snippets.iter().find(|s| s.id == id).ok_or_else(|| format!("No snippet has the id {id}. It may have been deleted."));
    }
    let Some(trigger) = string(args, "trigger")? else {
        return Err("Pass the snippet's `id` or one of its `trigger`s.".into());
    };
    let t = trigger.trim();
    lib.snippets
        .iter()
        .find(|s| s.triggers.iter().any(|x| x == t))
        .or_else(|| lib.snippets.iter().find(|s| s.triggers.iter().any(|x| x.to_lowercase() == t.to_lowercase())))
        .ok_or_else(|| format!("No snippet has the trigger “{t}”."))
}

fn labels(r: &Redacted) -> Value {
    json!(r.kinds.iter().map(|k| k.label()).collect::<Vec<_>>())
}

fn truncate(text: &str, max: usize) -> (String, bool) {
    match text.char_indices().nth(max) {
        Some((i, _)) => (format!("{}…", &text[..i]), true),
        None => (text.to_string(), false),
    }
}

fn one_line(text: &str, max: usize) -> String {
    let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
    truncate(&flat, max).0
}

fn has_redaction_marker(text: &str) -> bool {
    text.contains("[redacted:")
}

fn summary(lib: &KudzuLibrary, s: &SnippetRecord, usage: &HashMap<Uuid, (i64, Option<chrono::DateTime<Utc>>)>) -> Value {
    let r = secrets::redact(&s.content);
    let form = !template::analyze(&template::parse(&s.content)).form_fields.is_empty();
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

// MARK: Snippet tools

fn list_snippets(lib: &KudzuLibrary, args: &Args, access: &Access) -> Result<Value, String> {
    let limit = count(args, "limit", 50, 200)?.max(1);
    let offset = count(args, "offset", 0, usize::MAX)?;
    let favorites = boolean(args, "favoritesOnly")?.unwrap_or(false);
    let in_groups = match string(args, "group")? {
        Some(g) if !g.trim().is_empty() => {
            let id = find_group(&lib.groups, g)?.ok_or_else(|| format!("No group is called “{g}”. list_groups shows them all."))?;
            Some(descendants(&lib.groups, id))
        }
        _ => None,
    };
    let query = string(args, "query")?.unwrap_or_default().trim().to_lowercase();
    let terms: Vec<&str> = query.split_whitespace().collect();

    let mut hits: Vec<(u32, &SnippetRecord)> = lib
        .snippets
        .iter()
        .filter(|s| !favorites || s.is_favorite)
        .filter(|s| in_groups.as_ref().is_none_or(|set| s.group_id.is_some_and(|g| set.contains(&g))))
        .filter_map(|s| {
            if terms.is_empty() {
                return Some((0, s));
            }
            let name = s.name.to_lowercase();
            let triggers: Vec<String> = s.triggers.iter().map(|t| t.to_lowercase()).collect();
            // Search the redacted text, so a query can't confirm a secret.
            let content = secrets::redact(&s.content).text.to_lowercase();
            let all = format!("{name}\n{}\n{content}", triggers.join(" "));
            if !terms.iter().all(|t| all.contains(t)) {
                return None;
            }
            let score = if triggers.contains(&query) {
                4
            } else if name.contains(&query) {
                3
            } else if triggers.iter().any(|t| t.contains(&query)) {
                2
            } else {
                1
            };
            Some((score, s))
        })
        .collect();
    hits.sort_by_key(|h| std::cmp::Reverse(h.0));
    let usage = uses(lib);
    let total = hits.len();
    let page: Vec<Value> = hits.into_iter().skip(offset).take(limit).map(|(_, s)| summary(lib, s, &usage)).collect();
    Ok(json!({
        "total": total,
        "returned": page.len(),
        "triggerPrefix": access.trigger_prefix,
        "snippets": page,
    }))
}

fn get_snippet(lib: &KudzuLibrary, args: &Args) -> Result<Value, String> {
    let s = find_snippet(lib, args)?;
    let usage = uses(lib);
    let r = secrets::redact(&s.content);
    // Fields come from the scrubbed template too, so a default or option can't carry a secret.
    let by_id: HashMap<Uuid, String> = lib.snippets.iter().map(|s| (s.id, scrubbed(&s.content))).collect();
    let resolve = |id: Uuid| by_id.get(&id).cloned();
    let mut v = summary(lib, s, &usage);
    v["content"] = json!(r.text);
    v["groupId"] = json!(s.group_id);
    v["fireMode"] = json!(s.fire_mode);
    v["caseMode"] = json!(s.case_mode);
    v["formatted"] = json!(s.formatting.is_some());
    v["fields"] = json!(template::form_fields(&scrubbed(&s.content), &resolve));
    v["createdAt"] = json!(iso8601::format(&s.created_at));
    v["updatedAt"] = json!(iso8601::format(&s.updated_at));
    v["lastUsedAt"] = json!(usage.get(&s.id).and_then(|u| u.1).map(|d| iso8601::format(&d)));
    if let Some(obj) = v.as_object_mut() {
        obj.remove("preview");
    }
    if !r.is_clean() {
        v["note"] = json!("Parts that look like a password or key were withheld.");
    }
    Ok(v)
}

/// Template source with secrets replaced, so a secret can't reach the output through nesting
/// or the clipboard (where the surrounding text would hide its shape).
fn scrubbed(text: &str) -> String {
    let r = secrets::redact(text);
    if r.is_clean() {
        text.to_string()
    } else {
        r.text
    }
}

/// Text of the copy `n` before the current clipboard, for `{{clipboard history=N}}`. Agents only
/// get it when the user shares clipboard history, and never an entry marked as a secret.
fn older_copy(host: &dyn Host, access: &Access, n: usize, current: Option<&str>) -> Option<String> {
    if !access.clipboard {
        return None;
    }
    let mut entries = host.clipboard().ok()?;
    entries.sort_by_key(|e| std::cmp::Reverse(e.copied_at));
    let history = crate::clipboard::History { entries, ..Default::default() };
    let entry = history.before_current(n, current).filter(|e| !e.is_secret())?;
    host.clipboard_text(entry.id).ok().flatten().or_else(|| entry.text.clone())
}

/// What a `{{key}}` or `{{delay}}` tag does, in words an agent can relay ("Tab ×2").
fn describe_action(action: &template::ActionKind) -> String {
    match action {
        template::ActionKind::Key { key, count: 1 } => format!("press {}", key.name()),
        template::ActionKind::Key { key, count } => format!("press {} ×{count}", key.name()),
        template::ActionKind::Pause { ms } => format!("wait {ms} ms"),
    }
}

fn expand_snippet(lib: &KudzuLibrary, args: &Args, env: RenderEnv, older: &dyn Fn(usize, Option<&str>) -> Option<String>) -> Result<Value, String> {
    let s = find_snippet(lib, args)?;
    let mut given = Values::new();
    match args.get("values") {
        None | Some(Value::Null) => {}
        Some(Value::Object(m)) => {
            for (k, v) in m {
                let text = match v {
                    Value::String(s) => s.clone(),
                    Value::Number(_) | Value::Bool(_) => v.to_string(),
                    _ => return Err(format!("The value for “{k}” must be text.")),
                };
                given.insert(k.clone(), text);
            }
        }
        Some(_) => return Err("`values` must be an object of field names to text.".into()),
    }
    let by_id: HashMap<Uuid, String> = lib.snippets.iter().map(|s| (s.id, scrubbed(&s.content))).collect();
    let resolve = |id: Uuid| by_id.get(&id).cloned();
    let source = by_id.get(&s.id).cloned().unwrap_or_default();
    let fields = template::form_fields(&source, &resolve);
    let mut form_values = FormMemory::default().initial_values(s.id, &fields);
    let mut missing = Vec::new();
    for f in &fields {
        match given.remove(&f.name) {
            Some(v) => {
                if f.kind == FormFieldKind::Choice && !f.options.contains(&v) {
                    return Err(format!("“{v}” isn't an option for “{}”. Choose one of: {}.", f.name, f.options.join(", ")));
                }
                form_values.insert(f.name.clone(), v);
            }
            None if f.kind != FormFieldKind::Choice && f.default_value.is_none() => missing.push(f.name.clone()),
            None => {}
        }
    }
    let history = |n: usize| older(n, env.clipboard.as_deref()).as_deref().map(scrubbed);
    let ctx = RenderContext {
        now: env.now,
        locale: env.locale,
        clipboard: env.clipboard.as_deref().map(scrubbed),
        form_values,
        resolve_snippet: &resolve,
        resolve_formatting: None,
        root_snippet_id: Some(s.id),
        // There is no target app, so nothing is selected.
        selection: None,
        clipboard_history: Some(&history),
        new_uuid: Some(&template::random_uuid),
        pick: Some(&template::random_index),
    };
    let rendered = template::render(&source, &ctx);
    let r = secrets::redact(&rendered.text);
    let mut v = json!({
        "id": s.id,
        "name": s.name,
        "text": r.text,
        "fields": fields,
    });
    // Key presses and pauses only mean something while Kudzu types into an app. They are left out
    // of the text and listed, so the agent knows the text isn't everything the snippet does.
    if !rendered.actions.is_empty() {
        let steps: Vec<Value> = rendered
            .actions
            .iter()
            .map(|a| json!({ "afterCharacters": template_prefix_chars(&rendered.text, a.offset), "action": describe_action(&a.action) }))
            .collect();
        v["skippedActions"] = json!(steps);
        v["actionsNote"] = json!("When the user expands this snippet, Kudzu also presses keys or pauses at the listed positions (for example Tab between form fields). Those steps aren't part of `text`.");
    }
    if !missing.is_empty() {
        v["emptyFields"] = json!(missing);
    }
    if !given.is_empty() {
        let mut unknown: Vec<String> = given.into_keys().collect();
        unknown.sort();
        v["unknownFields"] = json!(unknown);
    }
    // Markers come from the template, nested snippets, the clipboard or the output itself.
    if has_redaction_marker(&r.text) {
        v["note"] = json!("Parts that look like a password or key were withheld.");
    }
    Ok(v)
}

/// Characters in the first `offset` UTF-16 units of `text` (offsets agents see count characters).
fn template_prefix_chars(text: &str, offset: usize) -> usize {
    let mut units = 0;
    text.chars().take_while(|c| { units += c.len_utf16(); units <= offset }).count()
}

fn list_groups(lib: &KudzuLibrary) -> Value {
    let mut counts: HashMap<Uuid, usize> = HashMap::new();
    for s in &lib.snippets {
        if let Some(g) = s.group_id {
            *counts.entry(g).or_default() += 1;
        }
    }
    let mut groups: Vec<Value> = lib
        .groups
        .iter()
        .map(|g| {
            json!({
                "id": g.id,
                "name": g.name,
                "path": group_path(&lib.groups, Some(g.id)),
                "parentId": g.parent_id,
                "enabled": g.is_enabled,
                "snippets": counts.get(&g.id).copied().unwrap_or(0),
            })
        })
        .collect();
    groups.sort_by(|a, b| a["path"].as_str().unwrap_or_default().to_lowercase().cmp(&b["path"].as_str().unwrap_or_default().to_lowercase()));
    let ungrouped = lib.snippets.iter().filter(|s| s.group_id.is_none()).count();
    json!({ "groups": groups, "ungroupedSnippets": ungrouped })
}

/// Checks new triggers for `id` against each other and the rest of the library.
fn checked_triggers(lib: &KudzuLibrary, id: Option<Uuid>, raw: &[String], case: CaseMode) -> Result<Vec<String>, String> {
    let mut accepted: Vec<String> = Vec::new();
    for t in raw {
        let t = validate_trigger(t, &accepted, case == CaseMode::Exact).map_err(|e| format!("Trigger “{t}”: {}", e.message()))?;
        if let Some(other) = lib.snippets.iter().find(|s| Some(s.id) != id && s.triggers.iter().any(|x| x.to_lowercase() == t.to_lowercase())) {
            let name = if other.name.is_empty() { "another snippet".to_string() } else { format!("“{}”", other.name) };
            return Err(format!("“{t}” is already the trigger of {name} (id {}). Choose another trigger, or edit that snippet instead.", other.id));
        }
        accepted.push(t);
    }
    if accepted.is_empty() {
        return Err("A snippet needs at least one trigger.".into());
    }
    Ok(accepted)
}

/// Template diagnostics and trigger overlaps worth telling the agent about.
fn warnings(lib: &KudzuLibrary, s: &SnippetRecord) -> Vec<String> {
    let mut out: Vec<String> = template::parse(&s.content).diagnostics.into_iter().map(|d| d.message).collect();
    for c in find_conflicts(&lib.snippets, &lib.groups) {
        if c.kind != ConflictKind::Shadowing || (c.snippet_id != s.id && c.other_snippet_id != s.id) {
            continue;
        }
        out.push(format!("“{}” fires before “{}” can be typed, so the longer trigger never expands. Consider wholeWord or a different trigger.", c.trigger, c.other_trigger));
    }
    out
}

fn check_content(content: &str) -> Result<(), String> {
    if has_redaction_marker(content) {
        return Err("The content contains a “[redacted: …]” marker. Secrets can't be written back through an agent; ask the user to edit the snippet in Kudzu.".into());
    }
    Ok(())
}

fn create_snippet(lib: &mut KudzuLibrary, args: &Args, now: chrono::DateTime<Utc>) -> Result<Value, String> {
    let name = string(args, "name")?.map(str::trim).unwrap_or_default();
    if name.is_empty() {
        return Err("Give the snippet a `name`.".into());
    }
    let content = string(args, "content")?.unwrap_or_default();
    if content.trim().is_empty() {
        return Err("The snippet's `content` is empty.".into());
    }
    check_content(content)?;
    let case = case_mode(args)?.unwrap_or_default();
    let raw = triggers(args)?.ok_or("Give the snippet at least one trigger in `triggers`.")?;
    let triggers = checked_triggers(lib, None, &raw, case)?;
    let fire = fire_mode(args)?.unwrap_or_default();
    let favorite = boolean(args, "favorite")?.unwrap_or(false);
    let group = string(args, "group")?.map(str::trim).filter(|g| !g.is_empty());
    if let Some(g) = group {
        find_group(&lib.groups, g)?; // report ambiguity before changing anything
    }

    let mut s = SnippetRecord::new(name, triggers, content);
    s.created_at = now;
    s.updated_at = now;
    s.case_mode = case;
    s.fire_mode = fire;
    s.is_favorite = favorite;
    s.group_id = group.map(|g| ensure_group(lib, g)).transpose()?;
    lib.snippets.insert(0, s.clone());
    let usage = uses(lib);
    let mut v = json!({ "created": summary(lib, &s, &usage) });
    let w = warnings(lib, &s);
    if !w.is_empty() {
        v["warnings"] = json!(w);
    }
    Ok(v)
}

fn update_snippet(lib: &mut KudzuLibrary, args: &Args, now: chrono::DateTime<Utc>) -> Result<Value, String> {
    let raw_id = string(args, "id")?.ok_or("Pass the `id` of the snippet to edit.")?;
    let id = parse_id(raw_id, "snippet")?;
    let mut s = lib.snippets.iter().find(|s| s.id == id).cloned().ok_or_else(|| format!("No snippet has the id {id}."))?;
    let before = s.clone();

    if let Some(name) = string(args, "name")? {
        let name = name.trim();
        if name.is_empty() {
            return Err("A snippet's name can't be empty.".into());
        }
        s.name = name.to_string();
    }
    if let Some(content) = string(args, "content")? {
        if content != s.content {
            if secrets::contains_secret(&s.content) {
                return Err("This snippet holds what looks like a password or key, so its content can only be changed in Kudzu. Other fields can still be edited here.".into());
            }
            if s.formatting.is_some() {
                return Err("This is a formatted snippet (bold, links or images). Edit its content in Kudzu so the formatting isn't lost.".into());
            }
            if content.trim().is_empty() {
                return Err("The new content is empty.".into());
            }
            check_content(content)?;
            s.content = content.to_string();
        }
    }
    if let Some(mode) = case_mode(args)? {
        s.case_mode = mode;
    }
    if let Some(mode) = fire_mode(args)? {
        s.fire_mode = mode;
    }
    if let Some(raw) = triggers(args)? {
        s.triggers = checked_triggers(lib, Some(id), &raw, s.case_mode)?;
    }
    if let Some(enabled) = boolean(args, "enabled")? {
        s.is_enabled = enabled;
    }
    if let Some(favorite) = boolean(args, "favorite")? {
        s.is_favorite = favorite;
    }
    if args.contains_key("group") {
        let g = string(args, "group")?.map(str::trim).filter(|g| !g.is_empty());
        if let Some(g) = g {
            find_group(&lib.groups, g)?;
        }
        s.group_id = g.map(|g| ensure_group(lib, g)).transpose()?;
    }
    if s == before {
        let usage = uses(lib);
        return Ok(json!({ "updated": summary(lib, &s, &usage), "unchanged": true }));
    }
    s.updated_at = now;
    if let Some(slot) = lib.snippets.iter_mut().find(|x| x.id == id) {
        *slot = s.clone();
    }
    let usage = uses(lib);
    let mut v = json!({ "updated": summary(lib, &s, &usage) });
    let w = warnings(lib, &s);
    if !w.is_empty() {
        v["warnings"] = json!(w);
    }
    Ok(v)
}

// MARK: Clipboard tools

/// Redacts an entry's text. A cut-off excerpt loses its last word first, so a secret split at
/// the cut can't slip through as a fragment too short to recognize.
fn redact_entry_text(text: &str, complete: bool) -> Redacted {
    if complete {
        return secrets::redact(text);
    }
    let cut = text.rfind(char::is_whitespace).unwrap_or(0);
    let mut r = secrets::redact(&text[..cut]);
    r.whole = false;
    r
}

/// Redaction for an entry, honouring the user's correction: "not a secret" is shared as is,
/// "hide it" is withheld whole.
fn entry_redaction(e: &ClipEntry, text: &str, complete: bool) -> Redacted {
    match e.secret_override {
        Some(false) => Redacted { text: text.to_string(), kinds: vec![], whole: false },
        Some(true) => Redacted { text: String::new(), kinds: vec![e.secret.unwrap_or(secrets::SecretKind::Credential)], whole: true },
        None => redact_entry_text(text, complete),
    }
}

fn clip_summary(e: &ClipEntry, text: Option<&str>, complete: bool, max: usize) -> Value {
    let mut v = json!({
        "id": e.id,
        "kind": e.kind,
        "copiedAt": iso8601::format(&e.copied_at),
        "copyCount": e.copy_count,
        "pinned": e.pinned,
        "length": e.text_len,
        "lines": e.line_count,
    });
    if let Some(app) = &e.source {
        v["app"] = json!(app.name);
    }
    if !e.files.is_empty() {
        v["files"] = json!(e.files);
    }
    if let Some(size) = e.image {
        v["image"] = json!({ "width": size.width, "height": size.height });
    }
    if let Some(color) = &e.color {
        v["color"] = json!(color);
    }
    if let Some(text) = text {
        let r = entry_redaction(e, text, complete);
        if r.whole {
            v["text"] = Value::Null;
            // The length of a password narrows it down; leave the shape out too.
            if let Some(obj) = v.as_object_mut() {
                obj.remove("length");
                obj.remove("lines");
            }
            v["withheld"] = json!(match e.secret_override {
                Some(true) => "The user marked this as a secret, so Kudzu doesn't share it.".to_string(),
                _ => format!("This looks like a {}, so Kudzu doesn't share it.", r.kinds.first().map(|k| k.label()).unwrap_or("secret")),
            });
        } else {
            let (shown, cut) = truncate(&r.text, max);
            v["text"] = json!(shown);
            if cut || !complete {
                v["truncated"] = json!(true);
            }
        }
        if !r.is_clean() {
            v["redacted"] = labels(&r);
        }
    }
    v
}

fn parse_kind(raw: &str) -> Result<ClipKind, String> {
    serde_json::from_value(json!(raw)).map_err(|_| format!("Unknown kind “{raw}”: use text, code, link, color, image, file or other."))
}

fn list_clipboard(mut entries: Vec<ClipEntry>, args: &Args) -> Result<Value, String> {
    let limit = count(args, "limit", 20, 100)?.max(1);
    let kind = string(args, "kind")?.map(parse_kind).transpose()?;
    let pinned = boolean(args, "pinnedOnly")?.unwrap_or(false);
    let query = string(args, "query")?.unwrap_or_default().trim().to_lowercase();
    let terms: Vec<&str> = query.split_whitespace().collect();
    entries.sort_by_key(|e| std::cmp::Reverse(e.copied_at));
    let mut total = 0;
    let mut items = Vec::new();
    for e in &entries {
        if kind.is_some_and(|k| k != e.kind) || (pinned && !e.pinned) {
            continue;
        }
        let complete = e.text_len <= TEXT_EXCERPT_LIMIT;
        if !terms.is_empty() {
            // Search the redacted text, so a query can't confirm a secret.
            let text = e.text.as_deref().map(|t| entry_redaction(e, t, complete)).filter(|r| !r.whole).map(|r| r.text).unwrap_or_default();
            let hay = format!("{}\n{}\n{}", text, e.files.join("\n"), e.source.as_ref().map(|s| s.name.as_str()).unwrap_or_default()).to_lowercase();
            if !terms.iter().all(|t| hay.contains(t)) {
                continue;
            }
        }
        total += 1;
        if items.len() < limit {
            items.push(clip_summary(e, e.text.as_deref(), complete, PREVIEW_CHARS * 2));
        }
    }
    Ok(json!({ "total": total, "returned": items.len(), "items": items }))
}

fn get_clipboard_item(host: &dyn Host, args: &Args) -> Result<Value, String> {
    let raw = string(args, "id")?.ok_or("Pass the `id` of a clipboard item from list_clipboard.")?;
    let id = parse_id(raw, "clipboard item")?;
    let entries = host.clipboard()?;
    let e = entries.iter().find(|e| e.id == id).ok_or("That item is no longer in the clipboard history.")?;
    let full = host.clipboard_text(id)?;
    let (text, complete) = match full {
        Some(t) => (Some(t), true),
        None => (e.text.clone(), e.text_len <= TEXT_EXCERPT_LIMIT),
    };
    Ok(clip_summary(e, text.as_deref(), complete, MAX_ITEM_CHARS))
}
