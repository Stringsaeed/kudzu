//! Event log shared by Rune's function-pointer hooks and the driver thread.

use std::collections::BTreeSet;
use std::sync::{Condvar, Mutex};
use std::time::{Duration, Instant};

use crate::native::Snap;

struct Inner {
    ready: BTreeSet<&'static str>,
    paint: Vec<(&'static str, u8)>,
    frames: Vec<FrameNote>,
    focus: Vec<FocusNote>,
    clicks: u32,
}

pub struct FrameNote {
    pub name: &'static str,
    pub at: Instant,
    pub outcome: &'static str,
    pub seq: u64,
    pub token: u8,
    pub alpha: f64,
    pub encode_ms: Option<f64>,
}

#[derive(Clone)]
pub struct FocusNote {
    pub name: &'static str,
    pub at: Instant,
    pub focused: bool,
    pub snap: Snap,
}

pub struct Bus {
    inner: Mutex<Inner>,
    cv: Condvar,
}

impl Bus {
    fn new() -> Self {
        Self {
            inner: Mutex::new(Inner {
                ready: BTreeSet::new(),
                paint: Vec::new(),
                frames: Vec::new(),
                focus: Vec::new(),
                clicks: 0,
            }),
            cv: Condvar::new(),
        }
    }

    pub fn mark_ready(&self, name: &'static str, _number: Option<u32>) {
        let mut g = self.inner.lock().expect("bus");
        g.ready.insert(name);
        self.cv.notify_all();
    }

    pub fn wait_ready(&self, timeout: Duration) -> bool {
        let deadline = Instant::now() + timeout;
        let mut g = self.inner.lock().expect("bus");
        loop {
            if ["palette", "picker", "form"]
                .iter()
                .all(|n| g.ready.contains(n))
            {
                return true;
            }
            let now = Instant::now();
            if now >= deadline {
                return false;
            }
            let (next, _) = self.cv.wait_timeout(g, deadline - now).expect("bus");
            g = next;
        }
    }

    pub fn set_paint(&self, name: &'static str, token: u8) {
        let mut g = self.inner.lock().expect("bus");
        if let Some(slot) = g.paint.iter_mut().find(|s| s.0 == name) {
            slot.1 = token;
        } else {
            g.paint.push((name, token));
        }
    }

    pub fn paint(&self, name: &str) -> u8 {
        self.inner
            .lock()
            .expect("bus")
            .paint
            .iter()
            .find(|s| s.0 == name)
            .map(|s| s.1)
            .unwrap_or(0)
    }

    pub fn push_frame(&self, note: FrameNote) {
        let mut g = self.inner.lock().expect("bus");
        g.frames.push(note);
        self.cv.notify_all();
    }

    pub fn push_focus(&self, note: FocusNote) {
        let mut g = self.inner.lock().expect("bus");
        g.focus.push(note);
        self.cv.notify_all();
    }

    pub fn focus_len(&self) -> usize {
        self.inner.lock().expect("bus").focus.len()
    }

    pub fn frame_len(&self) -> usize {
        self.inner.lock().expect("bus").frames.len()
    }

    pub fn wait_focus(&self, name: &str, focused: bool, prev: usize, timeout: Duration) -> bool {
        let deadline = Instant::now() + timeout;
        let mut g = self.inner.lock().expect("bus");
        loop {
            if g.focus
                .iter()
                .skip(prev)
                .any(|e| e.name == name && e.focused == focused)
            {
                return true;
            }
            let now = Instant::now();
            if now >= deadline {
                return false;
            }
            let (next, _) = self.cv.wait_timeout(g, deadline - now).expect("bus");
            g = next;
        }
    }

    pub fn focus_since(&self, name: &str, focused: bool, prev: usize) -> Option<FocusNote> {
        self.inner
            .lock()
            .expect("bus")
            .focus
            .iter()
            .skip(prev)
            .find(|e| e.name == name && e.focused == focused)
            .cloned()
    }

    pub fn frames_since(&self, name: &str, prev: usize, t0: Instant) -> Vec<FrameNote> {
        self.inner
            .lock()
            .expect("bus")
            .frames
            .iter()
            .skip(prev)
            .filter(|f| f.name == name && f.at >= t0)
            .cloned()
            .collect()
    }

    pub fn note_click(&self) {
        let mut g = self.inner.lock().expect("bus");
        g.clicks += 1;
        self.cv.notify_all();
    }

    pub fn clicks(&self) -> u32 {
        self.inner.lock().expect("bus").clicks
    }
}

impl Clone for FrameNote {
    fn clone(&self) -> Self {
        Self {
            name: self.name,
            at: self.at,
            outcome: self.outcome,
            seq: self.seq,
            token: self.token,
            alpha: self.alpha,
            encode_ms: self.encode_ms,
        }
    }
}

pub fn bus() -> &'static Bus {
    use std::sync::OnceLock;
    static BUS: OnceLock<Bus> = OnceLock::new();
    BUS.get_or_init(Bus::new)
}
