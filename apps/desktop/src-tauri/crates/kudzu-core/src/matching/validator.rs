//! Validation of triggers typed into the editor.

use unicode_segmentation::UnicodeSegmentation;

use super::KeyBuffer;

/// Why a trigger typed into the editor was rejected.
#[derive(Debug, Clone, PartialEq, Eq, Hash, thiserror::Error)]
pub enum TriggerIssue {
    /// Nothing was entered.
    #[error("Enter a trigger first.")]
    Empty,
    /// Only spaces, tabs or newlines were entered.
    #[error("A trigger can't be only spaces.")]
    WhitespaceOnly,
    /// The snippet already has this trigger.
    #[error("This snippet already has that trigger.")]
    Duplicate,
    /// Longer than [`MAX_TRIGGER_LENGTH`], so it could never fire.
    #[error(
        "Triggers can be at most {} characters; this one has {length}.",
        MAX_TRIGGER_LENGTH
    )]
    TooLong { length: usize },
}

impl TriggerIssue {
    /// A short sentence suitable for inline display.
    pub fn message(&self) -> String {
        self.to_string()
    }
}

/// Longest usable trigger: the key buffer holds this many clusters, so longer ones never match.
pub const MAX_TRIGGER_LENGTH: usize = KeyBuffer::DEFAULT_CAPACITY;

/// Returns the trimmed trigger or why it was rejected.
///
/// Length counts grapheme clusters. Duplicates compare case-insensitively unless `case_sensitive`.
pub fn validate_trigger(
    raw: &str,
    existing: &[String],
    case_sensitive: bool,
) -> Result<String, TriggerIssue> {
    if raw.is_empty() {
        return Err(TriggerIssue::Empty);
    }
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(TriggerIssue::WhitespaceOnly);
    }
    let length = trimmed.graphemes(true).count();
    if length > MAX_TRIGGER_LENGTH {
        return Err(TriggerIssue::TooLong { length });
    }
    let is_duplicate = existing.iter().any(|other| {
        if case_sensitive {
            other == trimmed
        } else {
            other.to_lowercase() == trimmed.to_lowercase()
                || other.to_uppercase() == trimmed.to_uppercase()
        }
    });
    if is_duplicate {
        return Err(TriggerIssue::Duplicate);
    }
    Ok(trimmed.to_string())
}

/// Splits pasted or typed text on whitespace into candidate triggers.
pub fn trigger_tokens(text: &str) -> Vec<String> {
    text.split_whitespace().map(String::from).collect()
}
