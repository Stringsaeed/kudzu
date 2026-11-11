//! RUNE-GAP(#141): delete when Rune supports template tray images with scale representations.
use image::{ImageEncoder, Rgba, RgbaImage};
use objc2::rc::Retained;
use objc2_app_kit::{NSBitmapImageRep, NSImage};
use objc2_foundation::{MainThreadMarker, NSData, NSSize};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GlyphKind {
    Normal,
    Paused,
    SecureInput,
    Permissions,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Glyph {
    pub kind: GlyphKind,
    pub update: bool,
}

impl Glyph {
    /// The committed 1x, 2x and 3x glyphs, plus a black template dot and clear halo.
    pub fn pixels(self, scale: u32) -> Result<RgbaImage, &'static str> {
        let reps: [&[u8]; 3] = match self.kind {
            GlyphKind::Normal => [
                include_bytes!("../../../../assets/tray/normal.png"),
                include_bytes!("../../../../assets/tray/normal@2x.png"),
                include_bytes!("../../../../assets/tray/normal@3x.png"),
            ],
            GlyphKind::Paused => [
                include_bytes!("../../../../assets/tray/paused.png"),
                include_bytes!("../../../../assets/tray/paused@2x.png"),
                include_bytes!("../../../../assets/tray/paused@3x.png"),
            ],
            GlyphKind::SecureInput => [
                include_bytes!("../../../../assets/tray/secure-input.png"),
                include_bytes!("../../../../assets/tray/secure-input@2x.png"),
                include_bytes!("../../../../assets/tray/secure-input@3x.png"),
            ],
            GlyphKind::Permissions => [
                include_bytes!("../../../../assets/tray/permissions.png"),
                include_bytes!("../../../../assets/tray/permissions@2x.png"),
                include_bytes!("../../../../assets/tray/permissions@3x.png"),
            ],
        };
        let bytes = scale
            .checked_sub(1)
            .and_then(|s| reps.get(s as usize))
            .ok_or("glyph scale must be 1, 2 or 3")?;
        let mut image = image::load_from_memory(bytes)
            .map_err(|_| "invalid committed glyph")?
            .to_rgba8();
        if self.update {
            for (x, y, pixel) in image.enumerate_pixels_mut() {
                let dx = (x as f64 + 0.5) / scale as f64 - 14.;
                let dy = (y as f64 + 0.5) / scale as f64 - 14.;
                let distance = dx.hypot(dy);
                if distance < 4. {
                    *pixel = Rgba([
                        0,
                        0,
                        0,
                        ((2.5 - distance).clamp(0., 1.) * 255.).round() as u8,
                    ]);
                }
            }
        }
        Ok(image)
    }

    pub(crate) fn native(self, mtm: MainThreadMarker) -> Result<Retained<NSImage>, &'static str> {
        let size = NSSize::new(18., 18.);
        let image = NSImage::initWithSize(mtm.alloc(), size);
        for scale in 1..=3 {
            let pixels = self.pixels(scale)?;
            let mut png = Vec::new();
            image::codecs::png::PngEncoder::new(&mut png)
                .write_image(
                    pixels.as_raw(),
                    pixels.width(),
                    pixels.height(),
                    image::ExtendedColorType::Rgba8,
                )
                .map_err(|_| "glyph PNG encoding failed")?;
            let rep = NSBitmapImageRep::imageRepWithData(&NSData::with_bytes(&png))
                .ok_or("AppKit glyph decoding failed")?;
            rep.setSize(size);
            image.addRepresentation(&rep);
        }
        image.setTemplate(true);
        Ok(image)
    }
}
