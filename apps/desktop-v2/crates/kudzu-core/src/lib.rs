//! Kudzu core: the pure domain. Rules and algorithms only, grouped by domain, with no OS, UI or
//! async dependency. The application layer (`kudzu-app`) owns state, persistence and
//! coordination; the engine (`kudzu-engine`) owns the keystroke path. Both build on this crate.
//!
//! - [`library`]: the library model (snippets, groups, usage, app rules), its JSON codec, group
//!   inheritance, trigger validation and conflicts, usage statistics, dedupe, naming, and the
//!   starter snippets.
//! - [`template`]: template parser, serializer, renderer, analysis, the ICU date formatter, form
//!   fields and form memory, and the editor document (`template::doc::EditorDoc`).
//! - [`matching`]: keystroke buffer, trigger index, matcher, case propagation, undo, and the
//!   matching configuration with its defaults.
//! - [`formatting`]: formatting spans over a template and their HTML output.
//! - [`interchange`]: Espanso, Raycast and TextExpander readers, the Espanso writer, rich-text
//!   readers, format detection, merging an import into a library, and backup naming.
//! - [`clipboard`]: the clipboard history model: classification, history, retention, blobs;
//!   query, sections and facts arrive in S22.
//! - [`secrets`]: password, key and token detection, so secrets are withheld from AI agents.
//! - [`sync`]: the iCloud sync model: records, the local index and merge rules.
//! - [`activity`]: the in-memory log of expansion attempts and the explanation of each outcome.
//! - [`system`]: the vocabulary shared with the engine and the application layer.
//! - [`search`], [`color`], [`keys`], [`release_notes`], [`fmt`]: pure helpers filled by S21 and
//!   S24.
//!
//! Persisted formats (library JSON, the clipboard index, sync payloads, form memory, backup
//! names) are pinned byte-for-byte by `tests/golden_formats.rs`; change them only on purpose.

pub mod activity;
pub mod clipboard;
pub mod color;
pub mod fmt;
pub mod formatting;
pub mod interchange;
pub mod keys;
pub mod library;
pub mod matching;
pub mod release_notes;
pub mod search;
pub mod secrets;
pub mod sync;
pub mod system;
pub mod template;

pub use library::model::{
    AppRule, AppRuleMode, CaseMode, FireMode, GroupRecord, InsertionMethod, KudzuLibrary,
    SnippetRecord, UsageRecord,
};

#[cfg(test)]
extern crate self as kudzu_core;

#[cfg(test)]
#[path = "../tests/unit/mod.rs"]
mod tests;
