//! A borderless window under the spike window, painted with a fixed test pattern, so the
//! material shows the same thing behind it in every capture, whatever the wallpaper is.
//! It ignores the mouse and never becomes key.

use std::io::Cursor;

use image::{ImageFormat, Rgba, RgbaImage};
use objc2::rc::{Allocated, Retained};
use objc2::runtime::AnyObject;
use objc2::{class, msg_send};
use objc2_app_kit::{NSBackingStoreType, NSWindow, NSWindowStyleMask};
use objc2_foundation::{MainThreadMarker, NSData, NSPoint, NSRect, NSSize};

/// Bright saturated hues in horizontal bands, a diagonal sweep, hard black and white blocks and
/// a fine stripe field: blur, tint and luminance shifts all show on it.
fn pattern(w: u32, h: u32) -> RgbaImage {
    let mut img = RgbaImage::new(w, h);
    for y in 0..h {
        for x in 0..w {
            let band = (y * 7 / h) as f32;
            let hue = (band * 51.0 + x as f32 * 0.12 + y as f32 * 0.05) % 360.0;
            let (r, g, b) = hsv(hue, 0.85, 0.95);
            let stripe = (x / 6 + y / 6) % 2 == 0 && x < w / 4;
            let (r, g, b) = if stripe {
                (255 - r, 255 - g, 255 - b)
            } else {
                (r, g, b)
            };
            img.put_pixel(x, y, Rgba([r, g, b, 255]));
        }
    }
    for (bx, by, bw, bh, v) in [
        (0.30, 0.10, 0.12, 0.20, 255u8),
        (0.30, 0.55, 0.12, 0.20, 0),
        (0.05, 0.40, 0.10, 0.12, 255),
        (0.05, 0.80, 0.10, 0.12, 0),
    ] {
        for y in (by * h as f32) as u32..((by + bh) * h as f32) as u32 {
            for x in (bx * w as f32) as u32..((bx + bw) * w as f32) as u32 {
                img.put_pixel(x, y, Rgba([v, v, v, 255]));
            }
        }
    }
    img
}

fn hsv(h: f32, s: f32, v: f32) -> (u8, u8, u8) {
    let c = v * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = v - c;
    let (r, g, b) = match (h / 60.0) as u32 % 6 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    (
        ((r + m) * 255.0) as u8,
        ((g + m) * 255.0) as u8,
        ((b + m) * 255.0) as u8,
    )
}

/// Open the backdrop behind `frame` (AppKit coordinates), `margin` points larger on each side.
pub fn open(mtm: MainThreadMarker, frame: NSRect, margin: f64) -> Retained<NSWindow> {
    let rect = NSRect::new(
        NSPoint::new(frame.origin.x - margin, frame.origin.y - margin),
        NSSize::new(
            frame.size.width + 2.0 * margin,
            frame.size.height + 2.0 * margin,
        ),
    );
    let (pw, ph) = (
        (rect.size.width * 2.0) as u32,
        (rect.size.height * 2.0) as u32,
    );
    let mut png = Vec::new();
    pattern(pw, ph)
        .write_to(&mut Cursor::new(&mut png), ImageFormat::Png)
        .expect("encode the pattern");
    // SAFETY: AppKit window and image construction on the main thread.
    unsafe {
        let window = NSWindow::initWithContentRect_styleMask_backing_defer(
            mtm.alloc(),
            rect,
            NSWindowStyleMask::Borderless,
            NSBackingStoreType::Buffered,
            false,
        );
        let data = NSData::with_bytes(&png);
        let alloc: Allocated<AnyObject> = msg_send![class!(NSImage), alloc];
        let image: Retained<AnyObject> = msg_send![alloc, initWithData: &*data];
        let _: () = msg_send![&*image, setSize: NSSize::new(rect.size.width, rect.size.height)];
        let color: Retained<AnyObject> = msg_send![class!(NSColor), colorWithPatternImage: &*image];
        let _: () = msg_send![&*window, setBackgroundColor: &*color];
        let _: () = msg_send![&*window, setReleasedWhenClosed: false];
        let _: () = msg_send![&*window, setIgnoresMouseEvents: true];
        let _: () = msg_send![&*window, setHasShadow: false];
        let _: () = msg_send![&*window, setLevel: 3isize];
        let _: () = msg_send![&*window, orderFront: std::ptr::null::<AnyObject>()];
        window
    }
}
