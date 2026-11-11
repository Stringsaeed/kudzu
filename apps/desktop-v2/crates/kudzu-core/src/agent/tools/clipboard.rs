//! Clipboard tools: search the history and read one item, with secrets withheld.

use serde_json::{json, Value};

use super::{
    boolean, count, labels, parse_id, string, truncate, Args, MAX_ITEM_CHARS, PREVIEW_CHARS,
};
use crate::agent::Host;
use crate::clipboard::{ClipEntry, ClipKind, TEXT_EXCERPT_LIMIT};
use crate::library::model::iso8601;
use crate::secrets::{self, Redacted};

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
        Some(false) => Redacted {
            text: text.to_string(),
            kinds: vec![],
            whole: false,
        },
        Some(true) => Redacted {
            text: String::new(),
            kinds: vec![e.secret.unwrap_or(secrets::SecretKind::Credential)],
            whole: true,
        },
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
                Some(true) =>
                    "The user marked this as a secret, so Kudzu doesn't share it.".to_string(),
                _ => format!(
                    "This looks like a {}, so Kudzu doesn't share it.",
                    r.kinds.first().map(|k| k.label()).unwrap_or("secret")
                ),
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
    serde_json::from_value(json!(raw)).map_err(|_| {
        format!("Unknown kind “{raw}”: use text, code, link, color, image, file or other.")
    })
}

pub(super) fn list_clipboard(mut entries: Vec<ClipEntry>, args: &Args) -> Result<Value, String> {
    let limit = count(args, "limit", 20, 100)?.max(1);
    let kind = string(args, "kind")?.map(parse_kind).transpose()?;
    let pinned = boolean(args, "pinnedOnly")?.unwrap_or(false);
    let query = string(args, "query")?
        .unwrap_or_default()
        .trim()
        .to_lowercase();
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
            let text = e
                .text
                .as_deref()
                .map(|t| entry_redaction(e, t, complete))
                .filter(|r| !r.whole)
                .map(|r| r.text)
                .unwrap_or_default();
            let hay = format!(
                "{}\n{}\n{}",
                text,
                e.files.join("\n"),
                e.source
                    .as_ref()
                    .map(|s| s.name.as_str())
                    .unwrap_or_default()
            )
            .to_lowercase();
            if !terms.iter().all(|t| hay.contains(t)) {
                continue;
            }
        }
        total += 1;
        if items.len() < limit {
            items.push(clip_summary(
                e,
                e.text.as_deref(),
                complete,
                PREVIEW_CHARS * 2,
            ));
        }
    }
    Ok(json!({ "total": total, "returned": items.len(), "items": items }))
}

pub(super) fn get_clipboard_item(host: &dyn Host, args: &Args) -> Result<Value, String> {
    let raw =
        string(args, "id")?.ok_or("Pass the `id` of a clipboard item from list_clipboard.")?;
    let id = parse_id(raw, "clipboard item")?;
    let entries = host.clipboard()?;
    let e = entries
        .iter()
        .find(|e| e.id == id)
        .ok_or("That item is no longer in the clipboard history.")?;
    let full = host.clipboard_text(id)?;
    let (text, complete) = match full {
        Some(t) => (Some(t), true),
        None => (e.text.clone(), e.text_len <= TEXT_EXCERPT_LIMIT),
    };
    Ok(clip_summary(e, text.as_deref(), complete, MAX_ITEM_CHARS))
}
