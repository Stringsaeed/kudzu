//! The Model Context Protocol (MCP) server that lets AI agents use the library: list, read,
//! expand, create and edit snippets, and search clipboard history.
//!
//! Transport-agnostic: [`handle`] takes one JSON-RPC message and returns the reply, if any.
//! The app serves it on a Unix socket, and `Kudzu --mcp` bridges an agent's stdio to that
//! socket (answering through the offline handler while Kudzu isn't running).
//!
//! Everything sent to an agent passes through [`kudzu_core::secrets`]: text that looks like a
//! password, key or token is replaced with a `[redacted: …]` marker, and search only sees the
//! redacted text, so a query can't probe for a secret either.

mod service;
pub use service::{AgentConfig, AgentError, AgentStatus, AgentsService};
pub mod bridge;
mod setup;
mod socket;
pub use setup::{AgentSetup, SkillState};

mod tools;

use std::collections::HashMap;

use chrono::{DateTime, FixedOffset};
use serde_json::{json, Value};

pub use tools::definitions;

/// The agent skill (`skills/kudzu/SKILL.md`): served as an MCP resource, installed
/// by Settings ▸ AI agents, and bundled in `Kudzu.app/Contents/Resources/skills/kudzu/`.
pub const SKILL: &str = include_str!("../../../../skills/kudzu/SKILL.md");
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
        Self {
            clipboard: true,
            write: true,
            trigger_prefix: ";".into(),
        }
    }
}

/// Inputs for rendering a snippet as if it were expanded now.
#[derive(Clone)]
pub struct RenderEnv {
    pub now: DateTime<FixedOffset>,
    pub locale: String,
    /// Plain-text clipboard, or `None` when it's empty or marked private.
    pub clipboard: Option<String>,
}

/// Why an offline bridge cannot answer tool calls.
pub const OFFLINE_MESSAGE: &str = "Kudzu isn't running, or AI agent access is turned off. Open Kudzu, go to Settings ▸ AI agents, and turn on “Allow AI agents”.";

/// Guidance sent with `initialize`; agents show it to the model.
pub const INSTRUCTIONS: &str = r#"Kudzu is the user's text expander and clipboard manager on macOS. Snippets expand when the user types a trigger (like ";sig") in any app.

- Use list_snippets / get_snippet to find existing snippets before writing new ones, and reuse text the user already keeps there (signatures, addresses, boilerplate replies).
- Use expand_snippet to get a snippet's final text (dates filled in, fill-in fields from `values`).
- Create or edit snippets only when the user asks. Pick short, memorable triggers that start with the user's prefix and aren't already taken.
- Snippet content is a template: literal text plus tags such as {{date format="yyyy-MM-dd"}}, {{time}}, {{clipboard}}, {{cursor}}, {{field name="Name" default="…"}}, {{choice name="Tone" options="Formal|Casual"}} and {{snippet id="<uuid>"}}. Write \{{ for a literal {{.
- A snippet can also press keys while it is inserted ({{key name="tab"}}, {{key name="return"}}) and pause ({{delay ms=300}}), to fill forms. expand_snippet leaves those steps out of `text` and lists them in `skippedActions`.
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
fn handle(
    host: Option<&service::ServiceHost>,
    server_version: &str,
    message: &str,
) -> Option<String> {
    let reply = match serde_json::from_str::<Value>(message) {
        Err(_) => Some(error(Value::Null, -32700, "Parse error")),
        Ok(Value::Object(m)) => dispatch(host, server_version, &m),
        Ok(_) => Some(error(
            Value::Null,
            -32600,
            "Invalid request: expected one JSON-RPC object",
        )),
    };
    reply.map(|v| v.to_string())
}

fn dispatch(
    host: Option<&service::ServiceHost>,
    server_version: &str,
    m: &serde_json::Map<String, Value>,
) -> Option<Value> {
    let method = match m.get("method") {
        Some(Value::String(method)) => method.as_str(),
        Some(_) => {
            return m.get("id").map(|id| {
                error(
                    id.clone(),
                    -32600,
                    "Invalid request: method must be a string",
                )
            })
        }
        // A response to a request this server never sends: nothing to say back.
        None => {
            return (!is_response(m)).then(|| {
                error(
                    m.get("id").cloned().unwrap_or(Value::Null),
                    -32600,
                    "Invalid request",
                )
            })
        }
    };
    let id = m.get("id").cloned();
    let params = m.get("params").cloned().unwrap_or_else(|| json!({}));
    let result = match method {
        "initialize" => Ok(initialize(&params, server_version)),
        "ping" => Ok(json!({})),
        "tools/list" => Ok(
            json!({ "tools": definitions(&host.and_then(|h| h.access().ok()).unwrap_or_else(Access::all)) }),
        ),
        "tools/call" => tools::call(host, &params),
        "resources/list" => Ok(json!({ "resources": [skill_resource()] })),
        "resources/read" => read_resource(&params),
        "resources/templates/list" => Ok(json!({ "resourceTemplates": [] })),
        "prompts/list" => Ok(json!({ "prompts": [] })),
        _ if id.is_none() => return None,
        _ => Err((-32601, "Method not found".to_owned())),
    };
    let id = id?;
    Some(match result {
        Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
        Err((code, message)) => error(id, code, &message),
    })
}

fn initialize(params: &Value, server_version: &str) -> Value {
    let requested = params
        .get("protocolVersion")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let version = PROTOCOL_VERSIONS
        .iter()
        .find(|v| **v == requested)
        .copied()
        .unwrap_or(PROTOCOL_VERSIONS[0]);
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
    let uri = params
        .get("uri")
        .and_then(Value::as_str)
        .ok_or((-32602, "Missing resource uri".to_string()))?;
    if uri != SKILL_URI {
        return Err((-32002, "Resource not found".to_owned()));
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

#[cfg(test)]
mod test_support;
