//! `Thumbnailer` with NSImage, so HEIC, TIFF and PDF decode as they do in the system. Moved
//! from the engine in S35. Decoding runs on the calling thread: call from a worker.
use crate::apps::draw_png;
use kudzu_app::{
    error::PortError,
    ports::{Thumbnail, Thumbnailer},
};
use objc2::{rc::autoreleasepool, AnyThread};
use objc2_app_kit::NSImage;
use objc2_foundation::NSData;

#[derive(Default)]
pub struct ImageThumbnails;

impl Thumbnailer for ImageThumbnails {
    /// `Ok(None)` for bytes AppKit cannot decode.
    fn thumbnail(&self, bytes: &[u8], max_px: u32) -> Result<Option<Thumbnail>, PortError> {
        Ok(thumbnail(bytes, max_px))
    }
}

/// A PNG at most `max_px` on its longer side (never upscaled), plus the original pixel size.
fn thumbnail(data: &[u8], max_px: u32) -> Option<Thumbnail> {
    if data.is_empty() || max_px == 0 {
        return None;
    }
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
        let png = draw_png(&image, tw, th)?;
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
    use crate::apps::{icon_png, installed_apps};
    use kudzu_app::fixtures::contract::thumbnails::{self as contract, png_size, ThumbnailHarness};

    struct Real(ImageThumbnails, Vec<u8>);
    impl ThumbnailHarness for Real {
        fn thumbnailer(&self) -> &dyn Thumbnailer {
            &self.0
        }
        fn sample(&self) -> (Vec<u8>, u32, u32) {
            (self.1.clone(), 256, 256)
        }
    }

    /// An app icon rendered at 256 px: real image bytes without shipping a fixture.
    fn sample_png() -> Option<Vec<u8>> {
        let app = installed_apps().into_iter().find(|a| a.path.is_some())?;
        icon_png(app.path.as_deref()?, 256)
    }

    #[test]
    fn image_thumbnails_pass_the_thumbnail_contract() {
        let Some(png) = sample_png() else {
            return; // Not a regular macOS install (CI sandbox).
        };
        contract::bounds(&Real(ImageThumbnails, png));
    }

    #[test]
    fn thumbnails_scale_down_and_report_pixel_size() {
        let Some(png) = sample_png() else {
            return;
        };
        let thumb = thumbnail(&png, 64).expect("thumbnail");
        assert_eq!((thumb.width, thumb.height), (256, 256));
        assert_eq!(&thumb.png[..8], b"\x89PNG\r\n\x1a\n");
        assert_eq!(png_size(&thumb.png), Some((64, 64)));
        let small = thumbnail(&thumb.png, 1024).expect("no upscaling");
        assert_eq!((small.width, small.height), (64, 64));
        assert!(thumbnail(b"not an image", 64).is_none());
        assert!(ImageThumbnails.thumbnail(&[], 64).unwrap().is_none());
    }
}
