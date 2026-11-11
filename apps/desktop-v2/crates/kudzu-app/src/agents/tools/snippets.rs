//! Snippet tools: list, get, expand, groups, create and update.

use std::collections::HashMap;

use chrono::Utc;
use serde_json::{json, Value};
use uuid::Uuid;

use super::{
    boolean, case_mode, count, descendants, ensure_group, find_group, find_snippet, fire_mode,
    group_path, has_redaction_marker, parse_id, string, summary, triggers, uses, Args,
};
use crate::agents::{service::ServiceHost, Access, RenderEnv, Values};
use kudzu_core::library::conflicts::{find_conflicts, ConflictKind};
use kudzu_core::library::model::iso8601;
use kudzu_core::library::triggers::validate_trigger;
use kudzu_core::secrets;
use kudzu_core::template::{self, FormFieldKind, FormMemory, RenderContext};
use kudzu_core::{CaseMode, KudzuLibrary, SnippetRecord};

// MARK: Snippet tools

pub(super) fn list_snippets(
    lib: &KudzuLibrary,
    args: &Args,
    access: &Access,
) -> Result<Value, String> {
    let limit = count(args, "limit", 50, 200)?.max(1);
    let offset = count(args, "offset", 0, usize::MAX)?;
    let favorites = boolean(args, "favoritesOnly")?.unwrap_or(false);
    let in_groups = match string(args, "group")? {
        Some(g) if !g.trim().is_empty() => {
            let id = find_group(&lib.groups, g)?
                .ok_or_else(|| format!("No group is called “{g}”. list_groups shows them all."))?;
            Some(descendants(&lib.groups, id))
        }
        _ => None,
    };
    let query = string(args, "query")?
        .unwrap_or_default()
        .trim()
        .to_lowercase();
    let terms: Vec<&str> = query.split_whitespace().collect();

    let mut hits: Vec<(u32, &SnippetRecord)> = lib
        .snippets
        .iter()
        .filter(|s| !favorites || s.is_favorite)
        .filter(|s| {
            in_groups
                .as_ref()
                .is_none_or(|set| s.group_id.is_some_and(|g| set.contains(&g)))
        })
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
    let page: Vec<Value> = hits
        .into_iter()
        .skip(offset)
        .take(limit)
        .map(|(_, s)| summary(lib, s, &usage))
        .collect();
    Ok(json!({
        "total": total,
        "returned": page.len(),
        "triggerPrefix": access.trigger_prefix,
        "snippets": page,
    }))
}

pub(super) fn get_snippet(lib: &KudzuLibrary, args: &Args) -> Result<Value, String> {
    let s = find_snippet(lib, args)?;
    let usage = uses(lib);
    let r = secrets::redact(&s.content);
    // Fields come from the scrubbed template too, so a default or option can't carry a secret.
    let by_id: HashMap<Uuid, String> = lib
        .snippets
        .iter()
        .map(|s| (s.id, scrubbed(&s.content)))
        .collect();
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
    v["lastUsedAt"] = json!(usage
        .get(&s.id)
        .and_then(|u| u.1)
        .map(|d| iso8601::format(&d)));
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
pub(super) fn older_copy(
    host: &ServiceHost,
    access: &Access,
    n: usize,
    current: Option<&str>,
) -> Option<String> {
    if !access.clipboard {
        return None;
    }
    host.agent_history_text(n, current)
        .map(|s| s.chars().take(50_000).collect())
}

/// What a `{{key}}` or `{{delay}}` tag does, in words an agent can relay ("Tab ×2").
fn describe_action(action: &template::ActionKind) -> String {
    match action {
        template::ActionKind::Key { key, count: 1 } => format!("press {}", key.name()),
        template::ActionKind::Key { key, count } => format!("press {} ×{count}", key.name()),
        template::ActionKind::Pause { ms } => format!("wait {ms} ms"),
    }
}

pub(super) fn expand_snippet(
    lib: &KudzuLibrary,
    args: &Args,
    env: RenderEnv,
    older: &dyn Fn(usize, Option<&str>) -> Option<String>,
) -> Result<Value, String> {
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
    let by_id: HashMap<Uuid, String> = lib
        .snippets
        .iter()
        .map(|s| (s.id, scrubbed(&s.content)))
        .collect();
    let resolve = |id: Uuid| by_id.get(&id).cloned();
    let source = by_id.get(&s.id).cloned().unwrap_or_default();
    let fields = template::form_fields(&source, &resolve);
    let mut form_values = FormMemory::default().initial_values(s.id, &fields);
    let mut missing = Vec::new();
    for f in &fields {
        match given.remove(&f.name) {
            Some(v) => {
                if f.kind == FormFieldKind::Choice && !f.options.contains(&v) {
                    return Err(format!(
                        "“{v}” isn't an option for “{}”. Choose one of: {}.",
                        f.name,
                        f.options.join(", ")
                    ));
                }
                form_values.insert(f.name.clone(), v);
            }
            None if f.kind != FormFieldKind::Choice && f.default_value.is_none() => {
                missing.push(f.name.clone())
            }
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
    text.chars()
        .take_while(|c| {
            units += c.len_utf16();
            units <= offset
        })
        .count()
}

pub(super) fn list_groups(lib: &KudzuLibrary) -> Value {
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
    groups.sort_by(|a, b| {
        a["path"]
            .as_str()
            .unwrap_or_default()
            .to_lowercase()
            .cmp(&b["path"].as_str().unwrap_or_default().to_lowercase())
    });
    let ungrouped = lib.snippets.iter().filter(|s| s.group_id.is_none()).count();
    json!({ "groups": groups, "ungroupedSnippets": ungrouped })
}

/// Checks new triggers for `id` against each other and the rest of the library.
fn checked_triggers(
    lib: &KudzuLibrary,
    id: Option<Uuid>,
    raw: &[String],
    case: CaseMode,
) -> Result<Vec<String>, String> {
    let mut accepted: Vec<String> = Vec::new();
    for t in raw {
        let t = validate_trigger(t, &accepted, case == CaseMode::Exact)
            .map_err(|e| format!("Trigger “{t}”: {}", e.message()))?;
        if let Some(other) = lib.snippets.iter().find(|s| {
            Some(s.id) != id
                && s.triggers
                    .iter()
                    .any(|x| x.to_lowercase() == t.to_lowercase())
        }) {
            let name = if other.name.is_empty() {
                "another snippet".to_string()
            } else {
                format!("“{}”", other.name)
            };
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
    let mut out: Vec<String> = template::parse(&s.content)
        .diagnostics
        .into_iter()
        .map(|d| d.message)
        .collect();
    for c in find_conflicts(&lib.snippets, &lib.groups) {
        if c.kind != ConflictKind::Shadowing || (c.snippet_id != s.id && c.other_snippet_id != s.id)
        {
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

pub(super) fn create_snippet(
    lib: &mut KudzuLibrary,
    args: &Args,
    now: chrono::DateTime<Utc>,
) -> Result<Value, String> {
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
    let group = string(args, "group")?
        .map(str::trim)
        .filter(|g| !g.is_empty());
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

pub(super) fn update_snippet(
    lib: &mut KudzuLibrary,
    args: &Args,
    now: chrono::DateTime<Utc>,
) -> Result<Value, String> {
    let raw_id = string(args, "id")?.ok_or("Pass the `id` of the snippet to edit.")?;
    let id = parse_id(raw_id, "snippet")?;
    let mut s = lib
        .snippets
        .iter()
        .find(|s| s.id == id)
        .cloned()
        .ok_or_else(|| format!("No snippet has the id {id}."))?;
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
        let g = string(args, "group")?
            .map(str::trim)
            .filter(|g| !g.is_empty());
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
