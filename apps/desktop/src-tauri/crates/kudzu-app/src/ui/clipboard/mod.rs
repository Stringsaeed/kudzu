//! Clipboard history (`components/app/clipboard/`, `lib/clipboard.ts`): the main window's
//! Clipboard pages and the picker window.
//!
//! [`ClipData`] is `useClipboard`, the store each window keeps of the history: loaded once from
//! the backend, kept fresh by [`ClipEvent`]s, and changed optimistically (delete, restore, pin,
//! secret, clear) before the backend confirms. The pure helpers (search, rows, titles, times)
//! live in `kudzu_core::client::clipboard`.

pub mod color;
pub mod list;
pub mod media;
pub mod picker;
pub mod preview;
pub mod secret;
pub mod tile;
pub mod view;

use std::collections::HashMap;

use kudzu_core::client::clipboard::{self as clip, ClipFilter as CoreFilter};
use kudzu_core::clipboard::{ClipEntry, ClipKind};
use uuid::Uuid;

use crate::backend::clipboard::{self as backend, ClipEvent};
use crate::ui::main::{ClipFilter, MainUi};

/// The history as a window mirrors it (`useClipboard` in lib/clipboard.ts).
#[derive(Default)]
pub struct ClipData {
    /// The first load has landed.
    pub ready: bool,
    /// Newest copy first.
    pub entries: Vec<ClipEntry>,
    /// Bytes on disk, from the last load.
    pub disk_usage: u64,
    pub can_paste: bool,
    /// Bumps on every change, so caches keyed by it rebuild.
    pub rev: u64,
}

impl ClipData {
    fn changed(&mut self) {
        self.rev = self.rev.wrapping_add(1);
    }

    /// `refresh`: what `clipboard_list` returned.
    pub fn set(&mut self, info: backend::ClipboardInfo) {
        self.ready = true;
        self.entries = info.entries;
        clip::sort_newest(&mut self.entries);
        self.disk_usage = info.disk_usage;
        self.can_paste = info.can_paste;
        self.changed();
    }

    /// The store's event handler: upsert keeps newest-first order, remove drops, reset replaces.
    pub fn apply(&mut self, ev: &ClipEvent) {
        match ev {
            ClipEvent::Upsert { entry } => clip::upsert(&mut self.entries, (**entry).clone()),
            ClipEvent::Remove { ids } => self.entries.retain(|e| !ids.contains(&e.id)),
            ClipEvent::Reset { entries } => {
                self.entries = entries.clone();
                clip::sort_newest(&mut self.entries);
            }
        }
        self.changed();
    }

    pub fn get(&self, id: Uuid) -> Option<&ClipEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    /// `remove`: drops them at once, then deletes them; returns what was removed (for Undo).
    pub fn remove(&mut self, ids: &[Uuid]) -> Vec<ClipEntry> {
        self.entries.retain(|e| !ids.contains(&e.id));
        self.changed();
        backend::clipboard_delete(ids.to_vec())
    }

    /// `restore` (Undo of a delete).
    pub fn restore(&mut self, entries: Vec<ClipEntry>) {
        for e in &entries {
            clip::upsert(&mut self.entries, e.clone());
        }
        self.changed();
        backend::clipboard_restore(entries);
    }

    pub fn set_pinned(&mut self, id: Uuid, pinned: bool) {
        if let Some(e) = self.entries.iter_mut().find(|e| e.id == id) {
            e.pinned = pinned;
        }
        self.changed();
        backend::clipboard_pin(id, pinned);
    }

    /// `false` = "not a secret", `true` = "hide it", `None` = back to detection.
    pub fn set_secret(&mut self, id: Uuid, secret: Option<bool>) {
        if let Some(e) = self.entries.iter_mut().find(|e| e.id == id) {
            e.secret_override = secret;
        }
        self.changed();
        backend::clipboard_set_secret(id, secret);
    }

    /// `clear`: keeps pinned items unless asked; returns how many went.
    pub fn clear(&mut self, include_pinned: bool) -> usize {
        if include_pinned {
            self.entries.clear();
        } else {
            self.entries.retain(|e| e.pinned);
        }
        self.changed();
        backend::clipboard_clear(include_pinned)
    }
}

/// A window that shows clipboard history (the main window and the picker): the pieces shared
/// components read (`useClipboard`, the settings they need) and the secret correction, which
/// toasts where the window has toasts.
pub trait ClipHost: Sized + 'static {
    /// Settings ▸ Clipboard ▸ Show secrets.
    fn show_secrets(&self) -> bool;
    /// Settings ▸ Clipboard ▸ Keep history, in days (0 = forever).
    fn retention_days(&self) -> u32;
    /// `setSecretWithFeedback`.
    fn set_secret_with_feedback(&mut self, id: Uuid, secret: bool);
    /// Runs `f` on this window after `ms` (a `setTimeout`).
    fn later(&self, ms: u64, f: Later<Self>);
}

/// Work for [`ClipHost::later`].
pub type Later<S> = Box<dyn FnOnce(&mut S, &mut rune::Tx) + Send>;

impl ClipHost for MainUi {
    fn show_secrets(&self) -> bool {
        self.settings.clipboard_show_secrets
    }
    fn retention_days(&self) -> u32 {
        self.settings.clipboard_retention_days
    }
    fn set_secret_with_feedback(&mut self, id: Uuid, secret: bool) {
        secret::set_with_feedback(self, id, secret);
    }
    fn later(&self, ms: u64, f: Later<Self>) {
        self.spawn(move || std::thread::sleep(std::time::Duration::from_millis(ms)), move |s, (), tx| f(s, tx));
    }
}

/// Matching Kudzu's own detection clears the correction instead of storing a redundant one.
pub fn secret_correction(e: &ClipEntry, secret: bool) -> Option<bool> {
    let detected = e.secret.is_some();
    if secret == detected { None } else { Some(secret) }
}

/// The sidebar folder as the shared client filter.
pub fn core_filter(f: ClipFilter) -> CoreFilter {
    match f {
        ClipFilter::All => CoreFilter::All,
        ClipFilter::Pinned => CoreFilter::Pinned,
        ClipFilter::Text => CoreFilter::Kind(ClipKind::Text),
        ClipFilter::Image => CoreFilter::Kind(ClipKind::Image),
        ClipFilter::Link => CoreFilter::Kind(ClipKind::Link),
        ClipFilter::File => CoreFilter::Kind(ClipKind::File),
        ClipFilter::Code => CoreFilter::Kind(ClipKind::Code),
        ClipFilter::Color => CoreFilter::Kind(ClipKind::Color),
        ClipFilter::Other => CoreFilter::Kind(ClipKind::Other),
    }
}

/// `FILTERS[..].label` for a folder.
pub fn filter_label(f: ClipFilter) -> &'static str {
    let core = core_filter(f);
    clip::FILTERS.iter().find(|o| o.value == core).map_or("Other", |o| o.label)
}

/// Loads the history when the window starts (`refresh`, off the UI thread).
pub fn load(s: &mut MainUi) {
    if let Some(me) = s.me.clone() {
        media::on_loaded(move || me.update(|_, _| {}));
    }
    s.spawn(backend::clipboard_list, |s, info, _| s.clip.set(info));
}

/// Applies a history change from the backend.
pub fn apply(s: &mut MainUi, ev: &ClipEvent) {
    s.clip.apply(ev);
}

/// The sidebar's Clipboard section: how many items each folder holds (History is `All`).
/// Owned by the clipboard port; the sidebar only reads it.
pub fn folder_counts(s: &MainUi) -> HashMap<ClipFilter, usize> {
    let folders = [
        ClipFilter::All,
        ClipFilter::Pinned,
        ClipFilter::Text,
        ClipFilter::Image,
        ClipFilter::Link,
        ClipFilter::File,
        ClipFilter::Code,
        ClipFilter::Color,
        ClipFilter::Other,
    ];
    folders.into_iter().map(|f| (f, s.clip.entries.iter().filter(|e| clip::in_filter(e, core_filter(f))).count())).collect()
}

/// Now, in ms since the epoch, for the time helpers.
pub fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

/// The full text of an entry: the excerpt when it is whole, else the blob's text, loaded once per
/// id and content on a worker (`loadFullText`). `None` while it loads.
pub fn full_text(e: &ClipEntry) -> Option<std::sync::Arc<String>> {
    media::full_text(e)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};

    fn entry(n: u128, kind: ClipKind, minutes_ago: i64, pinned: bool) -> ClipEntry {
        let at = Utc.timestamp_opt(1_800_000_000 - minutes_ago * 60, 0).unwrap();
        ClipEntry {
            id: Uuid::from_u128(n),
            kind,
            text: Some(format!("item {n}")),
            text_len: 6,
            line_count: 1,
            files: vec![],
            image: None,
            color: None,
            rich: false,
            types: vec![],
            size: 6,
            source: None,
            hash: format!("h{n}"),
            first_copied_at: at,
            copied_at: at,
            copy_count: 1,
            timings: vec![at],
            last_used_at: None,
            use_count: 0,
            pinned,
            secret: None,
            secret_override: None,
        }
    }

    #[test]
    fn events_keep_newest_first_like_the_store() {
        let mut d = ClipData::default();
        d.apply(&ClipEvent::Reset { entries: vec![entry(1, ClipKind::Text, 10, false), entry(2, ClipKind::Link, 1, false)] });
        assert_eq!(d.entries.iter().map(|e| e.id.as_u128()).collect::<Vec<_>>(), [2, 1]);
        d.apply(&ClipEvent::Upsert { entry: Box::new(entry(3, ClipKind::Code, 5, false)) });
        assert_eq!(d.entries.iter().map(|e| e.id.as_u128()).collect::<Vec<_>>(), [2, 3, 1]);
        d.apply(&ClipEvent::Remove { ids: vec![Uuid::from_u128(2)] });
        assert_eq!(d.entries.len(), 2);
        let rev = d.rev;
        d.apply(&ClipEvent::Upsert { entry: Box::new(entry(1, ClipKind::Text, 0, true)) });
        assert_eq!(d.entries[0].id.as_u128(), 1);
        assert!(d.rev > rev);
    }

    #[test]
    fn folders_map_to_the_web_filters() {
        assert_eq!(filter_label(ClipFilter::Image), "Images");
        assert_eq!(filter_label(ClipFilter::Pinned), "Pinned");
        let text = entry(1, ClipKind::Other, 0, true);
        assert!(clip::in_filter(&text, core_filter(ClipFilter::Text)));
        assert!(clip::in_filter(&text, core_filter(ClipFilter::Pinned)));
    }
}
