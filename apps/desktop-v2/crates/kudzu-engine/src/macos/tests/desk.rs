//! The fake desktop: one target app with a text field, a pasteboard, a clock and the user.
//!
//! The engine reaches it only through the seams (see `fakes.rs`). The user's keys and clicks
//! are real `CGEvent` objects, created and handed to the listener the engine attached, never
//! posted. Whatever is not swallowed reaches the [`Document`], as it would reach the frontmost
//! app. Every event the engine posts is decoded into the [`Ev`] log and applied to the document
//! too, so a test can state both the exact sequence and the text left behind.
//!
//! Scripted actions ([`Desk::at`]) run on the engine's thread at a precise point (after its
//! n-th key press, at the start of a sleep, after it posts a held event). That is how a test
//! types, clicks or changes the system *during* an insertion, deterministically.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, Weak};
use std::time::{Duration, Instant};

use unicode_segmentation::UnicodeSegmentation;

use super::document::Document;
use super::keyboard::{apply_key, user_key, CgEvent, KeyEvent};
use super::panels::{windows, Panels};
use crate::macos::ffi::*;
use crate::macos::keys::SYNTHETIC_MARKER;
use crate::macos::pasteboard::Items;
use crate::macos::tap::Handler;
use crate::{lock, AppInfo, Engine, InputEvent, PermissionStatus, TapMode};

pub(super) const PLAIN: &str = "public.utf8-plain-text";

/// One synthetic or replayed event the engine posted, decoded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Out {
    /// A Unicode key press (typed text, at most 20 UTF-16 units).
    Type(String),
    Backspace,
    Delete,
    Left,
    Right,
    Up,
    Down,
    Return,
    Tab,
    Escape,
    Space,
    /// ⌘V.
    Paste,
    /// ⌘C.
    Copy,
    Other(u16),
    /// A held physical event posted again to be matched like typing.
    Replay(Key),
    /// A held physical event let go unmatched (cancellation, tap loss, shutdown, deadline).
    Release(Key),
}

/// A physical key event, by what it types (`⌫` Backspace, `⏎` Return, `⇥` Tab).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Key {
    pub label: String,
    pub down: bool,
}

/// What happened on the desktop, in order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Ev {
    Out(Out),
    /// The engine slept this long (logged when the sleep ends).
    Slept(Duration),
    Board(BoardOp),
    Listener(ListenerOp),
    /// The engine brought this app (bundle id) forward.
    Activated(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum BoardOp {
    /// The engine wrote its paste item: plain text, and whether an HTML flavour came with it.
    Transient { text: String, html: bool },
    /// Items were written back (a restore or a history pick): the first item's plain text.
    Wrote(Option<String>),
    /// The user copied something.
    UserCopy(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ListenerOp {
    Created(TapMode),
    Enabled(TapMode),
    Stopped(TapMode),
    Refused(TapMode),
}

/// When a scripted action runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum At {
    /// After the engine's n-th synthetic key press (0-based) reached the target.
    Press(usize),
    /// At the start of the engine's n-th sleep (0-based) of exactly this length.
    Sleep(Duration, usize),
    /// After the n-th held event the engine posted again (0-based).
    Held(usize),
    /// While the engine waits for its n-th app activation (0-based), once the app is in front.
    Activate(usize),
    /// After the n-th pasteboard snapshot (0-based), on whichever thread took it.
    Snapshot(usize),
    /// After the n-th transient paste item was written.
    Transient(usize),
    /// Just before the n-th time items are written back (a restore or a history pick).
    Writing(usize),
    /// While the n-th plain-text read of the clipboard waits (rendering `{{clipboard}}`).
    ReadClipboard(usize),
}

type Action = Box<dyn FnOnce(&Desk) + Send>;
pub(super) type Tick = Box<dyn FnMut() -> bool + Send>;

pub(super) struct Slot {
    pub id: usize,
    pub mode: TapMode,
    pub enabled: bool,
    pub handler: Arc<Handler>,
}

/// Everything the fakes read and write, behind one lock.
pub(super) struct State {
    pub doc: Document,
    pub log: Vec<Ev>,
    /// Real time of each synthetic press, for the latency budget.
    pub press_times: Vec<Instant>,
    pub presses: usize,
    pub helds: usize,
    pub activations: usize,
    /// The user's events are stamped 1001, 1002, …; the last one received.
    pub next_stamp: u64,
    pub last_stamp: u64,
    pub last_key: Option<u16>,
    /// The stamp of the last key-down or click received.
    pub last_down: u64,
    /// Pasteboard operations so far: snapshots, transient writes, item writes.
    pub board_ops: [usize; 4],
    pub sleeps: HashMap<Duration, usize>,
    // Pasteboard.
    pub items: Items,
    pub count: isize,
    /// Some promised data can't be read, so a snapshot is impossible.
    pub unreadable: bool,
    pub transient_fails: bool,
    /// ⌘V can't be posted (its event can't be created); other keys still can.
    pub paste_refused: bool,
    /// The press with this index (counting the ones that went out) can't be created, once.
    pub refuse_press: Option<usize>,
    /// That press's key-down (`true`) or key-up half can't be created, once.
    pub refuse_half: Option<(usize, bool)>,
    // Observations.
    pub elapsed: Duration,
    pub frontmost: Option<AppInfo>,
    pub running: Vec<AppInfo>,
    pub activation_works: bool,
    pub permissions: PermissionStatus,
    pub post_event: bool,
    pub secure: (bool, Option<i32>),
    // Listeners.
    pub listeners: Vec<Slot>,
    pub next_listener: usize,
    pub refuse: Vec<TapMode>,
    /// Most listeners ever enabled at once.
    pub peak_enabled: usize,
    // Kudzu's panels.
    pub panels: Panels,
}

pub(super) fn app(bundle_id: &str, name: &str, pid: i32) -> AppInfo {
    AppInfo {
        bundle_id: bundle_id.into(),
        name: name.into(),
        pid: Some(pid),
        path: None,
    }
}

/// Kudzu's own pid: the engine never expands into the app that runs it.
pub(super) fn own_pid() -> i32 {
    std::process::id() as i32
}

/// The target app. Its pid is never ours, whatever pid the test process got.
pub(super) fn editor() -> AppInfo {
    app("com.example.editor", "Editor", own_pid() + 1)
}

pub(super) fn mail() -> AppInfo {
    app("com.example.mail", "Mail", own_pid() + 2)
}

/// The clipboard the user had before any test step: two items, several flavours.
pub(super) fn user_clipboard() -> Items {
    vec![
        vec![
            (PLAIN.into(), b"user text".to_vec()),
            ("public.rtf".into(), b"{\\rtf1 user text}".to_vec()),
        ],
        vec![("public.png".into(), vec![0x89, b'P', b'N', b'G'])],
    ]
}

/// The first item's plain text.
pub(super) fn plain(items: &Items) -> Option<String> {
    items.first().and_then(|item| {
        item.iter()
            .find(|(kind, _)| kind == PLAIN)
            .map(|(_, data)| String::from_utf8_lossy(data).into_owned())
    })
}

struct Inner {
    state: Mutex<State>,
    actions: Mutex<Vec<(At, Action)>>,
    ticks: Mutex<Vec<Tick>>,
    engine: Mutex<Weak<Engine>>,
    base: Instant,
}

/// A handle on the fake desktop, shared by the fakes, the rig and scripted actions.
#[derive(Clone)]
pub(super) struct Desk(Arc<Inner>);

impl State {
    /// Whether the fake session can create this half of press `out` (the next press).
    pub fn can_create(&mut self, out: &Out, down: bool) -> bool {
        if down && self.paste_refused && *out == Out::Paste {
            return false;
        }
        if down && self.refuse_press == Some(self.presses) {
            self.refuse_press = None;
            return false;
        }
        if self.refuse_half == Some((self.presses, down)) {
            self.refuse_half = None;
            return false;
        }
        true
    }
}

impl Desk {
    pub fn new() -> Desk {
        let state = State {
            doc: Document::new(),
            log: Vec::new(),
            press_times: Vec::new(),
            presses: 0,
            helds: 0,
            activations: 0,
            next_stamp: 1_000,
            last_stamp: 1_000,
            last_key: None,
            last_down: 1_000,
            board_ops: [0; 4],
            sleeps: HashMap::new(),
            items: user_clipboard(),
            count: 100,
            unreadable: false,
            transient_fails: false,
            paste_refused: false,
            refuse_press: None,
            refuse_half: None,
            elapsed: Duration::ZERO,
            frontmost: Some(editor()),
            running: vec![editor(), mail()],
            activation_works: true,
            permissions: PermissionStatus {
                accessibility: true,
                input_monitoring: true,
            },
            post_event: true,
            secure: (false, None),
            listeners: Vec::new(),
            next_listener: 0,
            refuse: Vec::new(),
            peak_enabled: 0,
            panels: Panels::default(),
        };
        Desk(Arc::new(Inner {
            state: Mutex::new(state),
            actions: Mutex::new(Vec::new()),
            ticks: Mutex::new(Vec::new()),
            engine: Mutex::new(Weak::new()),
            base: Instant::now(),
        }))
    }

    pub fn attach(&self, engine: &Arc<Engine>) {
        *lock(&self.0.engine) = Arc::downgrade(engine);
    }

    /// The engine under test (for scripted actions).
    pub fn engine(&self) -> Arc<Engine> {
        lock(&self.0.engine).upgrade().expect("engine alive")
    }

    pub fn with<R>(&self, f: impl FnOnce(&mut State) -> R) -> R {
        f(&mut lock(&self.0.state))
    }

    pub fn now(&self) -> Instant {
        self.0.base + self.with(|s| s.elapsed)
    }

    // MARK: Scripting

    /// Runs `action` once, at `at`, on whichever thread reaches that point.
    pub fn at(&self, at: At, action: impl FnOnce(&Desk) + Send + 'static) {
        lock(&self.0.actions).push((at, Box::new(action)));
    }

    /// Runs the actions due at `at`, with no lock held.
    pub fn fire(&self, at: At) {
        let due: Vec<Action> = {
            let mut actions = lock(&self.0.actions);
            let (due, rest): (Vec<_>, Vec<_>) = actions.drain(..).partition(|(a, _)| *a == at);
            *actions = rest;
            due.into_iter().map(|(_, action)| action).collect()
        };
        for action in due {
            action(self);
        }
    }

    /// Scripted actions that never ran.
    pub fn unfired(&self) -> Vec<At> {
        lock(&self.0.actions).iter().map(|(at, _)| *at).collect()
    }

    pub fn add_tick(&self, tick: Tick) {
        lock(&self.0.ticks).push(tick);
    }

    /// One 100 ms poll tick: the hold deadline, and every tenth tick the status refresh.
    pub fn poll_tick(&self) {
        let ticks: Vec<Tick> = lock(&self.0.ticks).drain(..).collect();
        let mut kept = Vec::new();
        for mut tick in ticks {
            if tick() {
                kept.push(tick);
            }
        }
        lock(&self.0.ticks).extend(kept);
    }

    /// Ten poll ticks: exactly one permission, Secure Input and listener refresh.
    pub fn refresh(&self) {
        for _ in 0..10 {
            self.poll_tick();
        }
    }

    // MARK: What the tests read

    pub fn log(&self) -> Vec<Ev> {
        self.with(|s| s.log.clone())
    }

    /// Only the events the engine posted.
    pub fn posted(&self) -> Vec<Out> {
        self.log()
            .into_iter()
            .filter_map(|ev| match ev {
                Ev::Out(out) => Some(out),
                _ => None,
            })
            .collect()
    }

    pub fn listener_log(&self) -> Vec<ListenerOp> {
        self.log()
            .into_iter()
            .filter_map(|ev| match ev {
                Ev::Listener(op) => Some(op),
                _ => None,
            })
            .collect()
    }

    pub fn clear_log(&self) {
        self.with(|s| s.log.clear());
    }

    /// The target's text with `|` at the caret.
    pub fn doc(&self) -> String {
        self.with(|s| s.doc.show())
    }

    pub fn set_doc(&self, shown: &str) {
        self.with(|s| s.doc.set(shown));
    }

    pub fn clipboard(&self) -> Items {
        self.with(|s| s.items.clone())
    }

    // MARK: The system changing

    pub fn advance(&self, by: Duration) {
        self.with(|s| s.elapsed += by);
    }

    pub fn set_frontmost(&self, app: Option<AppInfo>) {
        self.with(|s| s.frontmost = app);
    }

    pub fn grant(&self, accessibility: bool, input_monitoring: bool) {
        self.with(|s| {
            s.permissions = PermissionStatus {
                accessibility,
                input_monitoring,
            };
            s.post_event = accessibility;
        });
    }

    pub fn set_post_event(&self, allowed: bool) {
        self.with(|s| s.post_event = allowed);
    }

    /// Secure Input on (held by `owner`) or off.
    pub fn set_secure_input(&self, on: bool, owner: Option<i32>) {
        self.with(|s| s.secure = (on, owner.filter(|_| on)));
    }

    // MARK: The user

    /// The user copies `text` in some app.
    pub fn user_copies(&self, text: &str) {
        self.with(|s| {
            s.items = vec![vec![(PLAIN.into(), text.as_bytes().to_vec())]];
            s.count += 1;
            s.log.push(Ev::Board(BoardOp::UserCopy(text.into())));
        });
    }

    /// Presses and releases each key of `keys` in the target app, right now.
    pub fn types(&self, keys: &str) {
        for key in keys.graphemes(true) {
            self.key(key, true, true);
            self.key(key, false, true);
        }
    }

    /// The same keys, typed into the form panel: the listener sees them, the document doesn't.
    pub fn types_in_panel(&self, keys: &str) {
        for key in keys.graphemes(true) {
            self.key(key, true, false);
            self.key(key, false, false);
        }
    }

    /// One key event. `to_document`: it goes to the target app unless swallowed.
    pub fn key(&self, key: &str, down: bool, to_document: bool) -> bool {
        let event = self.key_event(key, down);
        let swallowed = self.deliver_key(&event);
        if !swallowed && down && to_document {
            self.with(|s| apply_key(s, event.code, 0, &event.text));
        }
        swallowed
    }

    /// A key event built ahead of time (so a test can time only its delivery).
    pub fn key_event(&self, key: &str, down: bool) -> KeyEvent {
        let (code, text) = user_key(key);
        let event = CgEvent::key(code, &text, down);
        self.stamp(&event);
        KeyEvent {
            event,
            code,
            text,
            down,
        }
    }

    /// Gives a user event the next timestamp: the user's events are stamped in order.
    fn stamp(&self, event: &CgEvent) {
        let stamp = self.with(|s| {
            s.next_stamp += 1;
            s.next_stamp
        });
        unsafe { CGEventSetTimestamp(event.0, stamp) };
    }

    /// The last event the user's apps received: what the form window passes as the boundary of
    /// a focus change or a submission.
    pub fn last_event(&self) -> InputEvent {
        self.with(|s| s.last_event())
    }

    /// Hands `event` to the listeners only. Whether it was swallowed.
    pub fn deliver_key(&self, event: &KeyEvent) -> bool {
        let kind = if event.down {
            kCGEventKeyDown
        } else {
            kCGEventKeyUp
        };
        self.received(event.event.0, Some(event.code), event.down);
        self.deliver(kind, event.event.0)
    }

    /// Notes that the user's event reached the session.
    fn received(&self, event: CGEventRef, key_code: Option<u16>, down: bool) {
        let stamp = unsafe { CGEventGetTimestamp(event) };
        self.with(|s| {
            s.last_stamp = stamp;
            s.last_key = key_code;
            if down {
                s.last_down = stamp;
            }
        });
    }

    /// A click in one of Kudzu's panels: it doesn't touch the target's document.
    pub fn clicks(&self) {
        self.click_in(windows::PANEL);
    }

    /// A click in the target's text that puts the caret before grapheme `index`.
    pub fn clicks_text(&self, index: usize) {
        self.click_in(windows::TARGET);
        self.with(|s| s.doc.place_caret(index));
    }

    /// A mouse-down on `window` (see [`windows`]), handed to the listeners.
    pub fn click_in(&self, window: i64) {
        self.click_event(CgEvent::click(window));
    }

    /// A mouse-down whose two window fields differ (see [`CgEvent::click_fields`]).
    pub fn click_with_fields(&self, under: i64, handles: i64) {
        self.click_event(CgEvent::click_fields(under, handles));
    }

    /// A mouse-down on `window` that the session says goes to process `pid`
    /// (`kCGEventTargetUnixProcessID`).
    pub fn click_routed_to(&self, window: i64, pid: i32) {
        let event = CgEvent::click(window);
        unsafe {
            CGEventSetIntegerValueField(event.0, kCGEventTargetUnixProcessID, i64::from(pid))
        };
        self.click_event(event);
    }

    fn click_event(&self, event: CgEvent) {
        self.stamp(&event);
        self.received(event.0, None, true);
        self.deliver(kCGEventLeftMouseDown, event.0);
    }

    /// A synthetic press reaching the session: the listener sees its down and up, marked as
    /// ours, as it would on macOS. Whether a listener swallowed either (it never should).
    pub fn synthetic(&self, code: CGKeyCode, flags: CGEventFlags, text: &str, down: bool) -> bool {
        let event = CgEvent::key(code, text, down);
        unsafe {
            CGEventSetFlags(event.0, flags);
            CGEventSetIntegerValueField(event.0, kCGEventSourceUserData, SYNTHETIC_MARKER);
        }
        let kind = if down { kCGEventKeyDown } else { kCGEventKeyUp };
        self.deliver(kind, event.0)
    }

    /// macOS disables the tap because a callback took too long.
    pub fn tap_times_out(&self) {
        let event = CgEvent::click(0);
        self.deliver(kCGEventTapDisabledByTimeout, event.0);
    }

    /// Hands an event to every enabled listener, as the session would. Whether an active
    /// listener swallowed it.
    pub fn deliver(&self, kind: CGEventType, event: CGEventRef) -> bool {
        let handlers: Vec<(TapMode, Arc<Handler>)> = self.with(|s| {
            s.listeners
                .iter()
                .filter(|l| l.enabled)
                .map(|l| (l.mode, l.handler.clone()))
                .collect()
        });
        let mut swallowed = false;
        for (mode, handler) in handlers {
            swallowed |= handler(kind, event) && mode == TapMode::Active;
        }
        swallowed
    }
}
