//! The clipboard index: folding repeats into one entry, secrets, pins and use.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{
    Classified, ClipEntry, ClipKind, ClipSource, ImageSize, MAX_TIMINGS, TEXT_EXCERPT_LIMIT,
};
use crate::secrets::{self, SecretKind};

/// Cuts `text` to at most `limit` characters on a char boundary.
pub fn excerpt(text: &str, limit: usize) -> String {
    match text.char_indices().nth(limit) {
        Some((i, _)) => text[..i].to_string(),
        None => text.to_string(),
    }
}

/// The result of recording a copy.
#[derive(Debug, Clone, PartialEq)]
pub enum Ingest {
    /// New content.
    Added(ClipEntry),
    /// Content already in the history: moved to the top with a new timing.
    Repeated(ClipEntry),
}

impl Ingest {
    pub fn entry(&self) -> &ClipEntry {
        match self {
            Ingest::Added(e) | Ingest::Repeated(e) => e,
        }
    }
}

/// The clipboard history, newest copy first.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct History {
    #[serde(default = "History::format_version")]
    pub version: u32,
    /// The [`SECRET_RULES`] that last scanned the history; older entries are rescanned.
    #[serde(default)]
    pub secret_rules: u32,
    pub entries: Vec<ClipEntry>,
}

/// Bump when [`crate::secrets`] detection changes, so existing history is rescanned on launch.
pub const SECRET_RULES: u32 = 1;

/// Secret detection for one copy. Only text can be a secret; colours, files and images can't.
fn secret_of(kind: ClipKind, text: Option<&str>) -> Option<SecretKind> {
    match kind {
        ClipKind::Text | ClipKind::Code | ClipKind::Link | ClipKind::Other => {
            text.and_then(secrets::detect)
        }
        ClipKind::Color | ClipKind::Image | ClipKind::File => None,
    }
}

impl History {
    fn format_version() -> u32 {
        1
    }

    pub fn new() -> Self {
        Self {
            version: Self::format_version(),
            secret_rules: SECRET_RULES,
            entries: vec![],
        }
    }

    /// Rescans entries recorded under older detection rules (from their excerpts; longer
    /// copies were scanned in full when they were made). Returns whether anything changed.
    pub fn rescan_secrets(&mut self) -> bool {
        if self.secret_rules >= SECRET_RULES {
            return false;
        }
        for e in &mut self.entries {
            let found = secret_of(e.kind, e.text.as_deref());
            // Keep a detection made from the full text that the excerpt no longer shows.
            if found.is_some() || e.text_len <= TEXT_EXCERPT_LIMIT {
                e.secret = found;
            }
        }
        self.secret_rules = SECRET_RULES;
        true
    }

    /// Records the user's correction (`None` goes back to detection).
    pub fn set_secret(&mut self, id: Uuid, secret: Option<bool>) -> Option<ClipEntry> {
        let e = self.entries.iter_mut().find(|e| e.id == id)?;
        e.secret_override = secret;
        Some(e.clone())
    }

    pub fn get(&self, id: Uuid) -> Option<&ClipEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    /// The entry copied `n` copies before what is on the clipboard now (1 = the previous copy),
    /// for `{{clipboard history=N}}`. `current` is the clipboard's plain text. The newest entry is
    /// normally the current clipboard and is skipped; when it isn't (the copy came from an ignored
    /// app or was marked private, so it was never recorded), the newest entry is the previous copy.
    pub fn before_current(&self, n: usize, current: Option<&str>) -> Option<&ClipEntry> {
        let newest = self.entries.first()?;
        let newest_is_current = match (newest.text.as_deref(), current) {
            (Some(text), Some(current)) => text == excerpt(current, TEXT_EXCERPT_LIMIT),
            // An image or file on the clipboard has no plain text to compare.
            (None, None) => true,
            _ => false,
        };
        self.entries.get(if newest_is_current {
            n
        } else {
            n.checked_sub(1)?
        })
    }

    /// Records a copy made at `now`. Repeated content keeps its id, pin and history of timings.
    pub fn ingest(
        &mut self,
        c: &Classified,
        source: Option<ClipSource>,
        image: Option<ImageSize>,
        now: DateTime<Utc>,
    ) -> Ingest {
        let hash = format!("{:016x}", c.hash);
        let full_text = c.text.as_deref().unwrap_or_default();
        let fill = |e: &mut ClipEntry| {
            e.kind = c.kind;
            e.text = c.text.as_deref().map(|t| excerpt(t, TEXT_EXCERPT_LIMIT));
            e.text_len = full_text.chars().count();
            e.line_count = if full_text.is_empty() {
                0
            } else {
                full_text.lines().count().max(1)
            };
            e.files = c.files.clone();
            e.color = c.color.clone();
            e.rich = c.rich;
            e.types = c.types.clone();
            e.size = c.size;
            if image.is_some() {
                e.image = image;
            }
            e.secret = secret_of(c.kind, c.text.as_deref());
        };
        if let Some(pos) = self.entries.iter().position(|e| e.hash == hash) {
            let mut e = self.entries.remove(pos);
            fill(&mut e);
            e.copied_at = now;
            e.copy_count = e.copy_count.saturating_add(1);
            e.timings.push(now);
            if e.timings.len() > MAX_TIMINGS {
                let drop = e.timings.len() - MAX_TIMINGS;
                e.timings.drain(..drop);
            }
            if source.is_some() {
                e.source = source;
            }
            self.entries.insert(0, e.clone());
            return Ingest::Repeated(e);
        }
        let mut e = ClipEntry {
            id: Uuid::new_v4(),
            kind: c.kind,
            text: None,
            text_len: 0,
            line_count: 0,
            files: vec![],
            image: None,
            color: None,
            rich: false,
            types: vec![],
            size: 0,
            source,
            hash,
            first_copied_at: now,
            copied_at: now,
            copy_count: 1,
            timings: vec![now],
            last_used_at: None,
            use_count: 0,
            pinned: false,
            secret: None,
            secret_override: None,
        };
        fill(&mut e);
        self.entries.insert(0, e.clone());
        Ingest::Added(e)
    }

    /// Removes entries by id, returning them (in history order).
    pub fn remove(&mut self, ids: &[Uuid]) -> Vec<ClipEntry> {
        let (gone, kept) = self.entries.drain(..).partition(|e| ids.contains(&e.id));
        self.entries = kept;
        gone
    }

    /// Puts removed entries back in time order (undo). Ids already present are skipped.
    pub fn restore(&mut self, entries: Vec<ClipEntry>) -> Vec<ClipEntry> {
        let mut restored = Vec::new();
        for e in entries {
            if self
                .entries
                .iter()
                .any(|x| x.id == e.id || x.hash == e.hash)
            {
                continue;
            }
            restored.push(e.clone());
            self.entries.push(e);
        }
        self.entries.sort_by_key(|e| std::cmp::Reverse(e.copied_at));
        restored
    }

    /// Removes everything (pinned too when `include_pinned`), returning what was removed.
    pub fn clear(&mut self, include_pinned: bool) -> Vec<ClipEntry> {
        let (gone, kept) = self
            .entries
            .drain(..)
            .partition(|e| include_pinned || !e.pinned);
        self.entries = kept;
        gone
    }

    pub fn set_pinned(&mut self, id: Uuid, pinned: bool) -> Option<ClipEntry> {
        let e = self.entries.iter_mut().find(|e| e.id == id)?;
        e.pinned = pinned;
        Some(e.clone())
    }

    /// Notes that the user put this entry back on the clipboard; it moves to the top.
    pub fn mark_used(&mut self, id: Uuid, now: DateTime<Utc>) -> Option<ClipEntry> {
        let pos = self.entries.iter().position(|e| e.id == id)?;
        let mut e = self.entries.remove(pos);
        e.last_used_at = Some(now);
        e.use_count = e.use_count.saturating_add(1);
        e.copied_at = now;
        self.entries.insert(0, e.clone());
        Some(e)
    }
}
