//! Off-thread loading for previews: the complete text of a clip and its images. Nothing here
//! blocks the UI: each request is a worker `Task`, its answer is taken in `poll`, and images
//! decode on Rune's image worker, which wakes the window when one is ready.

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;

use kudzu_app::apps::Apps;
use kudzu_app::clipboard::{ClipboardError, ClipboardService, MediaKind};
use kudzu_app::tasks::Task;
use kudzu_app::watch::Subscription;
use rune::{Image, ImageCache, Pixels};
use uuid::Uuid;

/// Decoded images kept at once. Pictures the open clip or the rows in view need are never
/// evicted, whatever this is.
const KEEP: usize = 512;
/// Reads running at once. A folder of 200 files queues 200 icons; they load four at a time.
pub const MAX_WORKERS: usize = 4;

/// Which picture of a clip.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Want {
    Thumbnail,
    Image,
    /// The file at this index, when it is an image.
    File(usize),
    /// Its Finder icon.
    Icon(usize),
}

impl Want {
    fn kind(self) -> MediaKind {
        match self {
            Self::Thumbnail => MediaKind::Thumbnail,
            Self::Image => MediaKind::Image,
            Self::File(i) => MediaKind::File(i),
            Self::Icon(i) => MediaKind::Icon(i),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum Key {
    Clip(Uuid, Want),
    App(String),
}

type Read = Box<dyn FnOnce() -> Option<Vec<u8>> + Send>;

enum Slot {
    /// Waiting for one of [`MAX_WORKERS`] workers.
    Queued(Read),
    Loading {
        task: Task<Option<Vec<u8>>>,
        _wake: Subscription,
    },
    /// Decoding on the image worker (`false`) or done and drawn at least once (`true`).
    Ready { image: Image, announced: bool },
    /// No such picture (an icon the system could not draw): callers draw their fallback.
    Missing,
}

/// Loaded pictures: decoded handles by clip and kind. Debug prints counts only.
pub struct Media {
    cache: ImageCache,
    wake: Arc<dyn Fn() + Send + Sync>,
    slots: HashMap<Key, Slot>,
    order: VecDeque<Key>,
    queue: VecDeque<Key>,
    /// What the open clip shows, and what the rows in view show: neither is evicted.
    open: HashSet<Key>,
    rows: HashSet<Key>,
}

impl Media {
    /// `wake` brings the window's `poll` and view back when something finished.
    pub fn new(wake: Arc<dyn Fn() + Send + Sync>) -> Self {
        Self {
            cache: ImageCache::new(wake.clone()),
            wake,
            slots: HashMap::new(),
            order: VecDeque::new(),
            queue: VecDeque::new(),
            open: HashSet::new(),
            rows: HashSet::new(),
        }
    }

    /// Asks for a picture unless it is loaded or on its way. It starts when a worker is free.
    pub fn want(&mut self, service: &Arc<ClipboardService>, id: Uuid, want: Want) {
        let key = Key::Clip(id, want);
        if self.slots.contains_key(&key) {
            return;
        }
        let service = service.clone();
        let read: Read = Box::new(move || service.media(id, want.kind()).map(|m| m.bytes));
        self.insert(key.clone(), Slot::Queued(read));
        self.queue.push_back(key);
        self.pump();
    }

    /// The pictures the open clip shows: kept until the clip changes.
    pub fn need_open(&mut self, wants: impl IntoIterator<Item = (Uuid, Want)>) {
        self.open = wants.into_iter().map(|(id, w)| Key::Clip(id, w)).collect();
    }

    /// The pictures the rows in view show: kept until the view moves.
    pub fn need_rows(&mut self, wants: impl IntoIterator<Item = (Uuid, Want)>) {
        self.rows = wants.into_iter().map(|(id, w)| Key::Clip(id, w)).collect();
    }

    /// Starts loading an app's icon (the "From" row and the row badge).
    pub fn want_app(&mut self, apps: &Arc<Apps>, bundle_id: &str) {
        let key = Key::App(bundle_id.to_owned());
        if self.slots.contains_key(&key) {
            return;
        }
        let slot = match apps.icon(bundle_id, 32) {
            Ok(task) => {
                let wake = self.wake.clone();
                let sub = task.ready().subscribe(move || wake());
                Slot::Loading { task, _wake: sub }
            }
            Err(_) => Slot::Missing,
        };
        self.insert(key, slot);
    }

    fn insert(&mut self, key: Key, slot: Slot) {
        self.slots.insert(key.clone(), slot);
        self.order.push_back(key);
        // Make room by dropping the oldest finished pictures nobody shows. A queued or
        // loading one is never dropped: its answer would be lost and asked for again.
        let mut at = 0;
        while self.slots.len() > KEEP && at < self.order.len() {
            let old = &self.order[at];
            let keep = self.open.contains(old)
                || self.rows.contains(old)
                || matches!(
                    self.slots.get(old),
                    Some(Slot::Queued(_) | Slot::Loading { .. }) | None
                );
            if keep {
                at += 1;
            } else if let Some(old) = self.order.remove(at) {
                self.slots.remove(&old);
            }
        }
    }

    /// Reads running now.
    pub fn in_flight(&self) -> usize {
        self.slots
            .values()
            .filter(|s| matches!(s, Slot::Loading { .. }))
            .count()
    }

    /// Reads waiting for a worker.
    pub fn queued(&self) -> usize {
        self.slots
            .values()
            .filter(|s| matches!(s, Slot::Queued(_)))
            .count()
    }

    /// Starts queued reads while fewer than [`MAX_WORKERS`] run.
    fn pump(&mut self) {
        let mut running = self.in_flight();
        while running < MAX_WORKERS {
            let Some(key) = self.queue.pop_front() else {
                break;
            };
            let Some(Slot::Queued(read)) = self.slots.remove(&key) else {
                continue; // evicted or answered meanwhile
            };
            let slot = match Task::spawn("kudzu-clip-media", read) {
                Ok(task) => {
                    let wake = self.wake.clone();
                    let sub = task.ready().subscribe(move || wake());
                    running += 1;
                    Slot::Loading { task, _wake: sub }
                }
                Err(_) => Slot::Missing,
            };
            self.slots.insert(key, slot);
        }
    }

    /// Takes finished reads and hands their bytes to the image worker, then starts the next
    /// queued reads. True when anything finished.
    pub fn poll(&mut self) -> bool {
        let mut changed = false;
        for slot in self.slots.values_mut() {
            if let Slot::Ready { image, announced } = slot {
                // The decode finished on the image worker: the view draws it from now on.
                if !*announced && (image.is_ready() || image.error().is_some()) {
                    *announced = true;
                    changed = true;
                }
                continue;
            }
            let Slot::Loading { task, .. } = slot else {
                continue;
            };
            let Some(result) = task.take() else { continue };
            *slot = match result {
                Ok(Some(bytes)) => Slot::Ready {
                    image: self.cache.load(bytes),
                    announced: false,
                },
                _ => Slot::Missing,
            };
            changed = true;
        }
        self.pump();
        changed
    }

    /// The decoded picture, once it is ready.
    pub fn pixels(&self, id: Uuid, want: Want) -> Option<Pixels> {
        self.ready(&Key::Clip(id, want))
    }

    /// The first of `wants` that is ready: the original, then its thumbnail, then nothing.
    pub fn first_ready(&self, id: Uuid, wants: &[Want]) -> Option<Pixels> {
        wants.iter().find_map(|w| self.pixels(id, *w))
    }

    /// An app's icon, once it is ready.
    pub fn app_pixels(&self, bundle_id: &str) -> Option<Pixels> {
        self.ready(&Key::App(bundle_id.to_owned()))
    }

    fn ready(&self, key: &Key) -> Option<Pixels> {
        match self.slots.get(key)? {
            Slot::Ready { image, .. } if image.is_ready() && image.error().is_none() => {
                Some(image.pixels().clone())
            }
            _ => None,
        }
    }

    /// Nothing is queued, reading or decoding: every requested picture has settled.
    pub fn idle(&self) -> bool {
        self.slots.values().all(|slot| match slot {
            Slot::Queued(_) | Slot::Loading { .. } => false,
            Slot::Ready { image, .. } => image.is_ready() || image.error().is_some(),
            Slot::Missing => true,
        })
    }

    /// The picture is queued, loading or decoding (so a fallback should wait, not flash).
    pub fn pending(&self, id: Uuid, want: Want) -> bool {
        match self.slots.get(&Key::Clip(id, want)) {
            Some(Slot::Queued(_) | Slot::Loading { .. }) => true,
            Some(Slot::Ready { image, .. }) => !image.is_ready(),
            _ => false,
        }
    }

    /// The picture does not exist or could not be read.
    pub fn missing(&self, id: Uuid, want: Want) -> bool {
        match self.slots.get(&Key::Clip(id, want)) {
            Some(Slot::Missing) => true,
            Some(Slot::Ready { image, .. }) => image.error().is_some(),
            _ => false,
        }
    }
}

/// The picture a row shows in place of a glyph: an image's thumbnail, a file's image thumbnail
/// or its Finder icon.
pub fn row_want(e: &kudzu_core::clipboard::ClipEntry) -> Option<Want> {
    use kudzu_core::clipboard::ClipKind;
    match e.kind {
        ClipKind::Image => Some(Want::Thumbnail),
        ClipKind::File if e.image.is_some() => Some(Want::Thumbnail),
        ClipKind::File => Some(Want::Icon(0)),
        _ => None,
    }
}

/// The pictures a preview of `e` shows: the image and its thumbnail, a file list's icons (at
/// most [`FILE_LIMIT`](super::FILE_LIMIT)) and a single image file's picture.
pub fn open_wants(e: &kudzu_core::clipboard::ClipEntry) -> Vec<(Uuid, Want)> {
    use kudzu_core::clipboard::ClipKind;
    match e.kind {
        ClipKind::Image => vec![(e.id, Want::Image), (e.id, Want::Thumbnail)],
        ClipKind::File => {
            let mut wants: Vec<(Uuid, Want)> = (0..e.files.len().min(super::FILE_LIMIT))
                .map(|i| (e.id, Want::Icon(i)))
                .collect();
            if e.image.is_some() && e.files.len() == 1 {
                wants.push((e.id, Want::File(0)));
                wants.push((e.id, Want::Thumbnail));
            }
            wants
        }
        _ => Vec::new(),
    }
}

type TextLoad = (Task<Result<Option<String>, ClipboardError>>, Subscription);

/// The complete text of one clip, loaded once per `(id, hash)`.
#[derive(Default)]
pub struct FullText {
    key: Option<(Uuid, String)>,
    text: Option<Arc<str>>,
    task: Option<TextLoad>,
}

impl FullText {
    /// Points at a clip. The excerpt is the whole text when it is not cut; otherwise a worker
    /// reads the blob. Returns true when the loaded text changed.
    pub fn show(
        &mut self,
        service: &Arc<ClipboardService>,
        wake: &Arc<dyn Fn() + Send + Sync>,
        e: &kudzu_core::clipboard::ClipEntry,
    ) -> bool {
        let key = (e.id, e.hash.clone());
        if self.key.as_ref() == Some(&key) {
            return false;
        }
        self.key = Some(key);
        self.task = None;
        let complete = e
            .text
            .as_deref()
            .filter(|t| e.text_len <= t.chars().count());
        self.text = complete.map(Arc::from);
        if self.text.is_some() || e.text.is_none() && e.text_len == 0 {
            return true;
        }
        let (service, id) = (service.clone(), e.id);
        if let Ok(task) = Task::spawn("kudzu-clip-full-text", move || service.text(id)) {
            let wake = wake.clone();
            let sub = task.ready().subscribe(move || wake());
            self.task = Some((task, sub));
        }
        true
    }

    /// Takes the worker's answer. True when it arrived.
    pub fn poll(&mut self) -> bool {
        let Some((task, _)) = &self.task else {
            return false;
        };
        let Some(result) = task.take() else {
            return false;
        };
        self.task = None;
        if let Ok(Ok(Some(text))) = result {
            self.text = Some(Arc::from(text));
        }
        true
    }

    /// The text, once complete.
    pub fn get(&self) -> Option<&str> {
        self.text.as_deref()
    }

    /// The worker has not answered yet.
    pub fn loading(&self) -> bool {
        self.task.is_some()
    }
}

impl std::fmt::Debug for Media {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Media")
            .field("pictures", &self.slots.len())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn with_nothing_loaded_there_is_no_picture_to_fall_back_to() {
        let media = Media::new(Arc::new(|| {}));
        let id = Uuid::new_v4();
        assert!(media
            .first_ready(id, &[Want::File(0), Want::Thumbnail])
            .is_none());
        assert!(media.idle());
        assert_eq!((media.in_flight(), media.queued()), (0, 0));
    }
}
