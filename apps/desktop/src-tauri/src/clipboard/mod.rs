//! Clipboard history: a watcher that records copies, commands for the UI, and the `kzclip://`
//! protocol that serves thumbnails and images to the webviews without going through IPC.
//!
//! The pasteboard has no change notification, so (like every clipboard manager) a background
//! thread polls its change count — one integer read every [`POLL`]. Reading and storing a copy
//! only happens when the count moves, off the UI thread.

mod protocol;
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
use tauri::{AppHandle, Emitter, Manager, State};
use uuid::Uuid;

pub use protocol::handle as protocol_handler;
pub use store::ClipStore;

use crate::state::{core, AppCore};
use crate::windows;

pub const EV_CLIPBOARD: &str = "kudzu://clipboard";

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

type Core<'a> = State<'a, Arc<AppCore>>;
type Res<T> = Result<T, String>;

/// What changed in the history, for every window.
#[derive(Serialize, Clone)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ClipEvent {
    /// Added or changed; newest copies go to the top.
    Upsert { entry: Box<ClipEntry> },
    Remove { ids: Vec<Uuid> },
    /// Many changes at once (clear, restore, retention).
    Reset { entries: Vec<ClipEntry> },
}

fn emit(app: &AppHandle, event: ClipEvent) {
    let _ = app.emit(EV_CLIPBOARD, event);
}

fn emit_reset(core: &AppCore) {
    let entries = core.clip.lock().history.entries.clone();
    emit(&core.app, ClipEvent::Reset { entries });
}

/// Starts the watcher and the retention timer.
pub fn start(app: &AppHandle) {
    let app = app.clone();
    let _ = thread::Builder::new().name("kudzu-clipboard".into()).spawn(move || {
        let core = core(&app);
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
pub fn flush(app: &AppHandle) {
    if let Some(core) = app.try_state::<Arc<AppCore>>() {
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
    emit(&core.app, ClipEvent::Upsert { entry: Box::new(ingest.entry().clone()) });
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
        emit(&core.app, ClipEvent::Remove { ids: removed.iter().map(|e| e.id).collect() });
    }
}

/// Items to put back on the clipboard for an entry (plain text only when `plain`).
fn items_for(core: &AppCore, id: Uuid, plain: bool) -> Res<Vec<PasteItem>> {
    let clip = core.clip.lock();
    let entry = clip.history.get(id).ok_or("That item is no longer in your clipboard history.")?;
    let stored = clip.items(id);
    let full_text = stored.as_deref().and_then(model::text_of).or_else(|| (entry.text_len <= model::TEXT_EXCERPT_LIMIT).then(|| entry.text.clone()).flatten());
    if plain {
        if let Some(text) = full_text {
            return Ok(vec![vec![(uti::PLAIN_TEXT.to_string(), text.into_bytes())]]);
        }
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
        emit(&core.app, ClipEvent::Upsert { entry: Box::new(entry) });
    }
    Ok(())
}

// MARK: Commands

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipboardInfo {
    entries: Vec<ClipEntry>,
    /// Bytes on disk.
    disk_usage: u64,
    /// Whether ⌘V into other apps is possible (Accessibility granted, no Secure Input).
    can_paste: bool,
}

#[tauri::command]
pub async fn clipboard_list(core: Core<'_>) -> Res<ClipboardInfo> {
    let core = core.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let clip = core.clip.lock();
        ClipboardInfo { entries: clip.history.entries.clone(), disk_usage: clip.disk_usage(), can_paste: board::can_paste() }
    })
    .await
    .map_err(|e| e.to_string())
}

/// The full text of an entry (list payloads carry an excerpt).
#[tauri::command]
pub async fn clipboard_text(core: Core<'_>, id: Uuid) -> Res<Option<String>> {
    let core = core.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let clip = core.clip.lock();
        let entry = clip.history.get(id).cloned();
        clip.items(id).as_deref().and_then(model::text_of).or_else(|| entry.and_then(|e| e.text))
    })
    .await
    .map_err(|e| e.to_string())
}

/// Puts an entry back on the clipboard.
#[tauri::command]
pub fn clipboard_copy(core: Core, id: Uuid, plain: bool) -> Res<()> {
    put_back(&core, id, plain)
}

/// From the picker: puts an entry on the clipboard and pastes it into the app the user came from.
/// Returns `"pasted"`, or `"copied"` when pasting isn't allowed (the picker stays open to say so).
#[tauri::command]
pub async fn clipboard_paste(app: AppHandle, core: Core<'_>, id: Uuid, plain: bool) -> Res<&'static str> {
    let core = core.inner().clone();
    put_back(&core, id, plain)?;
    if !board::can_paste() {
        return Ok("copied");
    }
    let target = core.clip_target.lock().clone();
    let (tx, rx) = tokio::sync::oneshot::channel();
    let a = app.clone();
    let _ = app.run_on_main_thread(move || {
        windows::hide_clipboard(&a, false);
        let _ = tx.send(());
    });
    let _ = rx.await;
    let result = tauri::async_runtime::spawn_blocking(move || board::paste(target.as_ref())).await.map_err(|e| e.to_string())?;
    match result {
        Ok(()) => Ok("pasted"),
        Err(PasteError::TargetGone) => Err("The app you came from isn't available any more. The item is on your clipboard.".into()),
        Err(PasteError::NotAllowed) => Ok("copied"),
    }
}

#[tauri::command]
pub fn clipboard_delete(core: Core, ids: Vec<Uuid>) -> Vec<ClipEntry> {
    let removed = {
        let mut clip = core.clip.lock();
        let removed = clip.history.remove(&ids);
        clip.trash(&removed);
        clip.mark_dirty();
        removed
    };
    emit(&core.app, ClipEvent::Remove { ids: removed.iter().map(|e| e.id).collect() });
    removed
}

/// Undo for delete (while the files are still in the trash).
#[tauri::command]
pub fn clipboard_restore(core: Core, entries: Vec<ClipEntry>) {
    {
        let mut clip = core.clip.lock();
        clip.untrash(&entries);
        clip.history.restore(entries);
        clip.mark_dirty();
    }
    emit_reset(&core);
}

#[tauri::command]
pub fn clipboard_pin(core: Core, id: Uuid, pinned: bool) -> Option<ClipEntry> {
    let entry = {
        let mut clip = core.clip.lock();
        let e = clip.history.set_pinned(id, pinned);
        clip.mark_dirty();
        e
    };
    if let Some(e) = &entry {
        emit(&core.app, ClipEvent::Upsert { entry: Box::new(e.clone()) });
    }
    entry
}

/// Corrects secret detection for an entry: `Some(false)` shows it (to agents too), `Some(true)`
/// hides it, `None` goes back to what Kudzu detected.
#[tauri::command]
pub fn clipboard_set_secret(core: Core, id: Uuid, secret: Option<bool>) -> Option<ClipEntry> {
    let entry = {
        let mut clip = core.clip.lock();
        let e = clip.history.set_secret(id, secret);
        clip.mark_dirty();
        e
    };
    if let Some(e) = &entry {
        emit(&core.app, ClipEvent::Upsert { entry: Box::new(e.clone()) });
    }
    entry
}

/// Deletes the whole history (pinned items too when asked). Not undoable.
#[tauri::command]
pub fn clipboard_clear(core: Core, include_pinned: bool) -> usize {
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
#[tauri::command]
pub fn clipboard_open(app: AppHandle, core: Core, id: Uuid, index: Option<usize>) -> Res<()> {
    use tauri_plugin_opener::OpenerExt;
    let entry = core.clip.lock().history.get(id).cloned().ok_or("Not found")?;
    match entry.kind {
        ClipKind::Link => {
            let text = entry.text.unwrap_or_default();
            let url = text.trim();
            let url = if url.to_ascii_lowercase().starts_with("www.") { format!("https://{url}") } else { url.to_string() };
            app.opener().open_url(url, None::<&str>).map_err(|e| e.to_string())
        }
        ClipKind::File => {
            let path = entry.files.get(index.unwrap_or(0)).ok_or("Not found")?;
            app.opener().open_path(path, None::<&str>).map_err(|e| e.to_string())
        }
        _ => Err("Nothing to open".into()),
    }
}

/// Shows a copied file in Finder.
#[tauri::command]
pub fn clipboard_reveal(app: AppHandle, core: Core, id: Uuid, index: Option<usize>) -> Res<()> {
    use tauri_plugin_opener::OpenerExt;
    let entry = core.clip.lock().history.get(id).cloned().ok_or("Not found")?;
    let path = entry.files.get(index.unwrap_or(0)).ok_or("Not found")?;
    app.opener().reveal_item_in_dir(path).map_err(|e| e.to_string())
}

/// Opens the main window with a new snippet made from an entry's text.
#[tauri::command]
pub fn clipboard_to_snippet(app: AppHandle, core: Core, id: Uuid) -> Res<()> {
    let text = {
        let clip = core.clip.lock();
        let entry = clip.history.get(id).cloned().ok_or("Not found")?;
        clip.items(id).as_deref().and_then(model::text_of).or(entry.text).ok_or("Only text can become a snippet.")?
    };
    windows::hide_clipboard(&app, false);
    windows::show_main(&app);
    let _ = app.emit(crate::state::EV_NEW_SNIPPET, serde_json::json!({ "content": text }));
    Ok(())
}

#[tauri::command]
pub fn show_clipboard(app: AppHandle) {
    windows::toggle_clipboard(&app);
}

#[tauri::command]
pub fn hide_clipboard(app: AppHandle) {
    windows::hide_clipboard(&app, true);
}

#[tauri::command]
pub fn set_clipboard_hotkey(app: AppHandle, core: Core, accelerator: String) -> Res<crate::settings::Settings> {
    crate::hotkey::register(&app, crate::hotkey::Slot::Clipboard, &accelerator)?;
    let settings = {
        let mut s = core.store.lock();
        s.settings.clipboard_hotkey = accelerator.clone();
        s.settings.clone()
    };
    core.settings_changed();
    crate::tray::set_clipboard_hint(&app, &accelerator);
    Ok(settings)
}

/// Registers the picker shortcut when history is on (and removes it when off).
pub fn apply_enabled(app: &AppHandle) {
    let (enabled, accel) = {
        let c = core(app);
        let s = c.store.lock();
        (s.settings.clipboard_enabled, s.settings.clipboard_hotkey.clone())
    };
    if enabled {
        if let Err(e) = crate::hotkey::register(app, crate::hotkey::Slot::Clipboard, &accel) {
            log::warn!("clipboard hotkey: {e}");
        }
    } else {
        crate::hotkey::unregister(app, crate::hotkey::Slot::Clipboard);
        windows::hide_clipboard(app, false);
    }
}
