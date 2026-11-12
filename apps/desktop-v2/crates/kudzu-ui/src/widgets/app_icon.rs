//! An app's icon, or a monogram when the catalogue has none.
//!
//! Bytes come from [`kudzu_app::apps::Apps::icon`], which reads the catalogue on a worker and
//! caches them. [`IconCache`] decodes those bytes on Rune's image workers. Nothing here knows
//! which screen asked.

use std::collections::{HashMap, VecDeque};

use kudzu_app::apps::Apps;
use kudzu_app::tasks::Task;
use kudzu_app::watch::Subscription;
use rune::{el, text, El, Image, ImageCache, TextStyle};

use crate::theme::Theme;

const MONOGRAM_RADIUS: f32 = 5.0;

enum Slot {
    Loading {
        task: Task<Option<Vec<u8>>>,
        _wake: Subscription,
    },
    Image {
        image: Image,
        announced: bool,
    },
    Missing,
}

/// PNG bytes in flight or decoded, keyed by bundle id and point size.
pub struct IconCache {
    images: ImageCache,
    wake: rune::Waker,
    slots: HashMap<(String, u32), Slot>,
    queue: VecDeque<(String, u32)>,
    inflight: Option<(String, u32)>,
}

impl IconCache {
    pub fn new(wake: rune::Waker) -> Self {
        Self {
            images: ImageCache::new(wake.clone()),
            wake,
            slots: HashMap::new(),
            queue: VecDeque::new(),
            inflight: None,
        }
    }

    /// Queue a catalogue read when this id and size are not already cached or queued.
    /// At most one read runs. Returns without waiting for the icon. A later `request`
    /// starts the next queued read once the current one has finished.
    pub fn request(&mut self, apps: &Apps, bundle_id: &str, size: u32) -> bool {
        let key = (bundle_id.to_owned(), size);
        let fresh = self.inflight.as_ref() != Some(&key)
            && !self.slots.contains_key(&key)
            && !self.queue.iter().any(|queued| queued == &key);
        if fresh {
            self.queue.push_back(key);
        }
        self.pump(apps);
        fresh
    }

    fn pump(&mut self, apps: &Apps) {
        if self.inflight.is_some() {
            return;
        }
        while let Some(key) = self.queue.pop_front() {
            if self.slots.contains_key(&key) {
                continue;
            }
            match apps.icon(&key.0, key.1) {
                Ok(task) => {
                    let wake = self.wake.clone();
                    let ready = task.ready();
                    let sub = ready.subscribe(move || wake());
                    if *ready.get() {
                        (self.wake)();
                    }
                    self.slots
                        .insert(key.clone(), Slot::Loading { task, _wake: sub });
                    self.inflight = Some(key);
                    return;
                }
                Err(_) => {
                    self.slots.insert(key, Slot::Missing);
                }
            }
        }
    }

    /// Take finished reads and hand them to the image decoder. True when a slot changed.
    pub fn poll(&mut self) -> bool {
        let mut changed = false;
        let finished: Vec<_> = self
            .slots
            .iter()
            .filter_map(|(key, slot)| match slot {
                Slot::Loading { task, .. } => task.take().map(|result| (key.clone(), result)),
                _ => None,
            })
            .collect();
        for (key, result) in finished {
            changed = true;
            let slot = match result {
                Ok(Some(bytes)) => Slot::Image {
                    image: self.images.load(bytes),
                    announced: false,
                },
                _ => Slot::Missing,
            };
            self.slots.insert(key, slot);
        }
        let done = self
            .inflight
            .as_ref()
            .is_some_and(|key| !matches!(self.slots.get(key), Some(Slot::Loading { .. })));
        if done {
            self.inflight = None;
        }
        for slot in self.slots.values_mut() {
            if let Slot::Image { image, announced } = slot {
                if !*announced && image.is_ready() {
                    *announced = true;
                    changed = true;
                }
            }
        }
        changed
    }

    pub fn image(&self, bundle_id: &str, size: u32) -> Option<&Image> {
        match self.slots.get(&(bundle_id.to_owned(), size))? {
            Slot::Image { image, .. } if image.is_ready() && image.error().is_none() => Some(image),
            _ => None,
        }
    }
}

/// First letter of `name`, uppercased, or empty. Callers pass the display name, which is
/// the bundle id when the app is missing.
pub fn monogram(name: &str) -> String {
    name.chars()
        .next()
        .map(|c| c.to_uppercase().to_string())
        .unwrap_or_default()
}

/// `AppIcon::new("Mail").size(16.0).image(cache.image(id, 16))`.
pub struct AppIcon<'a> {
    name: String,
    size: f32,
    image: Option<&'a Image>,
}

impl<'a> AppIcon<'a> {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            size: 18.0,
            image: None,
        }
    }

    pub fn size(mut self, size: f32) -> Self {
        self.size = size;
        self
    }

    pub fn image(mut self, image: Option<&'a Image>) -> Self {
        self.image = image;
        self
    }

    pub fn build(self, theme: &Theme) -> El {
        if let Some(image) = self.image {
            return el()
                .key("app-icon")
                .size(self.size, self.size)
                .pixels(image.pixels());
        }
        let letter = monogram(&self.name);
        el().key("app-icon-mono")
            .size(self.size, self.size)
            .radius(MONOGRAM_RADIUS)
            .fill(theme.c.foreground.alpha(0.08))
            .center()
            .child(
                text(letter)
                    .font(TextStyle::new(self.size * 0.5, 600.0).lh(self.size))
                    .color(theme.c.muted_foreground)
                    .a11y_hidden(),
            )
    }
}

/// A 1×1 PNG. Tests decode it; the UI never invents icon bytes.
pub const PNG_1X1: &[u8] = &[
    0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x02, 0x00, 0x00, 0x00, 0x90, 0x77, 0x53,
    0xDE, 0x00, 0x00, 0x00, 0x0C, 0x49, 0x44, 0x41, 0x54, 0x08, 0xD7, 0x63, 0x60, 0x60, 0x60, 0x00,
    0x00, 0x00, 0x04, 0x00, 0x01, 0x27, 0x34, 0x27, 0x0A, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E,
    0x44, 0xAE, 0x42, 0x60, 0x82,
];

#[cfg(test)]
mod tests {
    use super::*;
    use kudzu_app::ports::AppCatalog;
    use kudzu_core::system::AppInfo;
    use rune::{Env, Runtime};
    use std::sync::mpsc;
    use std::sync::Arc;
    use std::sync::Mutex;
    use std::time::Duration;

    struct Gate {
        release: Mutex<mpsc::Receiver<()>>,
        entered: mpsc::Sender<()>,
    }

    impl AppCatalog for Gate {
        fn installed(&self) -> Result<Vec<AppInfo>, kudzu_app::error::PortError> {
            Ok(Vec::new())
        }

        fn icon(
            &self,
            _bundle_id: &str,
            _size: u32,
        ) -> Result<Option<Vec<u8>>, kudzu_app::error::PortError> {
            self.entered.send(()).unwrap();
            self.release.lock().unwrap().recv().unwrap();
            Ok(Some(PNG_1X1.to_vec()))
        }
    }

    #[test]
    fn a_cache_miss_returns_before_the_catalogue_answers() {
        let (release_tx, release_rx) = mpsc::channel();
        let (entered_tx, entered_rx) = mpsc::channel();
        let apps = Apps::new(Arc::new(Gate {
            release: Mutex::new(release_rx),
            entered: entered_tx,
        }));
        let mut cache = IconCache::new(Arc::new(|| {}));
        assert!(cache.request(&apps, "com.example.blocked", 18));
        entered_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        assert!(cache.image("com.example.blocked", 18).is_none());
        release_tx.send(()).unwrap();
        for _ in 0..50 {
            if let Some(Slot::Image { image, .. }) =
                cache.slots.get(&("com.example.blocked".into(), 18))
            {
                image.wait().unwrap();
            }
            cache.poll();
            if cache.image("com.example.blocked", 18).is_some() {
                return;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        panic!("icon never decoded");
    }

    #[test]
    fn a_second_request_does_not_start_another_read() {
        let (release_tx, release_rx) = mpsc::channel();
        let (entered_tx, entered_rx) = mpsc::channel();
        let apps = Apps::new(Arc::new(Gate {
            release: Mutex::new(release_rx),
            entered: entered_tx,
        }));
        let mut cache = IconCache::new(Arc::new(|| {}));
        assert!(cache.request(&apps, "com.example.once", 16));
        assert!(!cache.request(&apps, "com.example.once", 16));
        entered_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        release_tx.send(()).unwrap();
    }

    #[test]
    fn one_icon_read_runs_at_a_time() {
        let (release_tx, release_rx) = mpsc::channel();
        let (entered_tx, entered_rx) = mpsc::channel();
        let apps = Apps::new(Arc::new(Gate {
            release: Mutex::new(release_rx),
            entered: entered_tx,
        }));
        let mut cache = IconCache::new(Arc::new(|| {}));
        assert!(cache.request(&apps, "com.example.one", 64));
        entered_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        assert!(cache.request(&apps, "com.example.two", 64));
        assert!(entered_rx.try_recv().is_err());
        release_tx.send(()).unwrap();
        for _ in 0..50 {
            cache.poll();
            if cache.inflight.is_none() {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(cache.inflight.is_none());
        assert!(!cache.request(&apps, "com.example.two", 64));
        entered_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        release_tx.send(()).unwrap();
    }

    #[test]
    fn a_missing_icon_is_a_monogram_tile() {
        let mut rt = Runtime::new(
            (),
            |_, _| {
                // The window root is stretched to the frame, so the tile is a child.
                rune::row()
                    .items_start()
                    .child(AppIcon::new("mail").size(16.0).build(Theme::light()))
            },
            (40.0, 40.0),
            Env::new(true, 1.0),
        );
        rt.frame(0.0);
        let tile = rt
            .elements()
            .into_iter()
            .find(|e| e.key.as_deref() == Some("app-icon-mono"))
            .unwrap();
        assert_eq!((tile.rect[2], tile.rect[3], tile.radius), (16.0, 16.0, 5.0));
        let letter = rt
            .elements()
            .into_iter()
            .find(|e| e.text.as_deref() == Some("M"))
            .unwrap();
        assert_eq!(letter.text_color, Some(Theme::light().c.muted_foreground));
    }
}
