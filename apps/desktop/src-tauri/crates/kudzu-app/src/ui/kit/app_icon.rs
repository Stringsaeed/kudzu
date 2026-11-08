//! `components/app/app-icon.tsx`: an app's icon from its bundle, falling back to a monogram tile.
//!
//! Icons load once per bundle path, on a worker (`backend::commands::app_icon` is blocking), and
//! stay cached for the process. The monogram shows until the icon arrives, then fades out without
//! the view running again (the icon's pixels and the monogram's opacity are live values).

// The kit's whole API is ported now; screens adopt it as they port.
#![allow(dead_code)]

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use rune::{text, El, Live, Pixels, TextStyle};

use crate::ui::theme::{weight, Theme};

struct Loaded {
    pixels: Pixels,
    /// The monogram's opacity: 1 until the icon decodes.
    monogram: Live<f32>,
}

fn cache() -> &'static Mutex<HashMap<String, &'static Loaded>> {
    static CACHE: OnceLock<Mutex<HashMap<String, &'static Loaded>>> = OnceLock::new();
    CACHE.get_or_init(Default::default)
}

/// The cached icon for `path`, loading it the first time it's asked for.
fn load(path: &str) -> &'static Loaded {
    let mut map = cache().lock().unwrap_or_else(|e| e.into_inner());
    if let Some(l) = map.get(path) {
        return l;
    }
    let loaded: &'static Loaded = Box::leak(Box::new(Loaded { pixels: Pixels::new(), monogram: Live::new(1.0) }));
    map.insert(path.to_string(), loaded);
    let owned = path.to_string();
    let _ = std::thread::Builder::new().name("kudzu-app-icon".into()).spawn(move || {
        let Some(png) = crate::backend::commands::app_icon(owned) else { return };
        let Ok(img) = image::load_from_memory(&png) else { return };
        let rgba = img.to_rgba8();
        let (w, h) = rgba.dimensions();
        loaded.pixels.set(w, h, rgba.into_raw());
        loaded.monogram.set(0.0);
    });
    loaded
}

/// `<AppIcon path name size />` (18 pt by default).
pub fn app_icon(th: &Theme, path: Option<&str>, name: &str, size: f32) -> El {
    let t = &th.t;
    let letter: String = name.chars().next().map(|c| c.to_uppercase().collect()).unwrap_or_default();
    // `inline-flex items-center justify-center rounded-[5px] bg-foreground/[0.08] font-semibold
    // text-muted-foreground`, its letter half the tile's size.
    let font = TextStyle::new(size * 0.5, weight::SEMIBOLD).lh(size * 0.5);
    let monogram = rune::el()
        .pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0))
        .center()
        .radius(5.0)
        .fill(Theme::tint(t.foreground, 0.08))
        .child(text(letter).font(font).color(t.muted_foreground));
    let mut e = rune::el().size(size, size).a11y_hidden();
    match path.filter(|p| !p.is_empty()) {
        Some(p) => {
            let l = load(p);
            e = e.child(monogram.live_opacity(&l.monogram)).child(rune::el().pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0)).pixels(&l.pixels));
        }
        None => e = e.child(monogram),
    }
    e
}
