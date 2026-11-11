//! Clipboard history plumbing: read every type, write entries back, paste.

use std::thread;
use std::time::Duration;

use super::events::SystemEvents;
use super::pasteboard::Pasteboard;
use super::{apps, keys};
use crate::clipboard::PasteError;
use crate::AppInfo;

pub fn change_count() -> i64 {
    Pasteboard::shared().change_count() as i64
}

pub fn is_own_change(count: i64) -> bool {
    Pasteboard::shared().is_own_change(count as isize)
}

pub fn read() -> Vec<Vec<(String, Vec<u8>)>> {
    Pasteboard::shared().read_items()
}

pub fn write(items: &[Vec<(String, Vec<u8>)>]) -> i64 {
    // Finishes a snippet paste still waiting to restore first (see `Pasteboard::write_items`).
    Pasteboard::shared().write_items(items) as i64
}

pub fn can_paste() -> bool {
    keys::can_post()
}

pub fn paste(target: Option<&AppInfo>) -> Result<(), PasteError> {
    if let Some(app) = target {
        if !apps::activate_and_wait(app, Duration::from_millis(800)) {
            return Err(PasteError::TargetGone);
        }
        // Let the target's key window settle before ⌘V.
        thread::sleep(Duration::from_millis(40));
    }
    keys::paste(&SystemEvents).map_err(|_| PasteError::NotAllowed)
}
