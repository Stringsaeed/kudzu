//! Off-thread loading for previews: the complete text of a clip and its images. Nothing here
//! blocks the UI: each request is a worker `Task`, its answer is taken in `poll`, and images
//! decode on Rune's image worker, which wakes the window when one is ready.

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;

use kudzu_app::apps::Apps;
use kudzu_app::clipboard::{ClipboardError, ClipboardService, MediaKind};
use kudzu_app::tasks::Task;
use kudzu_app::watch::Subscription;
use rune::{Image, ImageCache, Pixels};
use uuid::Uuid;

/// Decoded images kept at once.
const KEEP: usize = 96;

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

enum Slot {
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
}

impl Media {
    /// `wake` brings the window's `poll` and view back when something finished.
    pub fn new(wake: Arc<dyn Fn() + Send + Sync>) -> Self {
        Self {
            cache: ImageCache::new(wake.clone()),
            wake,
            slots: HashMap::new(),
            order: VecDeque::new(),
        }
    }

    /// Starts loading a picture unless it is loaded or on its way.
    pub fn want(&mut self, service: &Arc<ClipboardService>, id: Uuid, want: Want) {
        let service = service.clone();
        self.start(Key::Clip(id, want), move || {
            service.media(id, want.kind()).map(|m| m.bytes)
        });
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

    fn start(&mut self, key: Key, work: impl FnOnce() -> Option<Vec<u8>> + Send + 'static) {
        if self.slots.contains_key(&key) {
            return;
        }
        let slot = match Task::spawn("kudzu-clip-media", work) {
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
        while self.order.len() > KEEP {
            if let Some(old) = self.order.pop_front() {
                self.slots.remove(&old);
            }
        }
    }

    /// Takes finished reads and hands their bytes to the image worker. True when any finished.
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
        changed
    }

    /// The decoded picture, once it is ready.
    pub fn pixels(&self, id: Uuid, want: Want) -> Option<Pixels> {
        self.ready(&Key::Clip(id, want))
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

    /// Nothing is reading or decoding: every requested picture has settled.
    pub fn idle(&self) -> bool {
        self.slots.values().all(|slot| match slot {
            Slot::Loading { .. } => false,
            Slot::Ready { image, .. } => image.is_ready() || image.error().is_some(),
            Slot::Missing => true,
        })
    }

    /// The picture is loaded or loading (so a fallback should wait rather than flash).
    pub fn pending(&self, id: Uuid, want: Want) -> bool {
        match self.slots.get(&Key::Clip(id, want)) {
            Some(Slot::Loading { .. }) => true,
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
