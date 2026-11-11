//! General pasteboard access with full save and restore, and paste-based insertion.
//!
//! [`Pasteboard`] owns the two records every pasteboard write in the process must share: the
//! paste whose old clipboard still has to come back, and the change counts of Kudzu's own
//! writes. Snippet pastes, restores, clipboard-history writes and selection copies all go
//! through it. The system pasteboard is one per process, so [`Pasteboard::shared`] is the one
//! process-wide instance; [`Pasteboard::restore_pending`] is its shutdown. The raw board is a
//! seam ([`Board`]) so the protocol tests run on a fake.

use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

use objc2::rc::{autoreleasepool, Retained};
use objc2::runtime::ProtocolObject;
use objc2_app_kit::{NSPasteboard, NSPasteboardItem, NSPasteboardTypeString, NSPasteboardWriting};
use objc2_foundation::{NSArray, NSData, NSString};

use super::events::Events;
use super::keys::{self, KeyError};
use super::observe::Clock;
use crate::lock;

/// There's no signal that the target consumed ⌘V; Electron apps can take well over 250 ms.
pub(crate) const RESTORE_DELAY: Duration = Duration::from_millis(400);

/// One pasteboard item as its (type, data) pairs.
pub(crate) type Item = Vec<(String, Vec<u8>)>;
/// Every item on the pasteboard.
pub(crate) type Items = Vec<Item>;

/// The raw pasteboard.
pub(crate) trait Board: Send + Sync {
    fn change_count(&self) -> isize;
    /// Every item and type; `None` if some promised data can't be read (restoring would lose it).
    fn snapshot(&self) -> Option<Items>;
    /// Every item and type, skipping types whose (promised) data can't be read.
    fn read_items(&self) -> Items;
    /// Clears the pasteboard, then writes `items`.
    fn write_items(&self, items: &[Item]);
    /// Writes `text` (and `html`) as one item marked transient and concealed, so clipboard
    /// managers skip it.
    fn write_transient(&self, text: &str, html: Option<&str>) -> bool;
    /// Plain-text contents.
    fn read_string(&self) -> Option<String>;
    /// Writes plain text as the only item.
    fn write_string(&self, text: &str) -> bool;
}

/// Why a paste failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PasteError {
    Unavailable,
    Key(KeyError),
}

/// The pasteboard and the records that must outlive any one write.
pub(crate) struct Pasteboard {
    board: Box<dyn Board>,
    /// A paste whose old clipboard still has to come back: (saved items, change count after our write).
    pending: Mutex<Option<(Items, isize)>>,
    /// Change counts produced by Kudzu's own writes (restores, copies), newest last. The
    /// clipboard history skips these so putting the user's clipboard back never looks like a copy.
    own: Mutex<[isize; 8]>,
}

impl Pasteboard {
    pub fn new(board: Box<dyn Board>) -> Self {
        Self {
            board,
            pending: Mutex::new(None),
            own: Mutex::new([-1; 8]),
        }
    }

    /// The process-wide pasteboard.
    pub fn shared() -> &'static Arc<Pasteboard> {
        static SHARED: OnceLock<Arc<Pasteboard>> = OnceLock::new();
        SHARED.get_or_init(|| Arc::new(Pasteboard::new(Box::new(SystemBoard))))
    }

    fn note_own_change(&self) {
        let count = self.board.change_count();
        let mut own = lock(&self.own);
        own.rotate_left(1);
        own[7] = count;
    }

    /// Whether `count` came from one of Kudzu's own writes.
    pub fn is_own_change(&self, count: isize) -> bool {
        lock(&self.own).contains(&count)
    }

    /// Current change count of the pasteboard.
    pub fn change_count(&self) -> isize {
        self.board.change_count()
    }

    /// Every item and type, skipping types whose (promised) data can't be read.
    pub fn read_items(&self) -> Items {
        self.board.read_items()
    }

    /// Replaces the pasteboard with `items` (a clipboard-history pick, as Kudzu's own change).
    /// Returns the new change count. A snippet paste still waiting to restore is finished
    /// first, so it can't overwrite what the user picks now.
    pub fn write_items(&self, items: &[Item]) -> isize {
        self.restore_pending();
        self.board.write_items(items);
        self.note_own_change();
        self.board.change_count()
    }

    fn restore(&self, saved: &[Item]) {
        self.board.write_items(saved);
        self.note_own_change();
    }

    /// Plain-text contents.
    pub fn read_string(&self) -> Option<String> {
        self.board.read_string()
    }

    /// Writes plain text as the only item (as Kudzu's own change).
    pub fn write_string(&self, text: &str) -> bool {
        let ok = self.board.write_string(text);
        self.note_own_change();
        ok
    }

    /// Pastes `text` via ⌘V, then restores the previous clipboard after [`RESTORE_DELAY`]
    /// (blocking, on `clock`).
    pub fn paste_text(
        &self,
        events: &dyn Events,
        clock: &dyn Clock,
        text: &str,
        html: Option<&str>,
    ) -> Result<(), PasteError> {
        // A previous paste still pending restoration owns the "real" clipboard; put it back first.
        self.restore_pending();
        let saved = self.board.snapshot().ok_or(PasteError::Unavailable)?;
        if !self.board.write_transient(text, html) {
            self.restore(&saved);
            return Err(PasteError::Unavailable);
        }
        *lock(&self.pending) = Some((saved, self.board.change_count()));
        if let Err(error) = keys::paste(events) {
            self.restore_pending();
            return Err(PasteError::Key(error));
        }
        clock.sleep(RESTORE_DELAY);
        self.restore_pending();
        Ok(())
    }

    /// Restores the clipboard a paste replaced, unless the user copied something since. Also
    /// the pasteboard's shutdown: the engine calls it when it stops.
    pub fn restore_pending(&self) {
        let Some((saved, count)) = lock(&self.pending).take() else {
            return;
        };
        if self.board.change_count() == count {
            self.restore(&saved);
        }
    }

    /// Copies the frontmost app's selection with ⌘C, waiting up to `timeout`; restores the clipboard.
    pub fn copy_selection(&self, events: &dyn Events, timeout: Duration) -> Option<String> {
        self.restore_pending();
        let saved = self.board.snapshot()?;
        let original = self.board.change_count();
        keys::copy(events).ok()?;
        let deadline = Instant::now() + timeout;
        let mut text = None;
        while Instant::now() < deadline {
            thread::sleep(Duration::from_millis(10));
            if self.board.change_count() != original {
                text = self.board.read_string().filter(|s| !s.is_empty());
                break;
            }
        }
        if self.board.change_count() != original {
            self.restore(&saved);
        }
        text
    }
}

/// The general `NSPasteboard`.
pub(crate) struct SystemBoard;

fn general() -> Retained<NSPasteboard> {
    NSPasteboard::generalPasteboard()
}

impl Board for SystemBoard {
    fn change_count(&self) -> isize {
        autoreleasepool(|_| general().changeCount())
    }

    fn snapshot(&self) -> Option<Items> {
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
            Some(items)
        })
    }

    fn read_items(&self) -> Items {
        autoreleasepool(|_| {
            let board = general();
            let mut items = Vec::new();
            if let Some(list) = board.pasteboardItems() {
                for item in list.iter() {
                    let entries: Vec<(String, Vec<u8>)> = item
                        .types()
                        .iter()
                        .filter_map(|kind| {
                            item.dataForType(&kind)
                                .map(|data| (kind.to_string(), data.to_vec()))
                        })
                        .collect();
                    if !entries.is_empty() {
                        items.push(entries);
                    }
                }
            }
            items
        })
    }

    fn write_items(&self, items: &[Item]) {
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

    fn write_transient(&self, text: &str, html: Option<&str>) -> bool {
        autoreleasepool(|_| {
            let item = NSPasteboardItem::new();
            let empty = NSData::new();
            let ok = item
                .setString_forType(&NSString::from_str(text), unsafe { NSPasteboardTypeString })
                && item.setData_forType(
                    &empty,
                    &NSString::from_str("org.nspasteboard.TransientType"),
                )
                && item.setData_forType(
                    &empty,
                    &NSString::from_str("org.nspasteboard.ConcealedType"),
                );
            if !ok
                || html.is_some_and(|html| {
                    !item.setString_forType(
                        &NSString::from_str(html),
                        &NSString::from_str("public.html"),
                    )
                })
            {
                return false;
            }
            let board = general();
            board.clearContents();
            let objects = NSArray::from_retained_slice(&[ProtocolObject::from_retained(item)]);
            board.writeObjects(&objects)
        })
    }

    fn read_string(&self) -> Option<String> {
        autoreleasepool(|_| {
            general()
                .stringForType(unsafe { NSPasteboardTypeString })
                .map(|s| s.to_string())
        })
    }

    fn write_string(&self, text: &str) -> bool {
        autoreleasepool(|_| {
            let board = general();
            board.clearContents();
            board.setString_forType(&NSString::from_str(text), unsafe { NSPasteboardTypeString })
        })
    }
}
