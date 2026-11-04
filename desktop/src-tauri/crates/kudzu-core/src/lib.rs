//! Kudzu core: pure, platform-independent logic shared by the Tauri app and the engine.
//!
//! - [`types`]: the library contract (snippets, groups, usage, app rules).
//! - [`template`]: template parser, serializer, renderer and static analysis.
//! - [`matching`]: keystroke buffer, trigger index, case propagation, undo, conflicts.
//! - [`interchange`]: Kudzu JSON, Espanso import/export, backups, dedupe.
//! - [`clipboard`]: clipboard history model: classification, timings, retention, blobs.

pub mod clipboard;
pub mod interchange;
pub mod matching;
pub mod sync;
pub mod template;
pub mod types;

pub use types::*;
