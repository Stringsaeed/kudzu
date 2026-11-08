//! Clipboard media for the Rune windows, in place of the web's `kzclip://` protocol: thumbnails
//! (`thumbs/<id>.png`), an item's own image (from its blob, any format), a copied image file, and
//! Finder icons of copied files. Each is read and decoded on a worker and lands in a [`Pixels`]
//! with live opacities, so the view never waits on a file and never re-runs when an image
//! arrives: a fallback shows until then, and stays if the image can't load (`ClipImage`'s
//! `onError`).
//!
//! Decoded images stay in a bounded LRU per kind (thumbnails and icons are small and many; full
//! images are large and few), so a 5,000-item history scrolls without holding every picture.
//! Long texts (`loadFullText`) are cached the same way and wake the windows when they land.

use std::collections::{HashMap, VecDeque};
use std::sync::mpsc::{channel, Sender};
use std::sync::{Arc, Mutex, OnceLock};

use kudzu_core::clipboard::ClipEntry;
use rune::{Live, Pixels};
use uuid::Uuid;

use crate::backend::core::core;

/// Longer side of a decoded full-size preview, in pixels (sharp at 2× in the large preview).
const FULL_PX: u32 = 1800;
/// Bytes of a copied image file read for a preview (the Tauri protocol's limit).
const MAX_FILE_BYTES: u64 = 80 * 1024 * 1024;
/// Finder icons render at this many pixels (32 pt at 2×).
const ICON_PX: u32 = 64;

/// What to load.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Source {
    /// The stored thumbnail of an image (or image file) entry.
    Thumb(Uuid),
    /// The entry's own image, from its blob.
    Image(Uuid),
    /// A copied file, as an image.
    File(String),
    /// A copied file's Finder icon.
    Icon(String),
}

impl Source {
    fn budget(&self) -> usize {
        match self {
            Source::Thumb(_) | Source::Icon(_) => 240,
            Source::Image(_) | Source::File(_) => 6,
        }
    }
    fn class(&self) -> u8 {
        match self {
            Source::Thumb(_) | Source::Icon(_) => 0,
            Source::Image(_) | Source::File(_) => 1,
        }
    }
}

/// A picture as the view draws it.
#[derive(Clone)]
pub struct Picture {
    pub pixels: Pixels,
    /// 0 until the picture decodes, then 1.
    pub shown: Live<f32>,
    /// 1 until the picture decodes, then 0: the fallback's opacity.
    pub fallback: Live<f32>,
    /// 1 when the decoded picture has transparent pixels (the checkerboard shows behind it).
    pub transparent: Live<f32>,
}

impl Picture {
    fn new() -> Picture {
        Picture { pixels: Pixels::new(), shown: Live::new(0.0), fallback: Live::new(1.0), transparent: Live::new(0.0) }
    }
}

#[derive(Default)]
struct Cache {
    map: HashMap<Source, Picture>,
    /// Use order per class, least recent first.
    order: [VecDeque<Source>; 2],
}

fn cache() -> &'static Mutex<Cache> {
    static CACHE: OnceLock<Mutex<Cache>> = OnceLock::new();
    CACHE.get_or_init(Default::default)
}

fn worker() -> &'static Mutex<Sender<(Source, Picture)>> {
    static WORKER: OnceLock<Mutex<Sender<(Source, Picture)>>> = OnceLock::new();
    WORKER.get_or_init(|| {
        let (tx, rx) = channel::<(Source, Picture)>();
        let _ = std::thread::Builder::new().name("kudzu-clip-media".into()).spawn(move || {
            while let Ok((source, picture)) = rx.recv() {
                if let Some((w, h, rgba)) = decode(&source) {
                    let transparent = rgba.as_chunks::<4>().0.iter().any(|p| p[3] < 255);
                    picture.pixels.set(w, h, rgba);
                    picture.transparent.set(if transparent { 1.0 } else { 0.0 });
                    picture.shown.set(1.0);
                    picture.fallback.set(0.0);
                }
            }
        });
        Mutex::new(tx)
    })
}

/// The picture for `source`, loading it the first time it's asked for (and again after the LRU
/// let it go).
pub fn picture(source: Source) -> Picture {
    let mut c = cache().lock().unwrap_or_else(|e| e.into_inner());
    let class = source.class() as usize;
    if let Some(p) = c.map.get(&source).cloned() {
        let order = &mut c.order[class];
        if let Some(i) = order.iter().position(|s| s == &source) {
            order.remove(i);
        }
        order.push_back(source);
        return p;
    }
    let p = Picture::new();
    c.map.insert(source.clone(), p.clone());
    c.order[class].push_back(source.clone());
    while c.order[class].len() > source.budget() {
        if let Some(old) = c.order[class].pop_front() {
            c.map.remove(&old);
        }
    }
    drop(c);
    let _ = worker().lock().unwrap_or_else(|e| e.into_inner()).send((source, p.clone()));
    p
}

/// Reads and decodes a source into straight RGBA.
fn decode(source: &Source) -> Option<(u32, u32, Vec<u8>)> {
    let png = match source {
        Source::Thumb(id) => {
            let path = core().clip.lock().thumb_path(*id);
            std::fs::read(path).ok()?
        }
        Source::Image(id) => {
            let items = core().clip.lock().items(*id)?;
            let (data, mime) = kudzu_core::clipboard::image_of(&items)?;
            shrink(data, mime == "image/png")?
        }
        Source::File(path) => {
            let meta = std::fs::metadata(path).ok().filter(|m| m.is_file() && m.len() <= MAX_FILE_BYTES)?;
            let _ = meta;
            let data = std::fs::read(path).ok()?;
            let png = kudzu_core::clipboard::image_mime_for_path(path) == Some("image/png");
            shrink(&data, png)?
        }
        Source::Icon(path) => kudzu_engine::system::app_icon_png(path, ICON_PX)?,
    };
    let img = image::load_from_memory(&png).ok()?.to_rgba8();
    let (w, h) = img.dimensions();
    Some((w, h, img.into_raw()))
}

/// Any image format as a PNG no longer than [`FULL_PX`] (ImageIO through the engine), or the
/// PNG itself where the engine can't.
fn shrink(data: &[u8], png: bool) -> Option<Vec<u8>> {
    match kudzu_engine::clipboard::thumbnail(data, FULL_PX) {
        Some(t) => Some(t.png),
        None if png => Some(data.to_vec()),
        None => None,
    }
}

// MARK: Full text

type Wake = Box<dyn Fn() + Send + Sync>;

fn wakers() -> &'static Mutex<Vec<Wake>> {
    static WAKERS: OnceLock<Mutex<Vec<Wake>>> = OnceLock::new();
    WAKERS.get_or_init(Default::default)
}

/// Runs `wake` (re-running a window's view) whenever a long text lands.
pub fn on_loaded(wake: impl Fn() + Send + Sync + 'static) {
    wakers().lock().unwrap_or_else(|e| e.into_inner()).push(Box::new(wake));
}

type TextKey = (Uuid, String);
type Texts = (HashMap<TextKey, Option<Arc<String>>>, VecDeque<TextKey>);

fn texts() -> &'static Mutex<Texts> {
    static TEXTS: OnceLock<Mutex<Texts>> = OnceLock::new();
    TEXTS.get_or_init(Default::default)
}

/// The full text of an entry (`useFullText`): the excerpt when it's whole, else the blob's text,
/// fetched once per id and content. `None` while it loads.
pub fn full_text(e: &ClipEntry) -> Option<Arc<String>> {
    if let Some(t) = kudzu_core::client::clipboard::inline_full_text(e) {
        return Some(Arc::new(t.to_string()));
    }
    let key = (e.id, e.hash.clone());
    let mut guard = texts().lock().unwrap_or_else(|e| e.into_inner());
    if let Some(v) = guard.0.get(&key) {
        return v.clone();
    }
    guard.0.insert(key.clone(), None);
    guard.1.push_back(key.clone());
    while guard.1.len() > 16 {
        if let Some(old) = guard.1.pop_front() {
            guard.0.remove(&old);
        }
    }
    drop(guard);
    let fallback = e.text.clone().unwrap_or_default();
    let _ = std::thread::Builder::new().name("kudzu-clip-text".into()).spawn(move || {
        let text = crate::backend::clipboard::clipboard_text(key.0).unwrap_or(fallback);
        texts().lock().unwrap_or_else(|e| e.into_inner()).0.insert(key, Some(Arc::new(text)));
        for wake in wakers().lock().unwrap_or_else(|e| e.into_inner()).iter() {
            wake();
        }
    });
    None
}
