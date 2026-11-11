//! Clipboard history plumbing: read every type, write entries back, paste; and image thumbnails,
//! which S35 moves to `kudzu-system::thumbnails` with their test.

use std::thread;
use std::time::Duration;

use objc2::rc::autoreleasepool;
use objc2::AnyThread;
use objc2_app_kit::NSImage;
use objc2_foundation::NSData;

use super::events::SystemEvents;
use super::pasteboard::Pasteboard;
use super::{apps, keys};
use crate::clipboard::PasteError;
use crate::outgoing::Thumbnail;
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

pub fn thumbnail(data: &[u8], max_px: u32) -> Option<Thumbnail> {
    autoreleasepool(|_| {
        let image = NSImage::initWithData(NSImage::alloc(), &NSData::with_bytes(data))?;
        // Pixel size from the largest bitmap representation; vector images use their point size.
        let (mut w, mut h) = (0f64, 0f64);
        for rep in image.representations().iter() {
            let (pw, ph) = (rep.pixelsWide() as f64, rep.pixelsHigh() as f64);
            if pw * ph > w * h {
                (w, h) = (pw, ph);
            }
        }
        if w <= 0.0 || h <= 0.0 {
            let s = image.size();
            (w, h) = (s.width, s.height);
        }
        if w < 1.0 || h < 1.0 {
            return None;
        }
        let scale = (f64::from(max_px) / w.max(h)).min(1.0);
        let (tw, th) = (
            ((w * scale).round() as u32).max(1),
            ((h * scale).round() as u32).max(1),
        );
        let png = apps::draw_png(&image, tw, th)?;
        Some(Thumbnail {
            png,
            width: w.round() as u32,
            height: h.round() as u32,
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thumbnails_scale_down_and_report_pixel_size() {
        let Some(app) = apps::installed_apps()
            .into_iter()
            .find(|a| a.path.is_some())
        else {
            return;
        };
        let png = apps::icon_png(app.path.as_deref().unwrap(), 256).expect("icon");
        let thumb = thumbnail(&png, 64).expect("thumbnail");
        assert_eq!((thumb.width, thumb.height), (256, 256));
        assert_eq!(&thumb.png[..8], b"\x89PNG\r\n\x1a\n");
        let small = thumbnail(&thumb.png, 1024).expect("no upscaling");
        assert_eq!((small.width, small.height), (64, 64));
        assert!(thumbnail(b"not an image", 64).is_none());
    }
}
