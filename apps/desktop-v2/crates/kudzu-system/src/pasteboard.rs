//! `Pasteboard` over the general NSPasteboard.
//!
//! Reads go straight to NSPasteboard: `read` keeps every type of every item, with empty bytes
//! for empty or unreadable promised data, because the clipboard service checks markers before
//! it looks at data. `change_count`, `is_own` and `has_private_marker` never touch item data.
//! Writes, `can_paste` and `paste` go through `kudzu_engine::clipboard`, the one owner of the
//! process's pasteboard transactions and its own-change record, so a history write never
//! races a snippet paste and its restore.
//!
//! Blocking (call from workers, never the UI thread): `read` may wait on another app's
//! promised data; `write` first finishes a snippet paste still waiting to restore, so it can
//! wait up to about half a second; `paste` activates the target and waits up to ~850 ms.
//! `change_count`, `is_own`, `has_private_marker` and `read_text` return promptly.
use kudzu_app::{
    error::PortError,
    ports::{PasteSnapshot, Pasteboard},
};
use kudzu_core::{
    clipboard::{uti::PRIVATE_MARKERS, PasteItem},
    system::AppInfo,
};
use objc2::rc::{autoreleasepool, Retained};
use objc2_app_kit::{NSPasteboard, NSPasteboardTypeString};
use objc2_foundation::NSString;

/// A read that keeps seeing the count move gives up rather than return a mixed snapshot.
const READ_ATTEMPTS: usize = 3;

#[derive(Default)]
pub struct SystemPasteboard {
    /// A private named pasteboard instead of the general one. Tests only: it is read-only
    /// through this adapter, because Kudzu's writes belong to the engine's record.
    named: Option<String>,
}

impl SystemPasteboard {
    /// The general pasteboard.
    pub fn general() -> Self {
        Self::default()
    }

    #[cfg(test)]
    fn named(name: String) -> Self {
        Self { named: Some(name) }
    }

    fn board(&self) -> Retained<NSPasteboard> {
        match &self.named {
            None => NSPasteboard::generalPasteboard(),
            Some(name) => NSPasteboard::pasteboardWithName(&NSString::from_str(name)),
        }
    }

    fn recorded_as_own(&self, count: i64) -> bool {
        self.named.is_none() && kudzu_engine::clipboard::is_own_change(count)
    }
}

impl Pasteboard for SystemPasteboard {
    fn change_count(&self) -> i64 {
        autoreleasepool(|_| self.board().changeCount() as i64)
    }

    /// False for a count that is no longer current, even if Kudzu wrote it.
    fn is_own(&self, change_count: i64) -> bool {
        self.change_count() == change_count && self.recorded_as_own(change_count)
    }

    /// `Cancelled` when the count is not current before or after the type scan.
    fn has_private_marker(&self, change_count: i64) -> Result<bool, PortError> {
        autoreleasepool(|_| {
            let board = self.board();
            let current = || board.changeCount() as i64 == change_count;
            if !current() {
                return Err(PortError::Cancelled);
            }
            let private = |kind: &NSString| PRIVATE_MARKERS.contains(&kind.to_string().as_str());
            let mut found = board
                .types()
                .is_some_and(|types| types.iter().any(|kind| private(&kind)));
            if let Some(items) = board.pasteboardItems() {
                found |= items
                    .iter()
                    .any(|item| item.types().iter().any(|kind| private(&kind)));
            }
            if !current() {
                return Err(PortError::Cancelled);
            }
            Ok(found)
        })
    }

    /// `Cancelled` when the pasteboard kept changing while it was read.
    fn read(&self) -> Result<PasteSnapshot, PortError> {
        for _ in 0..READ_ATTEMPTS {
            let (before, items, after) = autoreleasepool(|_| {
                let board = self.board();
                let before = board.changeCount() as i64;
                let items = read_items(&board);
                (before, items, board.changeCount() as i64)
            });
            if before == after {
                return Ok(PasteSnapshot {
                    change_count: before,
                    own: self.recorded_as_own(before),
                    items,
                });
            }
        }
        Err(PortError::Cancelled)
    }

    /// The plain-text flavour only; no other type's data is loaded.
    fn read_text(&self) -> Result<Option<String>, PortError> {
        Ok(autoreleasepool(|_| {
            // SAFETY: a framework constant, read after AppKit has loaded.
            let kind = unsafe { NSPasteboardTypeString };
            self.board().stringForType(kind).map(|s| s.to_string())
        }))
    }

    /// Recorded as Kudzu's own change. Finishes a pending snippet-paste restore first.
    fn write(&self, items: &[PasteItem]) -> Result<i64, PortError> {
        if self.named.is_some() {
            return Err(PortError::Unavailable);
        }
        Ok(kudzu_engine::clipboard::write(items))
    }

    fn can_paste(&self) -> bool {
        kudzu_engine::clipboard::can_paste()
    }

    fn paste(&self, target: Option<&AppInfo>) -> Result<(), PortError> {
        use kudzu_engine::clipboard::PasteError;
        kudzu_engine::clipboard::paste(target).map_err(|error| match error {
            PasteError::NotAllowed => PortError::PermissionDenied,
            PasteError::TargetGone => PortError::TargetGone,
        })
    }
}

/// Every item with every declared type; data that is empty, or promised and unreadable, is
/// kept as an empty value so its marker survives. Items without types carry nothing.
fn read_items(board: &NSPasteboard) -> Vec<PasteItem> {
    let Some(list) = board.pasteboardItems() else {
        return Vec::new();
    };
    list.iter()
        .map(|item| {
            item.types()
                .iter()
                .map(|kind| {
                    let data = item.dataForType(&kind).map(|d| d.to_vec());
                    (kind.to_string(), data.unwrap_or_default())
                })
                .collect::<PasteItem>()
        })
        .filter(|item| !item.is_empty())
        .collect()
}

#[cfg(test)]
mod tests;
