//! Kudzu core: pure, platform-independent logic shared by the Tauri app and the engine.
//!
//! - [`types`]: the library contract (snippets, groups, usage, app rules).
//! - [`template`]: template parser, serializer, renderer and static analysis.
//! - [`matching`]: keystroke buffer, trigger index, case propagation, undo, conflicts.
//! - [`interchange`]: Kudzu JSON, Espanso import/export, backups, dedupe.
//! - [`clipboard`]: clipboard history model: classification, timings, retention, blobs.
//! - [`secrets`]: password, key and token detection, so secrets are withheld from AI agents.
//! - [`agent`]: the MCP server that lets AI agents read and write snippets.

pub mod agent;
pub mod clipboard;
pub mod formatting;
pub mod interchange;
pub mod matching;
pub mod secrets;
pub mod sync;
pub mod template;
pub mod types;

pub use types::*;
