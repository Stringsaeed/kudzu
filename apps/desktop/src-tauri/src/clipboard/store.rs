//! Clipboard history on disk:
//!
//! ```text
//! clipboard/            (0700: only this user can read it)
//!   history.json        index: metadata and short text excerpts (kudzu_core::clipboard::History)
//!   items/<id>.kzc      every pasteboard type, for exact restoration
//!   thumbs/<id>.png     image thumbnails
//!   trash/              deleted entries' files, kept until the next launch for Undo
//! ```
//!
//! The index is written atomically and debounced; blobs are written once per copy.

use std::fs;
use std::path::{Path, PathBuf};

use kudzu_core::clipboard::{decode_items, ClipEntry, History, PasteItem};
use uuid::Uuid;

use crate::store::write_atomic;

pub struct ClipStore {
    pub dir: PathBuf,
    pub history: History,
    dirty: bool,
}

impl ClipStore {
    /// Loads (or creates) the history in `dir`. An unreadable index is moved aside.
    pub fn load(dir: PathBuf) -> Self {
        for sub in ["items", "thumbs", "trash"] {
            let _ = fs::create_dir_all(dir.join(sub));
        }
        restrict(&dir);
        let index = dir.join("history.json");
        let history = match fs::read(&index) {
            Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_else(|e| {
                log::error!("clipboard history unreadable ({e}); starting fresh");
                let _ = fs::rename(&index, dir.join(format!("history-corrupt-{}.json", chrono::Utc::now().timestamp())));
                History::new()
            }),
            Err(_) => History::new(),
        };
        let mut history = history;
        // Entries from before secret detection (or older rules) are scanned once.
        let dirty = history.rescan_secrets();
        let store = Self { dir, history, dirty };
        store.empty_trash();
        store.remove_orphans();
        store
    }

    pub fn blob_path(&self, id: Uuid) -> PathBuf {
        self.dir.join("items").join(format!("{id}.kzc"))
    }

    pub fn thumb_path(&self, id: Uuid) -> PathBuf {
        self.dir.join("thumbs").join(format!("{id}.png"))
    }

    fn trash_path(&self, file: &Path) -> Option<PathBuf> {
        Some(self.dir.join("trash").join(file.file_name()?))
    }

    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    /// Writes the index if anything changed since the last save.
    pub fn save_if_dirty(&mut self) {
        if !self.dirty {
            return;
        }
        self.dirty = false;
        match serde_json::to_vec(&self.history) {
            Ok(json) => {
                if let Err(e) = write_atomic(&self.dir.join("history.json"), &json) {
                    log::error!("saving clipboard history failed: {e}");
                    self.dirty = true;
                }
            }
            Err(e) => log::error!("encoding clipboard history failed: {e}"),
        }
    }

    /// Raw pasteboard items of an entry.
    pub fn items(&self, id: Uuid) -> Option<Vec<PasteItem>> {
        decode_items(&fs::read(self.blob_path(id)).ok()?)
    }

    /// Moves entries' files to the trash (Undo can bring them back until the next launch).
    pub fn trash(&self, entries: &[ClipEntry]) {
        for e in entries {
            for file in [self.blob_path(e.id), self.thumb_path(e.id)] {
                if let Some(to) = self.trash_path(&file) {
                    let _ = fs::rename(&file, to);
                }
            }
        }
    }

    pub fn untrash(&self, entries: &[ClipEntry]) {
        for e in entries {
            for file in [self.blob_path(e.id), self.thumb_path(e.id)] {
                if let Some(from) = self.trash_path(&file) {
                    let _ = fs::rename(from, &file);
                }
            }
        }
    }

    /// Deletes entries' files for good (expiry, Clear history).
    pub fn purge(&self, entries: &[ClipEntry]) {
        for e in entries {
            let _ = fs::remove_file(self.blob_path(e.id));
            let _ = fs::remove_file(self.thumb_path(e.id));
        }
    }

    fn empty_trash(&self) {
        if let Ok(list) = fs::read_dir(self.dir.join("trash")) {
            for entry in list.flatten() {
                let _ = fs::remove_file(entry.path());
            }
        }
    }

    /// Files whose entry is gone (a crash between delete and cleanup).
    fn remove_orphans(&self) {
        let live: std::collections::HashSet<String> = self.history.entries.iter().map(|e| e.id.to_string()).collect();
        for sub in ["items", "thumbs"] {
            let Ok(list) = fs::read_dir(self.dir.join(sub)) else { continue };
            for entry in list.flatten() {
                let path = entry.path();
                let stem = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
                if !live.contains(&stem) {
                    let _ = fs::remove_file(path);
                }
            }
        }
    }

    /// Bytes used on disk by blobs and thumbnails.
    pub fn disk_usage(&self) -> u64 {
        ["items", "thumbs"]
            .iter()
            .filter_map(|sub| fs::read_dir(self.dir.join(sub)).ok())
            .flat_map(|list| list.flatten())
            .filter_map(|e| e.metadata().ok())
            .map(|m| m.len())
            .sum()
    }
}

/// Clipboard history is private: owner-only permissions on the folder.
fn restrict(dir: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(dir, fs::Permissions::from_mode(0o700));
    }
    #[cfg(not(unix))]
    let _ = dir;
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use kudzu_core::clipboard::{classify, encode_items, uti};

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("kudzu-clip-{name}-{}", Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn add(store: &mut ClipStore, text: &str) -> ClipEntry {
        let items = vec![vec![(uti::PLAIN_TEXT.to_string(), text.as_bytes().to_vec())]];
        let c = classify(&items, None).unwrap();
        let e = store.history.ingest(&c, None, None, Utc::now()).entry().clone();
        fs::write(store.blob_path(e.id), encode_items(&items)).unwrap();
        store.mark_dirty();
        e
    }

    #[test]
    fn persists_and_restores_through_trash() {
        let dir = temp("persist");
        let mut store = ClipStore::load(dir.clone());
        let a = add(&mut store, "alpha");
        let b = add(&mut store, "beta");
        store.save_if_dirty();

        let mut again = ClipStore::load(dir.clone());
        assert_eq!(again.history.entries.len(), 2);
        assert_eq!(kudzu_core::clipboard::text_of(&again.items(a.id).unwrap()).as_deref(), Some("alpha"));

        let gone = again.history.remove(&[b.id]);
        again.trash(&gone);
        assert!(!again.blob_path(b.id).exists());
        again.untrash(&gone);
        assert!(again.blob_path(b.id).exists());

        // Trash is emptied and orphans removed on the next launch.
        again.trash(&gone);
        again.save_if_dirty();
        fs::write(again.blob_path(Uuid::new_v4()), b"orphan").unwrap();
        let third = ClipStore::load(dir.clone());
        assert_eq!(fs::read_dir(dir.join("trash")).unwrap().count(), 0);
        assert_eq!(fs::read_dir(dir.join("items")).unwrap().count(), 1, "orphan removed, trashed blob gone");
        assert!(third.disk_usage() > 0);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn corrupt_index_is_moved_aside() {
        let dir = temp("corrupt");
        fs::write(dir.join("history.json"), b"{nope").unwrap();
        let store = ClipStore::load(dir.clone());
        assert!(store.history.entries.is_empty());
        assert!(fs::read_dir(&dir).unwrap().flatten().any(|e| e.file_name().to_string_lossy().starts_with("history-corrupt-")));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(fs::metadata(&dir).unwrap().permissions().mode() & 0o777, 0o700);
        }
        let _ = fs::remove_dir_all(dir);
    }
}
