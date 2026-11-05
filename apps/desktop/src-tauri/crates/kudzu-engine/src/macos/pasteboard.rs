//! General pasteboard access with full save/restore, and paste-based insertion.

use std::sync::Mutex;
use std::thread;
use std::time::{Duration, Instant};

use objc2::rc::{autoreleasepool, Retained};
use objc2::runtime::ProtocolObject;
use objc2_app_kit::{NSPasteboard, NSPasteboardItem, NSPasteboardTypeString, NSPasteboardWriting};
use objc2_foundation::{NSArray, NSData, NSString};

use super::keys::{self, KeyError};
use crate::lock;

/// There's no signal that the target consumed ⌘V; Electron apps can take well over 250 ms.
const RESTORE_DELAY: Duration = Duration::from_millis(400);

/// Every item and type on the pasteboard, as raw data.
pub(crate) struct Snapshot {
    items: Vec<Vec<(String, Vec<u8>)>>,
}

/// A paste whose old clipboard still has to come back: (snapshot, change count after our write).
static PENDING_RESTORE: Mutex<Option<(Snapshot, isize)>> = Mutex::new(None);

/// Change counts produced by Kudzu's own writes (restores, copies), newest last. The clipboard
/// history skips these so putting the user's clipboard back never looks like a new copy.
static OWN_CHANGES: Mutex<[isize; 8]> = Mutex::new([-1; 8]);

fn note_own_change() {
    let count = change_count();
    let mut own = lock(&OWN_CHANGES);
    own.rotate_left(1);
    own[7] = count;
}

/// Whether `count` came from one of Kudzu's own writes.
pub(crate) fn is_own_change(count: isize) -> bool {
    lock(&OWN_CHANGES).contains(&count)
}

fn general() -> Retained<NSPasteboard> {
    NSPasteboard::generalPasteboard()
}

/// Current change count of the general pasteboard.
pub(crate) fn change_count() -> isize {
    autoreleasepool(|_| general().changeCount())
}

/// Saves every item; `None` if some promised data can't be read (restoring would lose it).
pub(crate) fn snapshot() -> Option<Snapshot> {
    autoreleasepool(|_| {
        let board = general();
        let mut items = Vec::new();
        if let Some(list) = board.pasteboardItems() {
            for item in list.iter() {
                let mut entries = Vec::new();
                for kind in item.types().iter() {
                    let data = item.dataForType(&kind)?;
                    entries.push((kind.to_string(), data.to_vec()));
                }
                items.push(entries);
            }
        }
        Some(Snapshot { items })
    })
}

/// Replaces the pasteboard with `snapshot`.
pub(crate) fn restore(snapshot: &Snapshot) {
    write_raw(&snapshot.items);
    note_own_change();
}

/// Every item and type, skipping types whose (promised) data can't be read.
pub(crate) fn read_items() -> Vec<Vec<(String, Vec<u8>)>> {
    autoreleasepool(|_| {
        let board = general();
        let mut items = Vec::new();
        if let Some(list) = board.pasteboardItems() {
            for item in list.iter() {
                let entries: Vec<(String, Vec<u8>)> =
                    item.types().iter().filter_map(|kind| item.dataForType(&kind).map(|data| (kind.to_string(), data.to_vec()))).collect();
                if !entries.is_empty() {
                    items.push(entries);
                }
            }
        }
        items
    })
}

/// Replaces the pasteboard with `items` (as Kudzu's own change). Returns the new change count.
pub(crate) fn write_items(items: &[Vec<(String, Vec<u8>)>]) -> isize {
    write_raw(items);
    note_own_change();
    change_count()
}

fn write_raw(items: &[Vec<(String, Vec<u8>)>]) {
    let snapshot = items;
    autoreleasepool(|_| {
        let board = general();
        board.clearContents();
        if snapshot.is_empty() {
            return;
        }
        let items: Vec<Retained<ProtocolObject<dyn NSPasteboardWriting>>> = snapshot
            .iter()
            .map(|entries| {
                let item = NSPasteboardItem::new();
                for (kind, data) in entries {
                    item.setData_forType(&NSData::with_bytes(data), &NSString::from_str(kind));
                }
                ProtocolObject::from_retained(item)
            })
            .collect();
        board.writeObjects(&NSArray::from_retained_slice(&items));
    })
}

/// Plain-text contents.
pub(crate) fn read_string() -> Option<String> {
    autoreleasepool(|_| general().stringForType(unsafe { NSPasteboardTypeString }).map(|s| s.to_string()))
}

/// Writes plain text as the only item.
pub(crate) fn write_string(text: &str) -> bool {
    let ok = autoreleasepool(|_| {
        let board = general();
        board.clearContents();
        board.setString_forType(&NSString::from_str(text), unsafe { NSPasteboardTypeString })
    });
    note_own_change();
    ok
}

/// Writes `text` marked transient and concealed so clipboard managers skip it.
fn write_transient(text: &str, html: Option<&str>) -> bool {
    autoreleasepool(|_| {
        let item = NSPasteboardItem::new();
        let empty = NSData::new();
        let ok = item.setString_forType(&NSString::from_str(text), unsafe { NSPasteboardTypeString })
            && item.setData_forType(&empty, &NSString::from_str("org.nspasteboard.TransientType"))
            && item.setData_forType(&empty, &NSString::from_str("org.nspasteboard.ConcealedType"));
        if !ok || html.is_some_and(|html| !item.setString_forType(&NSString::from_str(html), &NSString::from_str("public.html"))) {
            return false;
        }
        let board = general();
        board.clearContents();
        let objects = NSArray::from_retained_slice(&[ProtocolObject::from_retained(item)]);
        board.writeObjects(&objects)
    })
}

/// Why a paste failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PasteError {
    Unavailable,
    Key(KeyError),
}

/// Pastes `text` via ⌘V, then restores the previous clipboard after a delay (blocking).
pub(crate) fn paste_text(text: &str, html: Option<&str>) -> Result<(), PasteError> {
    // A previous paste still pending restoration owns the "real" clipboard; put it back first.
    restore_pending();
    let saved = snapshot().ok_or(PasteError::Unavailable)?;
    if !write_transient(text, html) {
        restore(&saved);
        return Err(PasteError::Unavailable);
    }
    *lock(&PENDING_RESTORE) = Some((saved, change_count()));
    if let Err(error) = keys::paste() {
        restore_pending();
        return Err(PasteError::Key(error));
    }
    thread::sleep(RESTORE_DELAY);
    restore_pending();
    Ok(())
}

/// Restores the clipboard a paste replaced, unless the user copied something since.
pub(crate) fn restore_pending() {
    let Some((saved, count)) = lock(&PENDING_RESTORE).take() else { return };
    if change_count() == count {
        restore(&saved);
    }
}

/// Copies the frontmost app's selection with ⌘C, waiting up to `timeout`; restores the clipboard.
pub(crate) fn copy_selection(timeout: Duration) -> Option<String> {
    restore_pending();
    let saved = snapshot()?;
    let original = change_count();
    keys::copy().ok()?;
    let deadline = Instant::now() + timeout;
    let mut text = None;
    while Instant::now() < deadline {
        thread::sleep(Duration::from_millis(10));
        if change_count() != original {
            text = read_string().filter(|s| !s.is_empty());
            break;
        }
    }
    if change_count() != original {
        restore(&saved);
    }
    text
}
