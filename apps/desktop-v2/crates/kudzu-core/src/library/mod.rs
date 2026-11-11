//! The library domain: what a snippet library is and the rules that keep it consistent.
//!
//! - [`model`]: the records (snippet, group, usage, app rule) and the ISO-8601 date codec. The
//!   JSON shape of these types is the library file format; changes must be additive.
//! - [`json`]: the Kudzu JSON codec (deterministic, sorted keys).
//! - [`groups`]: inheritance of enablement, app rules and insertion through the group tree, and
//!   the tree itself: ordered nodes, flattening, paths, descendant sets and recursive counts.
//! - [`triggers`]: validation of triggers typed into the editor.
//! - [`conflicts`]: duplicate and shadowed triggers.
//! - [`usage`]: usage totals across devices, time saved and the palette's usage rank, plus the
//!   Insights aggregation (`usage_by_id`, `totals`).
//! - [`dedupe`]: deterministic duplicate detection and merging after sync or import.
//! - [`naming`]: collision-free names and triggers for duplicates and new snippets.
//! - [`starter`]: the Getting Started snippets.
//!
//! The record types are re-exported at the crate root (`kudzu_core::SnippetRecord`).

pub mod conflicts;
pub mod dedupe;
pub mod groups;
pub mod json;
pub mod model;
pub mod naming;
pub mod starter;
pub mod triggers;
pub mod usage;
