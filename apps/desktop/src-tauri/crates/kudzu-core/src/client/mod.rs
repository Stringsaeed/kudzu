//! What the web UI computed client-side (`apps/desktop/lib`), for the native app.
//!
//! One module per TypeScript file, with the same behaviour: ranking, ordering, labels and
//! edge cases. JS semantics the UI relied on (UTF-16 offsets, `\s`, `Math.round`, `toFixed`,
//! `localeCompare`, en-US `Intl` output) live in [`js`]. Everything is pure: callers pass the
//! current time (ms since the epoch) and a time zone where the UI read the clock.
//!
//! - [`search`]: fuzzy snippet ranking, plus the palette's `#group` filter and scopes and the
//!   library list's views and sorts.
//! - [`groups`]: the group tree, paths, enablement and counts.
//! - [`stats`]: usage totals, time saved, durations, counts and relative times.
//! - [`keys`]: shortcut labels and matching, accelerators from key events.
//! - [`clipboard`]: history filters, search operators, day sections and row text.
//! - [`template`]: the editor's tokenizer, chips and summaries.
//! - [`formatting`], [`editor_formatting`]: span validation, parts, HTML and editor commands.
//! - [`updates`], [`apps`], [`theme`]: release notes, app names and appearance.

pub mod apps;
pub mod clipboard;
pub mod editor_formatting;
pub mod formatting;
pub mod groups;
pub mod js;
pub mod keys;
pub mod search;
pub mod stats;
pub mod template;
pub mod theme;
pub mod updates;
mod url;
