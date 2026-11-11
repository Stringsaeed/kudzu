//! Clipboard serialization accepts only the S28 agent-safe shape.
use super::{boolean, count, parse_id, string, Args};
use crate::{agents::service::ServiceHost, clipboard::AgentItem};
use kudzu_core::{
    clipboard::query::{ClipFilter, ClipQuery},
    library::model::iso8601,
};
use serde_json::{json, Value};

fn summary(mut e: AgentItem, max: usize) -> Value {
    if let Some(text) = e.text.as_mut() {
        if let Some((end, _)) = text.char_indices().nth(max) {
            text.truncate(end);
            e.truncated = true;
        }
    }
    let mut v = json!({"id":e.id,"kind":e.kind,"copiedAt":iso8601::format(&e.copied_at),"copyCount":e.copy_count,"pinned":e.pinned,"text":e.text});
    for (key, value) in [
        ("length", json!(e.length)),
        ("lines", json!(e.lines)),
        ("app", json!(e.app)),
        ("image", json!(e.image)),
        ("color", json!(e.color)),
        ("withheld", json!(e.withheld)),
    ] {
        if !value.is_null() {
            v[key] = value;
        }
    }
    if !e.files.is_empty() {
        v["files"] = json!(e.files);
    }
    if !e.redacted.is_empty() {
        v["redacted"] = json!(e.redacted);
    }
    if e.truncated {
        v["truncated"] = json!(true);
    }
    v
}
pub(super) fn list_clipboard(host: &ServiceHost, args: &Args) -> Result<Value, String> {
    let limit = count(args, "limit", 20, 100)?.max(1);
    let kind = string(args, "kind")?
        .map(|s| {
            serde_json::from_value(json!(s)).map_err(|_| {
                "Unknown clipboard kind: use text, code, link, color, image, file or other."
                    .to_string()
            })
        })
        .transpose()?;
    let query = ClipQuery {
        terms: string(args, "query")?
            .unwrap_or_default()
            .to_lowercase()
            .split_whitespace()
            .map(str::to_owned)
            .collect(),
        app: None,
        kind,
    };
    let filter = if boolean(args, "pinnedOnly")?.unwrap_or(false) {
        ClipFilter::Pinned
    } else {
        ClipFilter::All
    };
    let entries = host.clipboard(filter, &query)?;
    let total = entries.len();
    let items: Vec<_> = entries
        .into_iter()
        .take(limit)
        .map(|e| summary(e, 320))
        .collect();
    Ok(json!({"total":total,"returned":items.len(),"items":items}))
}
pub(super) fn get_clipboard_item(host: &ServiceHost, args: &Args) -> Result<Value, String> {
    let raw =
        string(args, "id")?.ok_or("Pass the `id` of a clipboard item from list_clipboard.")?;
    let id = parse_id(raw, "clipboard item")?;
    host.clipboard_item(id)?
        .map(|e| summary(e, 50_000))
        .ok_or_else(|| "That item is no longer in the clipboard history.".into())
}
