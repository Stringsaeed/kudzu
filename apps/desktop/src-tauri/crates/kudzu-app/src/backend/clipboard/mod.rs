//! Clipboard history: a watcher that records copies and commands for the UI. Ported from the
//! Tauri shell's `clipboard/mod.rs`; logic unchanged. Rune windows read thumbnails and images
//! straight from the store's files, so the `kzclip://` protocol is gone.
//!
//! The pasteboard has no change notification, so (like every clipboard manager) a background
//! thread polls its change count — one integer read every [`POLL`]. Reading and storing a copy
//! only happens when the count moves, off the UI thread.

#[allow(clippy::collapsible_if)] // shared with the Tauri shell (edition 2021)
#[path = "../../../../../src/clipboard/store.rs"]
mod store;

use std::fs;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use chrono::Utc;
use kudzu_core::clipboard::{self as model, uti, ClipEntry, ClipKind, ClipSource, ImageSize, Ingest, PasteItem};
use kudzu_engine::clipboard::{self as board, PasteError};
use kudzu_engine::{system, AppInfo};
use serde::Serialize;
use uuid::Uuid;

pub use store::ClipStore;

use super::core::{core, AppCore, Event, NewSnippetDraft, Slot, TrayUpdate};
use super::{system as open, windows};

const POLL: Duration = Duration::from_millis(300);
/// Save a dirty index at most this often (in polls).
const SAVE_EVERY: u32 = 3;
/// Apply retention this often (in polls): every 10 minutes.
const PRUNE_EVERY: u32 = 2000;
/// Larger copies keep their metadata and text but no restorable blob.
const MAX_BLOB_BYTES: u64 = 64 * 1024 * 1024;
/// Longer side of stored thumbnails, in pixels (sharp at 2× in the list and picker).
const THUMB_PX: u32 = 480;
/// Image files bigger than this aren't read for a thumbnail.
const MAX_FILE_THUMB_BYTES: u64 = 40 * 1024 * 1024;

type Res<T> = Result<T, String>;

/// What changed in the history, for every window.
#[derive(Serialize, Clone, Debug)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ClipEvent {
    /// Added or changed; newest copies go to the top.
    Upsert { entry: Box<ClipEntry> },
    Remove { ids: Vec<Uuid> },
    /// Many changes at once (clear, restore, retention).
    Reset { entries: Vec<ClipEntry> },
}

fn emit(core: &AppCore, event: ClipEvent) {
    core.emit(Event::Clipboard(Arc::new(event)));
}

fn emit_reset(core: &AppCore) {
    let entries = core.clip.lock().history.entries.clone();
    emit(core, ClipEvent::Reset { entries });
}

/// Starts the watcher and the retention timer.
pub fn start() {
    let _ = thread::Builder::new().name("kudzu-clipboard".into()).spawn(move || {
        let core = core();
        prune(&core);
        // What's on the clipboard at launch was copied earlier; don't count it as a new copy.
        let mut last = board::change_count();
        let mut tick: u32 = 0;
        loop {
            thread::sleep(POLL);
            tick = tick.wrapping_add(1);
            if tick.is_multiple_of(SAVE_EVERY) {
                core.clip.lock().save_if_dirty();
            }
            if tick.is_multiple_of(PRUNE_EVERY) {
                prune(&core);
            }
            let count = board::change_count();
            if count == last {
                continue;
            }
            last = count;
            if board::is_own_change(count) {
                continue;
            }
            let (enabled, ignored) = {
                let s = core.store.lock();
                (s.settings.clipboard_enabled, s.settings.clipboard_ignored_apps.clone())
            };
            if !enabled {
                continue;
            }
            let front = system::frontmost_app();
            if front.as_ref().is_some_and(|a| ignored.iter().any(|b| b == &a.bundle_id)) {
                continue;
            }
            capture(&core, front);
        }
    });
}

/// Saves the index now (on quit).
pub fn flush() {
    if let Some(core) = super::core::try_core() {
        core.clip.lock().save_if_dirty();
    }
}

fn capture(core: &AppCore, front: Option<AppInfo>) {
    let items = board::read();
    if items.is_empty() || model::is_private(&items) {
        return;
    }
    let bundle = front.as_ref().map(|a| a.bundle_id.clone());
    let Some(c) = model::classify(&items, bundle.as_deref()) else { return };
    let source = front.map(|a| ClipSource { bundle_id: a.bundle_id, name: a.name, path: a.path });

    // Thumbnails: copied images, or a single copied image file.
    let thumb = match c.kind {
        ClipKind::Image => model::image_of(&items).and_then(|(data, _)| board::thumbnail(data, THUMB_PX)),
        ClipKind::File if c.files.len() == 1 && model::image_mime_for_path(&c.files[0]).is_some() => fs::metadata(&c.files[0])
            .ok()
            .filter(|m| m.is_file() && m.len() <= MAX_FILE_THUMB_BYTES)
            .and_then(|_| fs::read(&c.files[0]).ok())
            .and_then(|data| board::thumbnail(&data, THUMB_PX)),
        _ => None,
    };
    let image = thumb.as_ref().map(|t| ImageSize { width: t.width, height: t.height });

    let (ingest, blob, thumb_path) = {
        let mut clip = core.clip.lock();
        let ingest = clip.history.ingest(&c, source, image, Utc::now());
        clip.mark_dirty();
        let id = ingest.entry().id;
        (ingest, clip.blob_path(id), clip.thumb_path(id))
    };
    // Same image bytes as before: the blob and thumbnail on disk are already right.
    let unchanged = matches!(ingest, Ingest::Repeated(_)) && c.kind == ClipKind::Image && blob.exists();
    if !unchanged {
        if c.size <= MAX_BLOB_BYTES {
            if let Err(e) = crate::store::write_atomic(&blob, &model::encode_items(&items)) {
                log::warn!("clipboard blob: {e}");
            }
        } else {
            let _ = fs::remove_file(&blob);
        }
        if let Some(t) = &thumb {
            let _ = fs::write(&thumb_path, &t.png);
        }
    }
    emit(core, ClipEvent::Upsert { entry: Box::new(ingest.entry().clone()) });
}

/// Applies the retention window and size cap.
pub fn prune(core: &AppCore) {
    let days = core.store.lock().settings.clipboard_retention_days;
    let removed = {
        let mut clip = core.clip.lock();
        let removed = clip.history.prune(Utc::now(), days);
        if !removed.is_empty() {
            clip.purge(&removed);
            clip.mark_dirty();
        }
        removed
    };
    if !removed.is_empty() {
        log::info!("clipboard: {} entries expired", removed.len());
        emit(core, ClipEvent::Remove { ids: removed.iter().map(|e| e.id).collect() });
    }
}

/// Items to put back on the clipboard for an entry (plain text only when `plain`).
fn items_for(core: &AppCore, id: Uuid, plain: bool) -> Res<Vec<PasteItem>> {
    let clip = core.clip.lock();
    let entry = clip.history.get(id).ok_or("That item is no longer in your clipboard history.")?;
    let stored = clip.items(id);
    let full_text = stored.as_deref().and_then(model::text_of).or_else(|| (entry.text_len <= model::TEXT_EXCERPT_LIMIT).then(|| entry.text.clone()).flatten());
    if plain
        && let Some(text) = full_text {
            return Ok(vec![vec![(uti::PLAIN_TEXT.to_string(), text.into_bytes())]]);
        }
    stored
        .or_else(|| full_text.map(|t| vec![vec![(uti::PLAIN_TEXT.to_string(), t.into_bytes())]]))
        .ok_or_else(|| "This item was too large to keep, so it can't be copied again.".to_string())
}

fn put_back(core: &AppCore, id: Uuid, plain: bool) -> Res<()> {
    let items = items_for(core, id, plain)?;
    board::write(&items);
    let entry = {
        let mut clip = core.clip.lock();
        let e = clip.history.mark_used(id, Utc::now());
        clip.mark_dirty();
        e
    };
    if let Some(entry) = entry {
        emit(core, ClipEvent::Upsert { entry: Box::new(entry) });
    }
    Ok(())
}

// MARK: Commands

#[derive(Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct ClipboardInfo {
    pub entries: Vec<ClipEntry>,
    /// Bytes on disk.
    pub disk_usage: u64,
    /// Whether ⌘V into other apps is possible (Accessibility granted, no Secure Input).
    pub can_paste: bool,
}

/// Blocking (sizes the store on disk): call it off the UI thread.
pub fn clipboard_list() -> ClipboardInfo {
    let core = core();
    let clip = core.clip.lock();
    ClipboardInfo { entries: clip.history.entries.clone(), disk_usage: clip.disk_usage(), can_paste: board::can_paste() }
}

/// The full text of an entry (list payloads carry an excerpt).
/// Blocking (reads the item from disk): call it off the UI thread.
pub fn clipboard_text(id: Uuid) -> Option<String> {
    let core = core();
    let clip = core.clip.lock();
    let entry = clip.history.get(id).cloned();
    clip.items(id).as_deref().and_then(model::text_of).or_else(|| entry.and_then(|e| e.text))
}

/// Puts an entry back on the clipboard.
pub fn clipboard_copy(id: Uuid, plain: bool) -> Res<()> {
    put_back(&core(), id, plain)
}

/// From the picker: puts an entry on the clipboard and pastes it into the app the user came from.
/// Returns `"pasted"`, or `"copied"` when pasting isn't allowed (the picker stays open to say so).
/// Blocking (waits for the paste): call it off the UI thread.
pub fn clipboard_paste(id: Uuid, plain: bool) -> Res<&'static str> {
    let core = core();
    put_back(&core, id, plain)?;
    if !board::can_paste() {
        return Ok("copied");
    }
    let target = core.clip_target.lock().clone();
    windows::hide_clipboard(false);
    let result = board::paste(target.as_ref());
    match result {
        Ok(()) => Ok("pasted"),
        Err(PasteError::TargetGone) => Err("The app you came from isn't available any more. The item is on your clipboard.".into()),
        Err(PasteError::NotAllowed) => Ok("copied"),
    }
}

pub fn clipboard_delete(ids: Vec<Uuid>) -> Vec<ClipEntry> {
    let core = core();
    let removed = {
        let mut clip = core.clip.lock();
        let removed = clip.history.remove(&ids);
        clip.trash(&removed);
        clip.mark_dirty();
        removed
    };
    emit(&core, ClipEvent::Remove { ids: removed.iter().map(|e| e.id).collect() });
    removed
}

/// Undo for delete (while the files are still in the trash).
pub fn clipboard_restore(entries: Vec<ClipEntry>) {
    let core = core();
    {
        let mut clip = core.clip.lock();
        clip.untrash(&entries);
        clip.history.restore(entries);
        clip.mark_dirty();
    }
    emit_reset(&core);
}

pub fn clipboard_pin(id: Uuid, pinned: bool) -> Option<ClipEntry> {
    let core = core();
    let entry = {
        let mut clip = core.clip.lock();
        let e = clip.history.set_pinned(id, pinned);
        clip.mark_dirty();
        e
    };
    if let Some(e) = &entry {
        emit(&core, ClipEvent::Upsert { entry: Box::new(e.clone()) });
    }
    entry
}

/// Corrects secret detection for an entry: `Some(false)` shows it (to agents too), `Some(true)`
/// hides it, `None` goes back to what Kudzu detected.
pub fn clipboard_set_secret(id: Uuid, secret: Option<bool>) -> Option<ClipEntry> {
    let core = core();
    let entry = {
        let mut clip = core.clip.lock();
        let e = clip.history.set_secret(id, secret);
        clip.mark_dirty();
        e
    };
    if let Some(e) = &entry {
        emit(&core, ClipEvent::Upsert { entry: Box::new(e.clone()) });
    }
    entry
}

/// Deletes the whole history (pinned items too when asked). Not undoable.
pub fn clipboard_clear(include_pinned: bool) -> usize {
    let core = core();
    let n = {
        let mut clip = core.clip.lock();
        let removed = clip.history.clear(include_pinned);
        clip.purge(&removed);
        clip.mark_dirty();
        clip.save_if_dirty();
        removed.len()
    };
    emit_reset(&core);
    n
}

/// Opens a link entry in the browser, or a file entry (by index) in its default app.
pub fn clipboard_open(id: Uuid, index: Option<usize>) -> Res<()> {
    let core = core();
    let entry = core.clip.lock().history.get(id).cloned().ok_or("Not found")?;
    match entry.kind {
        ClipKind::Link => {
            let text = entry.text.unwrap_or_default();
            let url = text.trim();
            let url = if url.to_ascii_lowercase().starts_with("www.") { format!("https://{url}") } else { url.to_string() };
            open::open_url(&url)
        }
        ClipKind::File => {
            let path = entry.files.get(index.unwrap_or(0)).ok_or("Not found")?;
            open::open_path(path)
        }
        _ => Err("Nothing to open".into()),
    }
}

/// Shows a copied file in Finder.
pub fn clipboard_reveal(id: Uuid, index: Option<usize>) -> Res<()> {
    let core = core();
    let entry = core.clip.lock().history.get(id).cloned().ok_or("Not found")?;
    let path = entry.files.get(index.unwrap_or(0)).ok_or("Not found")?;
    open::reveal(path)
}

/// Opens the main window with a new snippet made from an entry's text.
pub fn clipboard_to_snippet(id: Uuid) -> Res<()> {
    let core = core();
    let text = {
        let clip = core.clip.lock();
        let entry = clip.history.get(id).cloned().ok_or("Not found")?;
        clip.items(id).as_deref().and_then(model::text_of).or(entry.text).ok_or("Only text can become a snippet.")?
    };
    windows::hide_clipboard(false);
    windows::show_main();
    core.emit(Event::NewSnippet(NewSnippetDraft { content: Some(text), ..Default::default() }));
    Ok(())
}

pub fn hide_clipboard() {
    windows::hide_clipboard(true);
}

pub fn set_clipboard_hotkey(accelerator: String) -> Res<crate::settings::Settings> {
    let core = core();
    super::hotkey::register(Slot::Clipboard, &accelerator)?;
    let settings = {
        let mut s = core.store.lock();
        s.settings.clipboard_hotkey = accelerator.clone();
        s.settings.clone()
    };
    core.settings_changed();
    core.shell().tray(TrayUpdate::ClipboardHint(accelerator.clone()));
    Ok(settings)
}

/// Registers the picker shortcut when history is on (and removes it when off).
pub fn apply_enabled() {
    let (enabled, accel) = {
        let c = core();
        let s = c.store.lock();
        (s.settings.clipboard_enabled, s.settings.clipboard_hotkey.clone())
    };
    if enabled {
        if let Err(e) = super::hotkey::register(Slot::Clipboard, &accel) {
            log::warn!("clipboard hotkey: {e}");
        }
    } else {
        super::hotkey::unregister(Slot::Clipboard);
        windows::hide_clipboard(false);
    }
}
