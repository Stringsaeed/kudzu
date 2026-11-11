//! General pasteboard access with full save and restore, and paste-based insertion.
//!
//! [`Pasteboard`] owns the two records every pasteboard write in the process must share: the
//! paste whose old clipboard still has to come back, and the change counts of Kudzu's own
//! writes. Snippet pastes, restores, clipboard-history writes and selection copies all go
//! through it. The system pasteboard is one per process, so [`Pasteboard::shared`] is the one
//! process-wide instance; [`Pasteboard::restore_pending`] is its shutdown. Kudzu's own
//! operations on it run one at a time, as transactions. The raw board is a seam ([`Board`]) so
//! the protocol tests run on a fake.

use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
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
    /// Kudzu's own transactions on the board, one at a time: a paste from its snapshot to its
    /// ⌘V, a restore, a history write, a selection copy. Without it two could interleave, and
    /// one would save the other's transient snippet as the user's clipboard, paste the other's
    /// item, or overwrite its record. Lock order: this, then `records`.
    transaction: Mutex<()>,
    records: Mutex<Records>,
}

/// Held while a transaction runs: the methods that take one need the lock already held.
type Transaction<'a> = MutexGuard<'a, ()>;

struct Records {
    /// The paste whose old clipboard still has to come back.
    pending: Option<Pending>,
    /// Change counts produced by Kudzu's own writes (restores, copies), newest last. The
    /// clipboard history skips these so putting the user's clipboard back never looks like a copy.
    own: [isize; 8],
    next_paste: u64,
}

struct Pending {
    paste: u64,
    saved: Items,
    /// The change count after the paste's own write.
    count: isize,
}

impl Pasteboard {
    pub fn new(board: Box<dyn Board>) -> Self {
        Self {
            board,
            transaction: Mutex::new(()),
            records: Mutex::new(Records {
                pending: None,
                own: [-1; 8],
                next_paste: 0,
            }),
        }
    }

    /// The process-wide pasteboard.
    pub fn shared() -> &'static Arc<Pasteboard> {
        static SHARED: OnceLock<Arc<Pasteboard>> = OnceLock::new();
        SHARED.get_or_init(|| Arc::new(Pasteboard::new(Box::new(SystemBoard))))
    }

    fn begin(&self) -> Transaction<'_> {
        lock(&self.transaction)
    }

    fn note_own_change(&self, _: &Transaction<'_>) {
        let count = self.board.change_count();
        let mut records = lock(&self.records);
        records.own.rotate_left(1);
        records.own[7] = count;
    }

    /// Whether `count` came from one of Kudzu's own writes. Never waits for a transaction.
    pub fn is_own_change(&self, count: isize) -> bool {
        lock(&self.records).own.contains(&count)
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
        let transaction = self.begin();
        self.restore_pending_in(&transaction);
        self.board.write_items(items);
        self.note_own_change(&transaction);
        self.board.change_count()
    }

    fn restore(&self, transaction: &Transaction<'_>, saved: &[Item]) {
        self.board.write_items(saved);
        self.note_own_change(transaction);
    }

    /// Plain-text contents.
    pub fn read_string(&self) -> Option<String> {
        self.board.read_string()
    }

    /// Writes plain text as the only item (as Kudzu's own change).
    pub fn write_string(&self, text: &str) -> bool {
        let transaction = self.begin();
        let ok = self.board.write_string(text);
        self.note_own_change(&transaction);
        ok
    }

    /// Pastes `text` via ⌘V, then restores the previous clipboard after [`RESTORE_DELAY`]
    /// (blocking, on `clock`). The engine prepares first ([`Pasteboard::prepare_paste`]); tests
    /// paste in one go.
    #[cfg(test)]
    pub fn paste_text(
        &self,
        events: &dyn Events,
        clock: &dyn Clock,
        text: &str,
        html: Option<&str>,
    ) -> Result<(), PasteError> {
        self.prepare_paste(text, html)?.paste(events, clock)
    }

    /// The first half of a paste: saves the user's clipboard and writes `text` (and `html`) in
    /// its place, ready for [`PreparedPaste::paste`]. Nothing is posted yet, so a caller can do
    /// this before it deletes anything (D14). Fails, leaving the user's clipboard as it was,
    /// when the clipboard can't be saved or written.
    ///
    /// The transaction lasts until the ⌘V is posted (or the paste is dropped): no other Kudzu
    /// write can come between the snapshot, the write, the record and the keystroke.
    pub fn prepare_paste(
        &self,
        text: &str,
        html: Option<&str>,
    ) -> Result<PreparedPaste<'_>, PasteError> {
        let transaction = self.begin();
        // A previous paste still pending restoration owns the "real" clipboard; put it back first.
        self.restore_pending_in(&transaction);
        let saved = self.board.snapshot().ok_or(PasteError::Unavailable)?;
        if !self.board.write_transient(text, html) {
            self.restore(&transaction, &saved);
            return Err(PasteError::Unavailable);
        }
        let count = self.board.change_count();
        let paste = {
            let mut records = lock(&self.records);
            records.next_paste += 1;
            let paste = records.next_paste;
            records.pending = Some(Pending {
                paste,
                saved,
                count,
            });
            paste
        };
        Ok(PreparedPaste {
            pasteboard: self,
            paste,
            transaction: Some(transaction),
        })
    }

    /// Restores the clipboard a paste replaced, unless the user copied something since. Also
    /// the pasteboard's shutdown: the engine calls it when it stops.
    pub fn restore_pending(&self) {
        let transaction = self.begin();
        self.restore_pending_in(&transaction);
    }

    fn restore_pending_in(&self, transaction: &Transaction<'_>) {
        let pending = lock(&self.records).pending.take();
        self.put_back(transaction, pending);
    }

    /// Restores paste `paste`'s clipboard if it is still the pending one: a paste's late
    /// restore never finishes a newer paste's early.
    fn restore_paste_in(&self, transaction: &Transaction<'_>, paste: u64) {
        let pending = {
            let mut records = lock(&self.records);
            match records.pending.take() {
                Some(pending) if pending.paste == paste => Some(pending),
                other => {
                    records.pending = other;
                    None
                }
            }
        };
        self.put_back(transaction, pending);
    }

    fn put_back(&self, transaction: &Transaction<'_>, pending: Option<Pending>) {
        if let Some(pending) = pending {
            if self.board.change_count() == pending.count {
                self.restore(transaction, &pending.saved);
            }
        }
    }

    /// Copies the frontmost app's selection with ⌘C, waiting up to `timeout`; restores the
    /// clipboard. One transaction throughout, so a paste can't save the copied selection.
    pub fn copy_selection(&self, events: &dyn Events, timeout: Duration) -> Option<String> {
        let transaction = self.begin();
        self.restore_pending_in(&transaction);
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
            self.restore(&transaction, &saved);
        }
        text
    }
}

/// A paste whose clipboard is saved and written, waiting for its ⌘V, inside its transaction.
/// Dropping it without pasting puts the user's clipboard back at once.
pub(crate) struct PreparedPaste<'a> {
    pasteboard: &'a Pasteboard,
    paste: u64,
    /// Held until the ⌘V is posted.
    transaction: Option<Transaction<'a>>,
}

impl PreparedPaste<'_> {
    /// Presses ⌘V and ends the transaction, then restores the user's clipboard after
    /// [`RESTORE_DELAY`] (blocking, on `clock`). When ⌘V can't be posted the clipboard comes
    /// back at once.
    pub fn paste(mut self, events: &dyn Events, clock: &dyn Clock) -> Result<(), PasteError> {
        let pasteboard = self.pasteboard;
        let Some(transaction) = self.transaction.take() else {
            return Ok(());
        };
        if let Err(error) = keys::paste(events) {
            pasteboard.restore_paste_in(&transaction, self.paste);
            return Err(PasteError::Key(error));
        }
        drop(transaction);
        clock.sleep(RESTORE_DELAY);
        let transaction = pasteboard.begin();
        pasteboard.restore_paste_in(&transaction, self.paste);
        Ok(())
    }
}

impl Drop for PreparedPaste<'_> {
    fn drop(&mut self) {
        if let Some(transaction) = self.transaction.take() {
            self.pasteboard.restore_paste_in(&transaction, self.paste);
        }
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
