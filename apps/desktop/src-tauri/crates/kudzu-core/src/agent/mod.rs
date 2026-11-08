//! The Model Context Protocol (MCP) server that lets AI agents use the library: list, read,
//! expand, create and edit snippets, and search clipboard history.
//!
//! Transport-agnostic: [`handle`] takes one JSON-RPC message and returns the reply, if any.
//! The app serves it on a Unix socket, and `Kudzu --mcp` bridges an agent's stdio to that
//! socket (answering through [`Offline`] while Kudzu isn't running).
//!
//! Everything sent to an agent passes through [`crate::secrets`]: text that looks like a
//! password, key or token is replaced with a `[redacted: …]` marker, and search only sees the
//! redacted text, so a query can't probe for a secret either.

mod tools;

use std::collections::HashMap;

use chrono::{DateTime, FixedOffset};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::clipboard::ClipEntry;
use crate::KudzuLibrary;

pub use tools::definitions;

/// The agent skill (`apps/desktop/skills/kudzu/SKILL.md`): served as an MCP resource, installed
/// by Settings ▸ AI agents, and bundled in `Kudzu.app/Contents/Resources/skills/kudzu/`.
pub const SKILL: &str = include_str!("../../../../../skills/kudzu/SKILL.md");
/// The skill's resource URI.
pub const SKILL_URI: &str = "kudzu://skill/SKILL.md";

/// Protocol revisions this server speaks, newest first.
pub const PROTOCOL_VERSIONS: [&str; 4] = ["2025-11-25", "2025-06-18", "2025-03-26", "2024-11-05"];

/// What the user lets agents do (Settings ▸ AI agents).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Access {
    /// Agents may search clipboard history.
    pub clipboard: bool,
    /// Agents may create and edit snippets.
    pub write: bool,
    /// The user's usual trigger prefix (`;`), suggested to agents for new triggers.
    pub trigger_prefix: String,
}

impl Access {
    /// Everything allowed: what an offline bridge advertises.
    pub fn all() -> Self {
        Self { clipboard: true, write: true, trigger_prefix: ";".into() }
    }
}

/// Inputs for rendering a snippet as if it were expanded now.
pub struct RenderEnv {
    pub now: DateTime<FixedOffset>,
    pub locale: String,
    /// Plain-text clipboard, or `None` when it's empty or marked private.
    pub clipboard: Option<String>,
}

/// A library edit: changes the library in place and returns the tool's result.
pub type Edit<'a> = &'a mut dyn FnMut(&mut KudzuLibrary) -> Result<Value, String>;

/// What the server needs from the app. Every method fails with a user-facing message when the
/// app can't answer (not running, or access turned off).
pub trait Host {
    fn access(&self) -> Result<Access, String>;
    fn library(&self) -> Result<KudzuLibrary, String>;
    /// Runs `edit` on the live library and saves it when it succeeds.
    fn edit_library(&self, edit: Edit<'_>) -> Result<Value, String>;
    /// Clipboard history, newest first.
    fn clipboard(&self) -> Result<Vec<ClipEntry>, String>;
    /// The full text of a clipboard entry (entries carry an excerpt).
    fn clipboard_text(&self, id: Uuid) -> Result<Option<String>, String>;
    fn render_env(&self) -> RenderEnv;
}

/// The host used when Kudzu can't be reached: tools are listed, and calls explain what to do.
pub struct Offline;

/// Why [`Offline`] can't answer.
pub const OFFLINE_MESSAGE: &str = "Kudzu isn't running, or AI agent access is turned off. Open Kudzu, go to Settings ▸ AI agents, and turn on “Allow AI agents”.";

impl Host for Offline {
    fn access(&self) -> Result<Access, String> {
        Err(OFFLINE_MESSAGE.into())
    }
    fn library(&self) -> Result<KudzuLibrary, String> {
        Err(OFFLINE_MESSAGE.into())
    }
    fn edit_library(&self, _: Edit<'_>) -> Result<Value, String> {
        Err(OFFLINE_MESSAGE.into())
    }
    fn clipboard(&self) -> Result<Vec<ClipEntry>, String> {
        Err(OFFLINE_MESSAGE.into())
    }
    fn clipboard_text(&self, _: Uuid) -> Result<Option<String>, String> {
        Err(OFFLINE_MESSAGE.into())
    }
    fn render_env(&self) -> RenderEnv {
        RenderEnv { now: chrono::Local::now().fixed_offset(), locale: "en_US".into(), clipboard: None }
    }
}

/// Guidance sent with `initialize`; agents show it to the model.
pub const INSTRUCTIONS: &str = r#"Kudzu is the user's text expander and clipboard manager on macOS. Snippets expand when the user types a trigger (like ";sig") in any app.

- Use list_snippets / get_snippet to find existing snippets before writing new ones, and reuse text the user already keeps there (signatures, addresses, boilerplate replies).
- Use expand_snippet to get a snippet's final text (dates filled in, fill-in fields from `values`).
- Create or edit snippets only when the user asks. Pick short, memorable triggers that start with the user's prefix and aren't already taken.
- Snippet content is a template: literal text plus tags such as {{date format="yyyy-MM-dd"}}, {{time}}, {{clipboard}}, {{cursor}}, {{field name="Name" default="…"}}, {{choice name="Tone" options="Formal|Casual"}} and {{snippet id="<uuid>"}}. Write \{{ for a literal {{.
- list_clipboard / get_clipboard_item search what the user copied recently (only when they've allowed it).
- The resource kudzu://skill/SKILL.md has fuller guidance on triggers, templates and groups.
- Text that looks like a password, API key, token or private key is replaced with "[redacted: …]". The original can't be retrieved through this server; don't ask for it or try to reconstruct it, and never write a redaction marker into a snippet."#;

/// Whether [`handle`] answers `message` (requests and malformed input do; notifications and
/// responses don't). The bridge uses it to know when to wait for a reply.
pub fn expects_reply(message: &str) -> bool {
    match serde_json::from_str::<Value>(message) {
        Ok(Value::Object(m)) => match m.get("method") {
            Some(_) => m.contains_key("id"),
            None => !is_response(&m),
        },
        _ => true,
    }
}

fn is_response(m: &serde_json::Map<String, Value>) -> bool {
    m.contains_key("id") && (m.contains_key("result") || m.contains_key("error"))
}

/// Handles one JSON-RPC message and returns the serialized reply, if one is due.
pub fn handle(host: &dyn Host, server_version: &str, message: &str) -> Option<String> {
    let reply = match serde_json::from_str::<Value>(message) {
        Err(_) => Some(error(Value::Null, -32700, "Parse error")),
        Ok(Value::Object(m)) => dispatch(host, server_version, &m),
        Ok(_) => Some(error(Value::Null, -32600, "Invalid request: expected one JSON-RPC object")),
    };
    reply.map(|v| v.to_string())
}

fn dispatch(host: &dyn Host, server_version: &str, m: &serde_json::Map<String, Value>) -> Option<Value> {
    let method = match m.get("method") {
        Some(Value::String(method)) => method.as_str(),
        Some(_) => return m.get("id").map(|id| error(id.clone(), -32600, "Invalid request: method must be a string")),
        // A response to a request this server never sends: nothing to say back.
        None => return (!is_response(m)).then(|| error(m.get("id").cloned().unwrap_or(Value::Null), -32600, "Invalid request")),
    };
    let id = m.get("id").cloned();
    let params = m.get("params").cloned().unwrap_or_else(|| json!({}));
    let result = match method {
        "initialize" => Ok(initialize(&params, server_version)),
        "ping" => Ok(json!({})),
        "tools/list" => Ok(json!({ "tools": definitions(&host.access().unwrap_or_else(|_| Access::all())) })),
        "tools/call" => tools::call(host, &params),
        "resources/list" => Ok(json!({ "resources": [skill_resource()] })),
        "resources/read" => read_resource(&params),
        "resources/templates/list" => Ok(json!({ "resourceTemplates": [] })),
        "prompts/list" => Ok(json!({ "prompts": [] })),
        _ if id.is_none() => return None,
        _ => Err((-32601, format!("Method not found: {method}"))),
    };
    let id = id?;
    Some(match result {
        Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
        Err((code, message)) => error(id, code, &message),
    })
}

fn initialize(params: &Value, server_version: &str) -> Value {
    let requested = params.get("protocolVersion").and_then(Value::as_str).unwrap_or_default();
    let version = PROTOCOL_VERSIONS.iter().find(|v| **v == requested).copied().unwrap_or(PROTOCOL_VERSIONS[0]);
    json!({
        "protocolVersion": version,
        "capabilities": { "tools": { "listChanged": false }, "resources": { "subscribe": false, "listChanged": false } },
        "serverInfo": { "name": "kudzu", "title": "Kudzu", "version": server_version },
        "instructions": INSTRUCTIONS,
    })
}

fn skill_resource() -> Value {
    json!({
        "uri": SKILL_URI,
        "name": "kudzu-skill",
        "title": "Kudzu skill",
        "description": "How and when to use Kudzu's tools: finding and expanding snippets, writing triggers and templates, clipboard history, and redacted secrets.",
        "mimeType": "text/markdown",
        "size": SKILL.len(),
    })
}

fn read_resource(params: &Value) -> Result<Value, (i64, String)> {
    let uri = params.get("uri").and_then(Value::as_str).ok_or((-32602, "Missing resource uri".to_string()))?;
    if uri != SKILL_URI {
        return Err((-32002, format!("Resource not found: {uri}")));
    }
    Ok(json!({ "contents": [{ "uri": SKILL_URI, "mimeType": "text/markdown", "text": SKILL }] }))
}

fn error(id: Value, code: i64, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

/// Fill-in values passed to `expand_snippet`.
pub(crate) type Values = HashMap<String, String>;

#[cfg(test)]
mod tests;
