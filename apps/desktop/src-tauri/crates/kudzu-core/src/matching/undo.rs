//! One-shot undo of the last expansion.

use unicode_segmentation::UnicodeSegmentation;

use super::{KeyInput, TriggerMatch, UndoAction};

#[derive(Debug, Clone, PartialEq, Eq)]
struct Pending {
    inserted_count: usize,
    typed_trigger: String,
    separator: Option<String>,
}

/// Remembers the last expansion so an immediately following backspace can revert it.
///
/// The backspace that triggers the undo must be swallowed by the app (it is accounted for in
/// `UndoAction::delete_count`); synthesized undo output must not be fed back as input.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UndoTracker {
    pending: Option<Pending>,
}

impl UndoTracker {
    /// A tracker with nothing pending.
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether the next backspace would undo an expansion.
    pub fn has_pending(&self) -> bool {
        self.pending.is_some()
    }

    /// Records an expansion; `inserted_count` is in grapheme clusters, excluding a re-emitted separator.
    pub fn record_expansion(&mut self, m: &TriggerMatch, inserted_count: usize) {
        self.pending = Some(Pending {
            inserted_count,
            typed_trigger: m.typed_trigger.clone(),
            separator: m.separator.clone(),
        });
    }

    /// Feeds the next input: a backspace directly after an expansion yields an undo; any input clears it.
    pub fn consume(&mut self, input: &KeyInput) -> Option<UndoAction> {
        let pending = self.pending.take()?;
        if *input != KeyInput::Backspace {
            return None;
        }
        let separator = pending.separator.unwrap_or_default();
        Some(UndoAction {
            delete_count: pending.inserted_count + separator.graphemes(true).count(),
            retype: pending.typed_trigger + &separator,
        })
    }

    /// Forgets any pending undo.
    pub fn clear(&mut self) {
        self.pending = None;
    }
}
