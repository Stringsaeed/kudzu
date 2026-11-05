//! Rolling buffer of the most recently typed grapheme clusters, plus per-cluster folding.

use std::borrow::Cow;
use std::collections::VecDeque;

use unicode_normalization::{is_nfc, UnicodeNormalization};
use unicode_segmentation::UnicodeSegmentation;

use super::KeyInput;

/// One buffered grapheme cluster with its precomputed lookup keys.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Cluster {
    /// As typed.
    pub text: String,
    /// NFC, so canonically equivalent input matches (Swift `Character` equality).
    pub exact: String,
    /// NFC of the case-folded cluster.
    pub folded: String,
}

impl Cluster {
    fn new(text: &str) -> Cluster {
        Cluster {
            text: text.to_string(),
            exact: nfc(text).into_owned(),
            folded: folded_key(text),
        }
    }
}

/// Lowercases one grapheme cluster without changing the cluster count (Swift `TriggerFolding`).
pub(crate) fn fold(cluster: &str) -> Cow<'_, str> {
    if cluster.len() == 1 {
        return if cluster.as_bytes()[0].is_ascii_uppercase() {
            Cow::Owned(cluster.to_ascii_lowercase())
        } else {
            Cow::Borrowed(cluster)
        };
    }
    let lowered = cluster.to_lowercase();
    if lowered == cluster {
        Cow::Borrowed(cluster)
    } else if lowered.graphemes(true).count() == 1 {
        Cow::Owned(lowered)
    } else {
        Cow::Borrowed(cluster)
    }
}

/// Canonical composition, borrowing when the text is already NFC.
pub(crate) fn nfc(text: &str) -> Cow<'_, str> {
    if is_nfc(text) {
        Cow::Borrowed(text)
    } else {
        Cow::Owned(text.nfc().collect())
    }
}

/// Lookup key of one cluster for case-insensitive matching.
pub(crate) fn folded_key(cluster: &str) -> String {
    nfc(&fold(cluster)).into_owned()
}

/// Lookup key of a whole string for case-insensitive comparison (folded cluster by cluster).
pub(crate) fn folded_string(text: &str) -> String {
    text.graphemes(true).map(folded_key).collect()
}

/// Rolling buffer of the most recently typed grapheme clusters, oldest first.
///
/// Each keystroke costs an append plus, once full, dropping the oldest cluster.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyBuffer {
    capacity: usize,
    clusters: VecDeque<Cluster>,
}

impl Default for KeyBuffer {
    fn default() -> Self {
        Self::new()
    }
}

impl KeyBuffer {
    /// Default capacity in grapheme clusters; longer triggers can never fire.
    pub const DEFAULT_CAPACITY: usize = 64;

    /// An empty buffer with the default capacity.
    pub fn new() -> Self {
        Self::with_capacity(Self::DEFAULT_CAPACITY)
    }

    /// An empty buffer keeping at most `capacity` (≥ 1) clusters.
    pub fn with_capacity(capacity: usize) -> Self {
        let capacity = capacity.max(1);
        KeyBuffer {
            capacity,
            clusters: VecDeque::with_capacity(capacity + 4),
        }
    }

    /// Maximum number of clusters retained.
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Number of buffered clusters.
    pub fn len(&self) -> usize {
        self.clusters.len()
    }

    /// Whether nothing is buffered.
    pub fn is_empty(&self) -> bool {
        self.clusters.is_empty()
    }

    /// The most recent cluster.
    pub fn last(&self) -> Option<&str> {
        self.clusters.back().map(|c| c.text.as_str())
    }

    /// Buffered clusters, oldest first.
    pub fn clusters(&self) -> impl Iterator<Item = &str> {
        self.clusters.iter().map(|c| c.text.as_str())
    }

    /// The buffered text.
    pub fn string(&self) -> String {
        self.clusters().collect()
    }

    pub(crate) fn cluster(&self, index: usize) -> &Cluster {
        &self.clusters[index]
    }

    /// Text of clusters `start..end`.
    pub(crate) fn text(&self, start: usize, end: usize) -> String {
        self.clusters
            .range(start..end)
            .map(|c| c.text.as_str())
            .collect()
    }

    /// Applies one input: appends text, drops the last cluster, or clears.
    pub fn apply(&mut self, input: &KeyInput) {
        match input {
            KeyInput::Character(text) => self.append(text),
            KeyInput::Backspace => self.remove_last(),
            KeyInput::Reset => self.reset(),
        }
    }

    /// Appends typed text, merging a leading combining mark / joiner / second regional indicator
    /// into the previous cluster, so the buffer holds whole clusters as a backspace would see them.
    pub fn append(&mut self, text: &str) {
        let Some(first) = text.chars().next() else {
            return;
        };
        // Fast path: only a scalar at or above U+0300 (or LF after CR) can extend the previous cluster.
        if u32::from(first) >= 0x300 || first == '\n' {
            if let Some(previous) = self.clusters.back() {
                let joined = format!("{}{}", previous.text, text);
                let previous_scalars = previous.text.chars().count();
                if joined
                    .graphemes(true)
                    .next()
                    .is_some_and(|head| head.chars().count() > previous_scalars)
                {
                    self.clusters.pop_back();
                    self.push_all(&joined);
                    return;
                }
            }
        }
        self.push_all(text);
    }

    fn push_all(&mut self, text: &str) {
        for cluster in text.graphemes(true) {
            self.clusters.push_back(Cluster::new(cluster));
        }
        while self.clusters.len() > self.capacity {
            self.clusters.pop_front();
        }
    }

    /// Drops the last cluster, if any.
    pub fn remove_last(&mut self) {
        self.clusters.pop_back();
    }

    /// Clears the buffer.
    pub fn reset(&mut self) {
        self.clusters.clear();
    }
}
