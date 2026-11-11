//! Keystroke matching: the rolling key buffer, the reversed-trigger index, case propagation,
//! undo, and the matching configuration with its defaults.
//!
//! - [`config`]: [`MatchingConfig`] and the insertion precedence chain.
//! - `key_buffer`: the rolling buffer and per-cluster key folding (shared with
//!   `library::conflicts`, which compares triggers the way the matcher does).
//! - `index`: the reversed-trigger suffix tries behind [`TriggerIndex`].
//! - `undo`: one-shot undo of the last expansion.
//!
//! The engine drives [`ExpansionMatcher`] from its event-tap thread; everything here is
//! `Send`, and an index is cheap to clone.

pub mod config;
mod index;
pub(crate) mod key_buffer;
mod undo;

use std::sync::Arc;

use uuid::Uuid;

use crate::library::model::{GroupRecord, InsertionMethod, SnippetRecord};

pub use config::MatchingConfig;
pub use key_buffer::KeyBuffer;
pub use undo::UndoTracker;

/// One real keystroke as seen by the matcher.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyInput {
    /// Typed text (usually one grapheme cluster).
    Character(String),
    Backspace,
    /// Click, app switch, navigation key, modifier shortcut: the context is gone.
    Reset,
}

/// How the user capitalised a typed trigger.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CaseStyle {
    AsIs,
    Capitalized,
    Upper,
}

/// Detects trigger case and applies it to expansions.
pub mod case_propagation {
    use unicode_segmentation::UnicodeSegmentation;

    use super::CaseStyle;

    /// Style of `typed_trigger`, judged on cased letters only.
    ///
    /// Punctuation, digits and uncased scripts are ignored: all uppercase with ≥ 2 letters is
    /// `Upper`, an uppercase first letter is `Capitalized`, anything else `AsIs`.
    pub fn detect(typed_trigger: &str) -> CaseStyle {
        let mut letters = 0;
        let mut first_is_upper = false;
        let mut all_upper = true;
        for cluster in typed_trigger
            .graphemes(true)
            .filter(|c| is_letter(c) && is_cased(c))
        {
            let upper = is_uppercase(cluster);
            if letters == 0 {
                first_is_upper = upper;
            }
            all_upper &= upper;
            letters += 1;
        }
        if letters >= 2 && all_upper {
            CaseStyle::Upper
        } else if letters >= 1 && first_is_upper {
            CaseStyle::Capitalized
        } else {
            CaseStyle::AsIs
        }
    }

    /// Applies `style` to expansion `text`.
    ///
    /// `Capitalized` uppercases only the first letter, skipping leading whitespace and
    /// punctuation; text whose first letter-or-digit is a digit is left untouched.
    pub fn apply(style: CaseStyle, text: &str) -> String {
        match style {
            CaseStyle::AsIs => text.to_string(),
            CaseStyle::Upper => text.to_uppercase(),
            CaseStyle::Capitalized => {
                let first = text
                    .grapheme_indices(true)
                    .find(|(_, c)| is_letter(c) || is_number(c));
                match first {
                    Some((index, cluster)) if is_letter(cluster) => {
                        format!(
                            "{}{}{}",
                            &text[..index],
                            cluster.to_uppercase(),
                            &text[index + cluster.len()..]
                        )
                    }
                    _ => text.to_string(),
                }
            }
        }
    }

    fn is_letter(cluster: &str) -> bool {
        cluster.chars().next().is_some_and(char::is_alphabetic)
    }

    fn is_number(cluster: &str) -> bool {
        cluster.chars().next().is_some_and(char::is_numeric)
    }

    /// Whether the cluster changes under some case conversion (Swift `Character.isCased`).
    pub(crate) fn is_cased(cluster: &str) -> bool {
        if cluster.len() == 1 {
            return cluster.as_bytes()[0].is_ascii_alphabetic();
        }
        cluster.to_uppercase() != cluster || cluster.to_lowercase() != cluster
    }

    fn is_uppercase(cluster: &str) -> bool {
        if cluster.len() == 1 {
            return cluster.as_bytes()[0].is_ascii_uppercase();
        }
        is_cased(cluster) && cluster.to_uppercase() == cluster
    }
}

/// A trigger that just fired. See the Swift `TriggerMatch` docs for delete semantics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TriggerMatch {
    pub snippet_id: Uuid,
    /// The trigger as stored.
    pub trigger: String,
    /// The trigger exactly as typed.
    pub typed_trigger: String,
    /// Grapheme clusters (one backspace each) of the typed trigger; never includes the separator.
    pub delete_count: usize,
    /// The separator that completed a whole-word match; `None` for immediate.
    pub separator: Option<String>,
    /// Resolved insertion method for the frontmost app.
    pub insertion: InsertionMethod,
    pub case_style: CaseStyle,
    pub fire_mode: crate::library::model::FireMode,
}

impl TriggerMatch {
    /// Backspaces needed when the separator keystroke already reached the document.
    pub fn delete_count_including_separator(&self) -> usize {
        self.delete_count + usize::from(self.separator.is_some())
    }
}

/// Why a fully typed trigger did not fire.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SkipReason {
    /// The snippet itself is turned off.
    SnippetOff,
    /// The nearest group in its ancestry that is turned off.
    GroupOff { group_id: Uuid },
    /// An app rule or exclusion. `group_id` is set at [`RuleLevel::Group`](crate::activity::RuleLevel::Group).
    Rule {
        level: crate::activity::RuleLevel,
        group_id: Option<Uuid>,
    },
}

/// A trigger that was typed in full but may not fire here. Never holds the text as typed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkippedMatch {
    pub snippet_id: Uuid,
    /// The trigger as stored.
    pub trigger: String,
    pub cause: SkipReason,
}

/// What the app must do to revert the last expansion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UndoAction {
    /// Characters to backspace (inserted text + re-emitted separator).
    pub delete_count: usize,
    /// Text to type afterwards: trigger as typed + separator.
    pub retype: String,
}

/// What the matcher wants the app to do after a keystroke.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MatcherEvent {
    Expand(TriggerMatch),
    Undo(UndoAction),
}

/// Immutable trigger lookup; O(longest trigger) per keystroke.
#[derive(Debug, Clone)]
pub struct TriggerIndex {
    /// Shared so cloning an index (e.g. into the event-tap thread) is cheap.
    data: Arc<index::IndexData>,
}

impl TriggerIndex {
    /// Builds the index. Effectively disabled snippets never fire; they are kept only so
    /// [`find_skipped`](Self::find_skipped) can say why. Empty triggers are skipped.
    pub fn new(snippets: &[SnippetRecord], groups: &[GroupRecord], config: MatchingConfig) -> Self {
        TriggerIndex {
            data: Arc::new(index::IndexData::new(snippets, groups, config)),
        }
    }

    /// An index with no triggers.
    pub fn empty() -> Self {
        Self::new(&[], &[], MatchingConfig::default())
    }

    /// Number of (snippet, trigger) pairs that can fire.
    pub fn trigger_count(&self) -> usize {
        self.data.trigger_count()
    }

    /// The configuration the index was built with.
    pub fn config(&self) -> &MatchingConfig {
        &self.data.config
    }

    /// The trigger completed by `last_input`, which must already be applied to `buffer`.
    ///
    /// Backspace and reset never fire. Longest trigger wins, then the most recently updated snippet.
    pub fn find_match(
        &self,
        buffer: &KeyBuffer,
        last_input: &KeyInput,
        bundle_id: Option<&str>,
    ) -> Option<TriggerMatch> {
        self.data.find_match(buffer, last_input, bundle_id)
    }

    /// Why the trigger completed by `last_input` did not fire. Call only after
    /// [`find_match`](Self::find_match) returned `None` for the same keystroke.
    ///
    /// Reports full matches only (the same firing conditions as `find_match`), so a near-miss
    /// or ordinary typing yields `None`.
    pub fn find_skipped(
        &self,
        buffer: &KeyBuffer,
        last_input: &KeyInput,
        bundle_id: Option<&str>,
    ) -> Option<SkippedMatch> {
        self.data.find_skipped(buffer, last_input, bundle_id)
    }
}

/// Buffer + index + undo tracker: the single entry point for the event tap.
/// Never feed synthesized events back into `handle`.
#[derive(Debug, Clone)]
pub struct ExpansionMatcher {
    buffer: KeyBuffer,
    index: TriggerIndex,
    undo: UndoTracker,
    /// The last handled keystroke was typed text that fired nothing.
    missed: bool,
}

impl ExpansionMatcher {
    /// An inert matcher with no triggers.
    pub fn new() -> Self {
        Self::with_index(TriggerIndex::empty())
    }

    /// A matcher over `index`.
    pub fn with_index(index: TriggerIndex) -> Self {
        ExpansionMatcher {
            buffer: KeyBuffer::new(),
            index,
            undo: UndoTracker::new(),
            missed: false,
        }
    }

    /// The rolling key buffer.
    pub fn buffer(&self) -> &KeyBuffer {
        &self.buffer
    }

    /// Processes one real keystroke typed in the app with `bundle_id`.
    ///
    /// A backspace right after `did_expand` returns `Undo` and is not applied to the buffer (the app
    /// swallows it). Otherwise the buffer is updated and a completed trigger returns `Expand`.
    pub fn handle(&mut self, input: KeyInput, bundle_id: Option<&str>) -> Option<MatcherEvent> {
        self.missed = false;
        if let Some(undo) = self.undo.consume(&input) {
            // Mirror the document after the app retypes the trigger; no matching is attempted.
            self.buffer.reset();
            self.buffer.append(&undo.retype);
            return Some(MatcherEvent::Undo(undo));
        }
        self.buffer.apply(&input);
        let found = self.index.find_match(&self.buffer, &input, bundle_id);
        self.missed = found.is_none() && matches!(input, KeyInput::Character(_));
        found.map(MatcherEvent::Expand)
    }

    /// Why the keystroke just passed to [`handle`](Self::handle) fired nothing, when it completed
    /// a trigger that is turned off or not allowed in `bundle_id`. `None` for ordinary typing.
    pub fn skipped(&self, bundle_id: Option<&str>) -> Option<SkippedMatch> {
        if !self.missed {
            return None;
        }
        self.index
            .find_skipped(&self.buffer, &KeyInput::Character(String::new()), bundle_id)
    }

    /// Call after inserting the expansion for `m`; `inserted_count` in grapheme clusters, excluding a re-emitted separator.
    pub fn did_expand(&mut self, m: &TriggerMatch, inserted_count: usize) {
        // The inserted text is not fed back, so the buffer no longer mirrors the document.
        self.buffer.reset();
        self.undo.record_expansion(m, inserted_count);
    }

    /// Swaps in a freshly built index. Pending undo is dropped.
    pub fn replace_index(&mut self, index: TriggerIndex) {
        self.missed = false;
        self.index = index;
        self.undo.clear();
    }

    /// Clears the key buffer and pending undo (click, app switch, pause…).
    pub fn reset(&mut self) {
        self.missed = false;
        self.buffer.reset();
        self.undo.clear();
    }

    /// The buffered text (for diagnostics/tests).
    pub fn buffer_string(&self) -> String {
        self.buffer.string()
    }
}

impl Default for ExpansionMatcher {
    fn default() -> Self {
        Self::new()
    }
}
