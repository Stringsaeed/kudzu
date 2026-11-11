//! What a `Thumbnailer` promises: the original pixel size, and PNG bytes no larger than the
//! bound on their longer side, never upscaled.
use crate::ports::Thumbnailer;

pub trait ThumbnailHarness {
    fn thumbnailer(&self) -> &dyn Thumbnailer;
    /// Encoded image bytes and their pixel size.
    fn sample(&self) -> (Vec<u8>, u32, u32);
}

/// Width and height from a PNG's IHDR chunk.
pub fn png_size(png: &[u8]) -> Option<(u32, u32)> {
    if !png.starts_with(super::apps::PNG_MAGIC) || png.len() < 24 || &png[12..16] != b"IHDR" {
        return None;
    }
    let word = |at: usize| u32::from_be_bytes(png[at..at + 4].try_into().unwrap());
    Some((word(16), word(20)))
}

pub fn bounds(h: &dyn ThumbnailHarness) {
    let (bytes, width, height) = h.sample();
    let bound = width.max(height) / 4;
    let thumb = h.thumbnailer().thumbnail(&bytes, bound).unwrap().unwrap();
    assert_eq!(
        (thumb.width, thumb.height),
        (width, height),
        "original size"
    );
    let (w, h_px) = png_size(&thumb.png).expect("PNG output");
    assert!(w.max(h_px) <= bound && w >= 1 && h_px >= 1);
    let big = h
        .thumbnailer()
        .thumbnail(&bytes, width.max(height) * 4)
        .unwrap()
        .unwrap();
    let (w, h_px) = png_size(&big.png).unwrap();
    assert!(w <= width && h_px <= height, "never upscaled");
}
