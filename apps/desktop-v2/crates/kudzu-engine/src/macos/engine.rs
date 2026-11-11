//! The macOS engine runtime: tap handling, insertion worker, status poll.
//!
//! Threads:
//! - **tap** (`kudzu-event-tap`): runs the CGEventTap callback. Matching happens here, under the
//!   `core` mutex, because the callback must decide synchronously whether to swallow the key.
//! - **worker** (`kudzu-engine-worker`): renders and inserts. Everything slow (sleeps, paste,
//!   app activation, host callbacks for forms/expansions) happens here.
//! - **poll** (`kudzu-engine-poll`): permissions, Secure Input and tap attachment every second;
//!   enforces the 1 s hold deadline every 100 ms.
//!
//! Host callbacks are never invoked while `core` is locked.
//!
//! The platform is reached through three seams (`super::Platform`): the event stream (posting
//! and the listener), the pasteboard, and observations (clock, frontmost app, permissions,
//! caret). The protocol tests in `super::tests` drive this coordinator through fakes of them.

use std::collections::HashMap;
#[cfg(test)]
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicU8, Ordering};
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex, Weak};
use std::thread;
use std::time::{Duration, Instant};

use kudzu_core::matching::{
    case_propagation, ExpansionMatcher, KeyInput, MatcherEvent, TriggerIndex, TriggerMatch,
};
use kudzu_core::template::RenderResult;
use kudzu_core::{InsertionMethod, SnippetRecord};
use objc2::rc::autoreleasepool;
use uuid::Uuid;

use super::events::{Events, Listener};
use super::ffi::{
    kCGEventKeyDown, kCGEventKeyUp, kCGEventTapDisabledByTimeout, kCGEventTapDisabledByUserInput,
    CGEventRef, CGEventType,
};
use super::input_source;
use super::keys::{self, KeyError, REPLAY_MARKER, SYNTHETIC_MARKER};
use super::observe::Observe;
use super::pasteboard::{PasteError, Pasteboard};
use super::tap::{self, HeldEvent};
use super::Platform;
use crate::activity::Recorder;
use crate::hold::{HoldReplayQueue, HoldState};
use crate::input::{self, Follow, KeyContext};
use crate::library::{self, Library};
use crate::plan::{self, InsertionPlan, PlanMode, Sequence, Step};
use crate::status::{self, StatusInputs};
use crate::{
    lock, AppInfo, EngineHost, EngineLibrary, EngineStatus, ExpansionEvent, FormRequest, TapMode,
};

mod activity;

/// Physical input is never held longer than this, whatever the target app does.
const HOLD_LIMIT: Duration = Duration::from_secs(1);
/// How long to wait for a reactivated app to become frontmost.
const ACTIVATION_TIMEOUT: Duration = Duration::from_millis(500);
/// Lets focus changes and flushed input reach the target before we act on it.
const SETTLE: Duration = Duration::from_millis(50);
/// Listen-only: let the final trigger key reach the document before deleting it.
const LISTEN_ONLY_SETTLE: Duration = Duration::from_millis(30);
/// After a `{{key}}` press: lets the target move focus (Tab) or react (Return) before the next step.
const KEY_SETTLE: Duration = Duration::from_millis(30);
/// A `{{delay}}` sleeps in slices this long, so a click or pause can stop the sequence promptly.
const PAUSE_SLICE: Duration = Duration::from_millis(50);
const POLL_TICK: Duration = Duration::from_millis(100);
const STATUS_EVERY_TICKS: u32 = 10;

/// Which guard an insertion re-checks between steps.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Guard {
    /// A typed trigger: blocked in our own windows and while suppressed.
    Typed,
    /// Something the user explicitly asked for (form submit, palette).
    Explicit,
}

#[derive(Debug, Clone)]
enum FormKind {
    /// Typed trigger, still in the document (as typed, with any separator).
    Typed(TriggerMatch),
    /// Palette insert: nothing to delete.
    Palette,
}

#[derive(Debug, Clone)]
struct PendingForm {
    token: u64,
    snippet_id: Uuid,
    kind: FormKind,
    target: Option<AppInfo>,
    /// Typing reached the target after the trigger; deleting it now would delete the wrong text.
    interrupted: bool,
    /// The form panel is key, so keys and clicks go to it rather than the target (FRM-4).
    panel_focused: bool,
    /// Selection captured before a palette insert (never set for a typed trigger).
    selection: Option<String>,
}

/// What a sequence replaces: a typed trigger, or nothing (palette).
struct TypedTrigger<'a> {
    trigger: &'a TriggerMatch,
    mode: PlanMode,
    swallowed: Option<&'a str>,
}

struct Matched {
    generation: u64,
    event: MatcherEvent,
    mode: PlanMode,
    undo_caret: usize,
    swallowed: Option<String>,
    pid: Option<i32>,
}

enum Job {
    Matched(Matched),
    Submit {
        token: u64,
        values: HashMap<String, String>,
    },
    Insert {
        snippet_id: Uuid,
        target: Option<AppInfo>,
        selection: Option<String>,
    },
    Replay,
    /// Something the tap callback saw, for the activity log.
    Note(activity::Note),
}

/// State the tap callback touches. Keep every operation on it short.
struct Core {
    matcher: ExpansionMatcher,
    library: Arc<Library>,
    held: HoldReplayQueue<HeldEvent>,
    hold_since: Option<Instant>,
    /// Extra time the current step of a key sequence may hold input (a `{{delay}}`).
    hold_extra: Duration,
    /// Bumped whenever in-flight work must stop (click, pause, deadline, new insertion).
    generation: u64,
    /// An insertion is in flight.
    busy: bool,
    listen_only_interrupted: bool,
    preparing_form: bool,
    /// A key reached the target while the form was being prepared.
    preparing_interrupted: bool,
    pending_form: Option<PendingForm>,
    next_form_token: u64,
    /// Left-arrow presses after the last expansion's cursor placement (undo moves back first).
    undo_caret_left: usize,
    frontmost_pid: Option<i32>,
    frontmost_bundle: Option<String>,
    /// A pending dead key or held key, which changes what the next key-down means.
    keys: KeyContext,
    /// The input source epoch the match buffer was typed under.
    input_epoch: u64,
}

impl Core {
    fn reset_matcher(&mut self) {
        self.matcher.reset();
        self.undo_caret_left = 0;
    }

    fn start_hold(&mut self, now: Instant) {
        if self.held.state() != HoldState::Inserting {
            self.hold_since = Some(now);
        }
        self.hold_extra = Duration::ZERO;
        self.held.begin();
    }

    /// Stops in-flight work and returns held input, to be posted unmatched.
    fn invalidate(&mut self) -> Vec<HeldEvent> {
        self.generation += 1;
        self.busy = false;
        self.preparing_form = false;
        self.hold_since = None;
        self.reset_matcher();
        self.keys.clear();
        self.held.drain()
    }

    /// Next held event to replay once nothing is in flight.
    fn next_replay(&mut self) -> Option<HeldEvent> {
        if self.busy {
            return None;
        }
        let next = self.held.next();
        if next.is_none() {
            self.hold_since = None;
        }
        next
    }

    fn hold_expired(&self, now: Instant) -> bool {
        self.held.state() == HoldState::Inserting
            && self
                .hold_since
                .is_some_and(|t| now.saturating_duration_since(t) > HOLD_LIMIT + self.hold_extra)
    }

    /// FRM-4 (D9): a key-down or click while the form panel is not key reaches the target app,
    /// which changes the document the typed trigger sits in. Suppression doesn't matter here: it
    /// stops matching, not this.
    fn note_form_input(&mut self) {
        if let Some(form) = self.pending_form.as_mut() {
            if !form.panel_focused {
                form.interrupted = true;
            }
        }
    }
}

#[derive(Default)]
struct Flags {
    running: AtomicBool,
    paused: AtomicBool,
    suppressed: AtomicBool,
    secure_input: AtomicBool,
    post_event: AtomicBool,
    input_monitoring: AtomicBool,
    tap_mode: AtomicU8,
    poll_epoch: AtomicU64,
}

fn encode(mode: TapMode) -> u8 {
    match mode {
        TapMode::Off => 0,
        TapMode::ListenOnly => 1,
        TapMode::Active => 2,
    }
}

fn decode(raw: u8) -> TapMode {
    match raw {
        1 => TapMode::ListenOnly,
        2 => TapMode::Active,
        _ => TapMode::Off,
    }
}

/// Effects of a tap callback, performed after `core` is unlocked.
#[derive(Default)]
struct Effects {
    post_unmatched: Vec<HeldEvent>,
    job: Option<Job>,
    replay: bool,
    note: Option<activity::Note>,
}

/// Why an insertion step failed.
#[derive(Debug)]
enum InsertError {
    Key(KeyError),
    Paste(PasteError),
}

impl std::fmt::Display for InsertError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InsertError::Key(e) => write!(f, "keystroke: {e:?}"),
            InsertError::Paste(e) => write!(f, "paste: {e:?}"),
        }
    }
}

impl From<KeyError> for InsertError {
    fn from(e: KeyError) -> Self {
        InsertError::Key(e)
    }
}

impl From<PasteError> for InsertError {
    fn from(e: PasteError) -> Self {
        InsertError::Paste(e)
    }
}

/// The macOS engine.
pub(crate) struct Imp {
    /// The host, reached through `recorder` so expansions and forms are noted on the way.
    host: Arc<dyn EngineHost>,
    recorder: Arc<Recorder>,
    weak: Weak<Imp>,
    platform: Platform,
    core: Mutex<Core>,
    flags: Flags,
    tap: Mutex<Option<Box<dyn Listener>>>,
    jobs: Mutex<Option<Sender<Job>>>,
    last_status: Mutex<Option<EngineStatus>>,
    own_pid: i32,
    /// Jobs sent and not yet finished, so tests can wait until the worker is idle.
    #[cfg(test)]
    in_flight: AtomicUsize,
}

impl Imp {
    pub fn new(host: Arc<dyn EngineHost>, platform: Platform) -> Arc<Imp> {
        Arc::new_cyclic(|weak: &Weak<Imp>| {
            let (sender, receiver) = mpsc::channel::<Job>();
            let worker = weak.clone();
            let spawned = thread::Builder::new()
                .name("kudzu-engine-worker".into())
                .spawn(move || {
                    for job in receiver {
                        let Some(imp) = worker.upgrade() else { break };
                        autoreleasepool(|_| imp.run(job));
                        #[cfg(test)]
                        imp.in_flight.fetch_sub(1, Ordering::SeqCst);
                    }
                });
            if let Err(error) = &spawned {
                log::error!("engine worker failed to start: {error}");
            }
            let flags = Flags::default();
            let observe = &platform.observe;
            flags
                .post_event
                .store(observe.post_event_allowed(), Ordering::Relaxed);
            flags
                .secure_input
                .store(observe.secure_input().0, Ordering::Relaxed);
            flags
                .input_monitoring
                .store(observe.permissions().input_monitoring, Ordering::Relaxed);
            input_source::install();
            let recorder = Arc::new(Recorder::new(host));
            Imp {
                host: recorder.clone(),
                recorder,
                weak: weak.clone(),
                platform,
                core: Mutex::new(Core {
                    matcher: ExpansionMatcher::new(),
                    library: Arc::new(Library::default()),
                    held: HoldReplayQueue::default(),
                    hold_since: None,
                    hold_extra: Duration::ZERO,
                    generation: 0,
                    busy: false,
                    listen_only_interrupted: false,
                    preparing_form: false,
                    preparing_interrupted: false,
                    pending_form: None,
                    next_form_token: 1,
                    undo_caret_left: 0,
                    frontmost_pid: None,
                    frontmost_bundle: None,
                    keys: KeyContext::default(),
                    input_epoch: input_source::epoch(),
                }),
                flags,
                tap: Mutex::new(None),
                jobs: Mutex::new(spawned.ok().map(|_| sender)),
                last_status: Mutex::new(None),
                own_pid: std::process::id() as i32,
                #[cfg(test)]
                in_flight: AtomicUsize::new(0),
            }
        })
    }

    fn events(&self) -> &dyn Events {
        self.platform.events.as_ref()
    }

    fn observe(&self) -> &dyn Observe {
        self.platform.observe.as_ref()
    }

    /// The pasteboard this engine pastes through (the process-wide one outside tests).
    fn pasteboard(&self) -> &Pasteboard {
        &self.platform.pasteboard
    }

    /// Plain-text clipboard, for rendering `{{clipboard}}` outside an insertion.
    pub fn clipboard_text(&self) -> Option<String> {
        self.pasteboard().read_string()
    }

    /// Whether every job sent to the worker has finished.
    #[cfg(test)]
    pub fn idle(&self) -> bool {
        self.in_flight.load(Ordering::SeqCst) == 0
    }

    // MARK: - Public surface

    pub fn start(&self) {
        if self.flags.running.swap(true, Ordering::SeqCst) {
            return;
        }
        autoreleasepool(|_| self.refresh());
        let epoch = self.flags.poll_epoch.fetch_add(1, Ordering::SeqCst) + 1;
        let weak = self.weak.clone();
        let mut tick: u32 = 0;
        let poll = move || {
            let Some(imp) = weak.upgrade() else {
                return false;
            };
            if imp.flags.poll_epoch.load(Ordering::SeqCst) != epoch
                || !imp.flags.running.load(Ordering::SeqCst)
            {
                return false;
            }
            tick = tick.wrapping_add(1);
            if tick.is_multiple_of(STATUS_EVERY_TICKS) {
                autoreleasepool(|_| imp.refresh());
            }
            imp.enforce_hold_deadline();
            true
        };
        let spawned = self
            .observe()
            .every("kudzu-engine-poll", POLL_TICK, Box::new(poll));
        if let Err(error) = spawned {
            log::error!("engine poll failed to start: {error}");
        }
    }

    pub fn stop(&self) {
        if self.flags.running.swap(false, Ordering::SeqCst) {
            self.recorder
                .health(kudzu_core::activity::ActivityOutcome::EngineStopped);
        }
        self.flags.poll_epoch.fetch_add(1, Ordering::SeqCst);
        let old = lock(&self.tap).take();
        drop(old);
        self.flags
            .tap_mode
            .store(encode(TapMode::Off), Ordering::SeqCst);
        self.invalidate();
        self.pasteboard().restore_pending();
        self.publish_status();
    }

    /// Stops everything and lets the worker exit (engine dropped).
    pub fn shutdown(&self) {
        self.stop();
        lock(&self.jobs).take();
    }

    pub fn reload(&self, library: EngineLibrary) {
        let new = Library::new(&library, super::system::locale);
        let current = lock(&self.core).library.clone();
        // Usage saves after every expansion can trigger reloads. Rebuilding would clear the live
        // key buffer and undo, dropping whatever was typed since, so skip no-op reloads.
        if *current == new {
            return;
        }
        // Built here, not on the tap thread.
        let index = TriggerIndex::new(&library.snippets, &library.groups, new.config.clone());
        let mut core = lock(&self.core);
        core.matcher.replace_index(index);
        core.library = Arc::new(new);
        core.undo_caret_left = 0;
    }

    pub fn library(&self) -> Arc<Library> {
        lock(&self.core).library.clone()
    }

    pub fn set_paused(&self, paused: bool) {
        self.flags.paused.store(paused, Ordering::SeqCst);
        self.invalidate();
        self.publish_status();
    }

    pub fn set_suppressed(&self, suppressed: bool) {
        self.flags.suppressed.store(suppressed, Ordering::SeqCst);
        lock(&self.core).reset_matcher();
    }

    pub fn status(&self) -> EngineStatus {
        self.current_status()
    }

    pub fn submit_form(&self, token: u64, values: HashMap<String, String>) {
        self.send(Job::Submit { token, values });
    }

    /// The form panel for `token` became key (`true`) or stopped being key (`false`). While it
    /// is not key, a key-down or click reaches the target app and interrupts a typed form.
    pub fn form_panel_focused(&self, token: u64, focused: bool) {
        let mut core = lock(&self.core);
        if let Some(form) = core.pending_form.as_mut().filter(|f| f.token == token) {
            form.panel_focused = focused;
        }
    }

    pub fn cancel_form(&self, token: u64) {
        {
            let mut core = lock(&self.core);
            if core.pending_form.as_ref().is_some_and(|f| f.token == token) {
                core.pending_form = None;
            }
        }
        self.recorder.form_cancelled(token);
    }

    pub fn insert_snippet(
        &self,
        snippet_id: Uuid,
        target: Option<AppInfo>,
        selection: Option<String>,
    ) {
        self.send(Job::Insert {
            snippet_id,
            target,
            selection,
        });
    }

    pub fn host(&self) -> &dyn EngineHost {
        self.host.as_ref()
    }

    // MARK: - Status

    fn current_status(&self) -> EngineStatus {
        let observe = self.observe();
        let permissions = observe.permissions();
        let (secure_input, owner_pid) = observe.secure_input();
        status::build(StatusInputs {
            paused: self.flags.paused.load(Ordering::SeqCst),
            tap: decode(self.flags.tap_mode.load(Ordering::SeqCst)),
            permissions,
            post_event: observe.post_event_allowed(),
            secure_input,
            secure_input_owner: owner_pid.and_then(|pid| observe.name_of(pid)),
        })
    }

    fn publish_status(&self) {
        let status = autoreleasepool(|_| self.current_status());
        {
            let mut last = lock(&self.last_status);
            if last.as_ref() == Some(&status) {
                return;
            }
            *last = Some(status.clone());
        }
        self.host.status_changed(status);
    }

    /// Permission/Secure Input poll; attaches or upgrades the tap.
    fn refresh(&self) {
        // The change notification normally keeps this current; re-reading is a cheap backstop.
        input_source::request_refresh();
        let permissions = self.observe().permissions();
        let post_event = self.observe().post_event_allowed();
        let secure = self.observe().secure_input().0;
        let was_secure = self.flags.secure_input.swap(secure, Ordering::SeqCst);
        let could_post = self.flags.post_event.swap(post_event, Ordering::SeqCst);
        let could_listen = self
            .flags
            .input_monitoring
            .swap(permissions.input_monitoring, Ordering::SeqCst);
        if was_secure != secure
            || (could_post && !post_event)
            || (could_listen && !permissions.input_monitoring)
        {
            self.invalidate();
        }
        if self.flags.running.load(Ordering::SeqCst) {
            self.attach_tap(permissions.accessibility, permissions.input_monitoring);
        }
        self.publish_status();
    }

    fn tap_handler(&self, mode: TapMode) -> tap::Handler {
        let weak = self.weak.clone();
        Box::new(move |kind, event| {
            weak.upgrade()
                .is_some_and(|imp| imp.receive(kind, event, mode))
        })
    }

    fn attach_tap(&self, accessibility: bool, input_monitoring: bool) {
        let desired = if accessibility {
            TapMode::Active
        } else {
            TapMode::ListenOnly
        };
        let mut slot = lock(&self.tap);
        let current = slot.as_ref().map(|tap| tap.mode());
        if !accessibility && !input_monitoring {
            if slot.take().is_some() {
                drop(slot);
                self.flags
                    .tap_mode
                    .store(encode(TapMode::Off), Ordering::SeqCst);
                self.invalidate();
                self.recorder
                    .health(kudzu_core::activity::ActivityOutcome::ListenerLost);
            }
            return;
        }
        if current == Some(desired) {
            return;
        }
        let mut created = self.events().listen(desired, self.tap_handler(desired));
        if created.is_none() && current.is_none() && desired == TapMode::Active && input_monitoring
        {
            created = self
                .events()
                .listen(TapMode::ListenOnly, self.tap_handler(TapMode::ListenOnly));
        }
        let Some(new) = created else { return };
        // Stop the old tap before enabling the new one so no key is matched twice.
        slot.take();
        new.enable();
        self.flags
            .tap_mode
            .store(encode(new.mode()), Ordering::SeqCst);
        *slot = Some(new);
        drop(slot);
        self.invalidate();
    }

    // MARK: - Guards

    fn explicit_allowed(&self) -> bool {
        !self.flags.paused.load(Ordering::SeqCst)
            && !self.flags.secure_input.load(Ordering::SeqCst)
            && self.flags.post_event.load(Ordering::SeqCst)
    }

    /// Typed triggers additionally never fire in our own windows (palette, form, editor).
    fn typed_allowed(&self) -> bool {
        self.explicit_allowed() && !self.flags.suppressed.load(Ordering::SeqCst)
    }

    fn allowed(&self, guard: Guard) -> bool {
        match guard {
            Guard::Typed => {
                self.typed_allowed() && self.observe().frontmost_pid() != Some(self.own_pid)
            }
            Guard::Explicit => self.explicit_allowed(),
        }
    }

    fn is_current(&self, generation: u64) -> bool {
        lock(&self.core).generation == generation
    }

    // MARK: - Tap callback

    fn receive(&self, kind: CGEventType, event: CGEventRef, mode: TapMode) -> bool {
        let mut effects = Effects::default();
        let swallow = {
            let mut core = lock(&self.core);
            self.receive_locked(&mut core, kind, event, mode, &mut effects)
        };
        for held in effects.post_unmatched {
            self.events().post_held(held, SYNTHETIC_MARKER);
        }
        if let Some(job) = effects.job {
            self.send(job);
        }
        if effects.replay {
            self.send(Job::Replay);
        }
        if let Some(note) = effects.note {
            self.send(Job::Note(note));
        }
        swallow
    }

    fn receive_locked(
        &self,
        core: &mut Core,
        kind: CGEventType,
        event: CGEventRef,
        mode: TapMode,
        effects: &mut Effects,
    ) -> bool {
        if kind == kCGEventTapDisabledByTimeout || kind == kCGEventTapDisabledByUserInput {
            effects.post_unmatched = core.invalidate();
            effects.note = activity::Note::tap_disabled(kind);
            return false;
        }
        let marker = tap::marker(event);
        if marker == SYNTHETIC_MARKER {
            return false;
        }
        if kind != kCGEventKeyDown && kind != kCGEventKeyUp {
            core.note_form_input();
            // A click moves the caret or focus: the typed context is gone. Clicks in our own
            // focused windows (form, palette) don't affect the target document.
            if !self.flags.suppressed.load(Ordering::SeqCst) {
                effects.post_unmatched = core.invalidate();
            }
            return false;
        }
        let replay = marker == REPLAY_MARKER;
        let key_down = kind == kCGEventKeyDown;
        if core.hold_expired(self.observe().now()) {
            effects.post_unmatched = core.invalidate();
        }
        // Holding comes first: typing during an insertion is replayed after it rather than
        // landing inside the text we're about to delete or insert.
        if !replay && mode == TapMode::Active && core.held.state() == HoldState::Inserting {
            if let Some(copy) = HeldEvent::copy(event, key_down) {
                if core.held.enqueue(copy).is_ok() {
                    return true;
                }
            }
        }
        if core.preparing_form {
            core.preparing_interrupted |= key_down;
            effects.replay = replay;
            return false;
        }
        // Typing that reaches another app while a form is open changes the document the
        // trigger sits in. Typing into the form panel itself doesn't. (The engine this was copied
        // from skipped this check while suppressed, which is the whole time a form is open; D9.)
        if key_down {
            core.note_form_input();
        }
        // Listen-only can't withhold keys. Anything typed before our backspaces go out would
        // land inside the deletion range, so the pending insertion is abandoned instead.
        if !replay && mode == TapMode::ListenOnly && core.busy {
            if key_down {
                core.listen_only_interrupted = true;
            }
            return false;
        }
        if !self.typed_allowed() {
            effects.note = self.observe_blocked(core, key_down, event);
            effects.replay = replay;
            return false;
        }
        if !key_down {
            effects.replay = replay;
            return false;
        }
        // Text typed under another input source says nothing about what comes next.
        let epoch = input_source::epoch();
        if epoch != core.input_epoch {
            core.input_epoch = epoch;
            core.reset_matcher();
            core.keys.clear();
        }
        // An input method is composing: the keys go to it, not to the document. Matching them
        // would fire inside a composition and delete composed text.
        if input_source::composing() {
            core.reset_matcher();
            core.keys.clear();
            effects.replay = replay;
            return false;
        }
        // A held key either repeats or opens the accent popup; neither is typed context.
        if tap::is_autorepeat(event) {
            core.reset_matcher();
            if matches!(tap::classify(event), Some(KeyInput::Character(text)) if input::can_open_accent_popup(&text))
            {
                core.keys.repeated();
            }
            effects.replay = replay;
            return false;
        }
        let Some(input) = tap::classify(event) else {
            if input::could_be_dead_key(tap::key_code(event)) {
                core.keys.silent();
            }
            effects.replay = replay;
            return false;
        };
        let follow = core.keys.next(matches!(input, KeyInput::Character(_)));
        let input = if follow == Follow::Broken {
            KeyInput::Reset
        } else {
            input
        };
        let pid = self.observe().frontmost_pid();
        if pid != core.frontmost_pid {
            core.frontmost_pid = pid;
            core.frontmost_bundle = pid.and_then(|pid| self.observe().bundle_id_of(pid));
            core.reset_matcher();
        }
        if pid == Some(self.own_pid) {
            core.reset_matcher();
            effects.replay = replay;
            return false;
        }
        let swallowed = match &input {
            KeyInput::Character(text) => Some(text.clone()),
            _ => None,
        };
        let undo_caret = std::mem::take(&mut core.undo_caret_left);
        let bundle = core.frontmost_bundle.clone();
        let Some(event) = core.matcher.handle(input, bundle.as_deref()) else {
            effects.note = activity::Note::skipped(core.matcher.skipped(bundle.as_deref()), pid);
            effects.replay = replay;
            return false;
        };
        if core.busy {
            core.reset_matcher();
            effects.note = activity::Note::dropped(&event, pid);
            return false;
        }
        core.busy = true;
        core.listen_only_interrupted = false;
        if mode == TapMode::Active {
            core.start_hold(self.observe().now());
        }
        core.generation += 1;
        // The app commits a dead-key accent only when it sees the key that completes it, so that
        // key goes through and the whole trigger is deleted afterwards, as in listen-only mode.
        let withhold = mode == TapMode::Active && follow != Follow::Composed;
        let plan_mode = if withhold {
            PlanMode::Active
        } else {
            PlanMode::ListenOnly
        };
        effects.job = Some(Job::Matched(Matched {
            generation: core.generation,
            event,
            mode: plan_mode,
            undo_caret,
            swallowed,
            pid,
        }));
        withhold
    }

    fn enforce_hold_deadline(&self) {
        let now = self.observe().now();
        let expired = {
            let mut core = lock(&self.core);
            if core.hold_expired(now) {
                core.invalidate()
            } else {
                Vec::new()
            }
        };
        for held in expired {
            self.events().post_held(held, SYNTHETIC_MARKER);
        }
    }

    fn invalidate(&self) {
        let drained = lock(&self.core).invalidate();
        // Never leave physical input trapped after focus, permission or pause changes.
        for held in drained {
            self.events().post_held(held, SYNTHETIC_MARKER);
        }
    }

    fn send(&self, job: Job) {
        if let Some(sender) = lock(&self.jobs).as_ref() {
            #[cfg(test)]
            self.in_flight.fetch_add(1, Ordering::SeqCst);
            if sender.send(job).is_err() {
                #[cfg(test)]
                self.in_flight.fetch_sub(1, Ordering::SeqCst);
            }
        }
    }

    // MARK: - Worker

    fn run(&self, job: Job) {
        match job {
            Job::Matched(matched) => {
                let generation = matched.generation;
                self.recorder.begin(self.typed_subject(&matched));
                if let Err(error) = self.perform_matched(matched) {
                    log::warn!("expansion failed: {error}");
                    self.recorder.resolve(activity::failed(&error));
                    let mut core = lock(&self.core);
                    if core.generation == generation {
                        core.reset_matcher();
                    }
                }
                self.recorder.finish(|| self.unresolved());
                self.finish(generation);
            }
            Job::Submit { token, values } => {
                self.recorder.begin_form(token);
                self.submit(token, values);
                self.recorder.finish(|| self.unresolved());
            }
            Job::Insert {
                snippet_id,
                target,
                selection,
            } => {
                self.recorder
                    .begin(self.palette_subject(snippet_id, target.as_ref()));
                self.palette_insert(snippet_id, target, selection);
                self.recorder.finish(|| self.unresolved());
            }
            Job::Note(note) => self.emit(note),
            Job::Replay => {
                let next = lock(&self.core).next_replay();
                if let Some(held) = next {
                    // One event at a time: its callback can start another expansion before we
                    // release the rest (including triggers typed during a paste).
                    self.events().post_held(held, REPLAY_MARKER);
                }
            }
        }
    }

    /// Ends the insertion `generation` (if still current) and resumes replay.
    fn finish(&self, generation: u64) {
        let next = {
            let mut core = lock(&self.core);
            if core.generation != generation {
                return;
            }
            core.busy = false;
            core.next_replay()
        };
        if let Some(held) = next {
            self.events().post_held(held, REPLAY_MARKER);
        }
    }

    /// Starts an explicit insertion: holds typing (active tap) and returns its generation.
    fn begin_explicit(&self) -> u64 {
        let active = decode(self.flags.tap_mode.load(Ordering::SeqCst)) == TapMode::Active;
        let now = self.observe().now();
        let mut core = lock(&self.core);
        core.generation += 1;
        core.busy = true;
        core.listen_only_interrupted = false;
        core.reset_matcher();
        if active {
            core.start_hold(now);
        }
        core.generation
    }

    fn perform_matched(&self, m: Matched) -> Result<(), InsertError> {
        let token = m.generation;
        if m.mode == PlanMode::ListenOnly {
            // Listen-only can't hold input: let the final key reach the app before deleting.
            self.observe().sleep(LISTEN_ONLY_SETTLE);
        }
        if !self.is_current(token)
            || !self.allowed(Guard::Typed)
            || self.observe().frontmost_pid() != m.pid
        {
            return Ok(());
        }
        if m.mode == PlanMode::ListenOnly && lock(&self.core).listen_only_interrupted {
            lock(&self.core).reset_matcher();
            return Ok(());
        }
        match m.event {
            MatcherEvent::Expand(trigger) => {
                let library = self.library();
                let Some(snippet) = library.snippets.get(&trigger.snippet_id) else {
                    return Ok(());
                };
                let fields = library.form_fields(snippet);
                if !fields.is_empty() {
                    return self.request_typed_form(
                        token,
                        trigger,
                        snippet,
                        fields,
                        m.mode,
                        m.swallowed,
                    );
                }
                // Typing the trigger replaced whatever was selected, so there is no selection.
                let rendered = self.render(&library, snippet, HashMap::new(), None);
                let swallowed = if m.mode == PlanMode::Active {
                    m.swallowed.as_deref()
                } else {
                    None
                };
                self.insert_expansion(token, &trigger, rendered, m.mode, swallowed, Guard::Typed)
            }
            MatcherEvent::Undo(undo) => {
                let events = self.events();
                keys::move_right(events, m.undo_caret)?;
                // A listen-only backspace already reached the document.
                let already = usize::from(m.mode == PlanMode::ListenOnly);
                keys::delete_backward(events, undo.delete_count.saturating_sub(already))?;
                keys::retype(events, &undo.retype)?;
                self.recorder.undone();
                Ok(())
            }
        }
    }

    fn render(
        &self,
        library: &Library,
        snippet: &SnippetRecord,
        values: HashMap<String, String>,
        selection: Option<String>,
    ) -> RenderResult {
        // Read before any paste of ours replaces the clipboard, and only when the template needs it.
        let clipboard = if library.uses_clipboard(snippet) {
            self.pasteboard().read_string()
        } else {
            None
        };
        let history =
            |index: usize, current: Option<&str>| self.host.clipboard_history(index, current);
        library.render(
            snippet,
            library::RenderInputs {
                now: library::now(),
                clipboard,
                selection,
                form_values: values,
                history: &history,
            },
        )
    }

    /// Lets the step that starts now hold typing for the usual limit plus `extra` (a `{{delay}}`).
    /// Without this a sequence of several pastes or pauses would hit the hold deadline midway.
    fn keep_holding(&self, token: u64, extra: Duration) {
        let mut core = lock(&self.core);
        if core.generation == token && core.hold_since.is_some() {
            core.hold_since = Some(self.observe().now());
            core.hold_extra = extra;
        }
    }

    /// Inserts an expansion that has `{{key}}` or `{{delay}}` steps: text, key presses and pauses in
    /// template order. The rules (see docs/TEMPLATE_SYNTAX.md, "Key presses and pauses"):
    /// - every text step is typed or pasted by the same rule as a whole expansion, and a paste's
    ///   clipboard is restored before the next step;
    /// - the sequence stops, leaving what was already inserted, as soon as the user clicks, the
    ///   frontmost app changes, or expansion is paused or blocked;
    /// - with key presses there is no Backspace undo and a whole-word trigger's separator isn't
    ///   typed again, because the text may be spread over several fields by then.
    fn insert_sequence(
        &self,
        token: u64,
        guard: Guard,
        typed: Option<TypedTrigger<'_>>,
        method: InsertionMethod,
        sequence: Sequence,
    ) -> Result<bool, InsertError> {
        if let Some(t) = &typed {
            let plan = InsertionPlan::new(t.trigger, String::new(), None, t.mode, t.swallowed);
            keys::delete_backward(self.events(), plan.delete_count)?;
        }
        let pid = self.observe().frontmost_pid();
        let live = |imp: &Imp| {
            imp.is_current(token) && imp.allowed(guard) && imp.observe().frontmost_pid() == pid
        };
        for step in &sequence.steps {
            if !live(self) {
                lock(&self.core).reset_matcher();
                return Ok(false);
            }
            match step {
                Step::Text { text, spans } => {
                    self.keep_holding(token, Duration::ZERO);
                    let html = spans
                        .as_ref()
                        .map(|spans| kudzu_core::formatting::html(text, spans));
                    self.deliver(text, html.as_deref(), method)?;
                }
                Step::Key { key, count } => {
                    self.keep_holding(token, Duration::ZERO);
                    keys::press_key(self.events(), *key, *count)?;
                    self.observe().sleep(KEY_SETTLE);
                }
                Step::Pause(duration) => {
                    self.keep_holding(token, *duration);
                    let until = self.observe().now() + *duration;
                    while let Some(left) = until
                        .checked_duration_since(self.observe().now())
                        .filter(|d| !d.is_zero())
                    {
                        self.observe().sleep(left.min(PAUSE_SLICE));
                        if !self.is_current(token) {
                            break;
                        }
                    }
                }
            }
        }
        self.keep_holding(token, Duration::ZERO);
        if !live(self) {
            lock(&self.core).reset_matcher();
            return Ok(false);
        }
        // An image's delivered length depends on the app, so the caret isn't moved by a guess.
        let mut caret_left = if sequence.has_images {
            0
        } else {
            sequence.caret_left
        };
        let separator = typed
            .as_ref()
            .filter(|_| !sequence.has_keys)
            .and_then(|t| t.trigger.separator.as_deref());
        if let Some(separator) = separator {
            keys::retype(self.events(), separator)?;
            if caret_left > 0 {
                caret_left += plan::grapheme_count(separator);
            }
        }
        keys::move_left(self.events(), caret_left)?;
        let mut core = lock(&self.core);
        if core.generation == token {
            match &typed {
                // Only pauses: the text is in one piece, so Backspace can still undo it.
                Some(t) if !sequence.has_keys && !sequence.has_images => {
                    let inserted: usize = sequence
                        .steps
                        .iter()
                        .map(|s| {
                            if let Step::Text { text, .. } = s {
                                plan::grapheme_count(text)
                            } else {
                                0
                            }
                        })
                        .sum();
                    core.matcher.did_expand(t.trigger, inserted);
                    core.undo_caret_left = caret_left;
                }
                _ => core.reset_matcher(),
            }
        }
        Ok(true)
    }

    /// Delivers text by typing or pasting.
    fn deliver(
        &self,
        text: &str,
        html: Option<&str>,
        method: InsertionMethod,
    ) -> Result<(), InsertError> {
        if text.is_empty() {
            return Ok(());
        }
        if plan::types(method, text, html.is_some(), input_source::composing()) {
            keys::type_text(self.events(), text)?;
        } else {
            let clock = self.observe();
            self.pasteboard()
                .paste_text(self.events(), clock, text, html)?;
        }
        Ok(())
    }

    fn insert_expansion(
        &self,
        token: u64,
        trigger: &TriggerMatch,
        rendered: RenderResult,
        mode: PlanMode,
        swallowed: Option<&str>,
        guard: Guard,
    ) -> Result<(), InsertError> {
        let text = case_propagation::apply(trigger.case_style, &rendered.text);
        let offset = rendered
            .cursor_offset
            .map(|o| plan::cased_cursor_offset(trigger.case_style, &rendered.text, o));
        let spans = rendered
            .formatting
            .as_ref()
            .map(|spans| plan::cased_formatting(trigger.case_style, &rendered.text, spans));
        if !rendered.actions.is_empty() {
            let actions =
                plan::cased_actions(trigger.case_style, &rendered.text, &rendered.actions);
            let sequence = Sequence::new(&text, spans.as_deref(), &actions, offset);
            let typed = TypedTrigger {
                trigger,
                mode,
                swallowed,
            };
            if self.insert_sequence(token, guard, Some(typed), trigger.insertion, sequence)? {
                self.host.expanded(ExpansionEvent {
                    snippet_id: trigger.snippet_id,
                    chars_saved: plan::chars_saved(&text, &trigger.typed_trigger),
                });
            }
            return Ok(());
        }
        let html = spans
            .as_ref()
            .map(|spans| kudzu_core::formatting::html(&text, spans));
        let has_images = trigger.insertion != InsertionMethod::Type
            && spans.as_ref().is_some_and(|s| {
                s.iter()
                    .any(|s| matches!(s.style, kudzu_core::formatting::FormatStyle::Image { .. }))
            });
        let mut plan = InsertionPlan::new(trigger, text, offset, mode, swallowed);
        keys::delete_backward(self.events(), plan.delete_count)?;
        let caret = has_images
            .then(|| self.observe().caret_snapshot())
            .flatten();
        self.deliver(&plan.text, html.as_deref(), trigger.insertion)?;
        let mut can_undo = true;
        if has_images {
            if let Some((text, cursor)) = plan::image_delivery(
                &plan.text,
                spans.as_deref().unwrap_or_default(),
                offset,
                caret.and_then(|c| c.inserted_utf16()),
            ) {
                plan = InsertionPlan::new(trigger, text, cursor, mode, swallowed);
            } else {
                plan.caret_left_count = 0;
                can_undo = false;
            }
        }
        if !self.is_current(token) || !self.allowed(guard) {
            return Ok(());
        }
        if let Some(separator) = &plan.reemit_separator {
            keys::retype(self.events(), separator)?;
        }
        keys::move_left(self.events(), plan.caret_left_count)?;
        {
            let mut core = lock(&self.core);
            if core.generation == token && can_undo {
                // The undo tracker accounts for the re-emitted separator itself.
                core.matcher
                    .did_expand(trigger, plan::grapheme_count(&plan.text));
                core.undo_caret_left = plan.caret_left_count;
            } else if core.generation == token {
                core.reset_matcher();
            }
        }
        self.host.expanded(ExpansionEvent {
            snippet_id: trigger.snippet_id,
            chars_saved: plan::chars_saved(&plan.text, &trigger.typed_trigger),
        });
        Ok(())
    }

    fn request_typed_form(
        &self,
        token: u64,
        trigger: TriggerMatch,
        snippet: &SnippetRecord,
        fields: Vec<kudzu_core::template::FormFieldSpec>,
        mode: PlanMode,
        swallowed: Option<String>,
    ) -> Result<(), InsertError> {
        // Put the withheld key back so the document shows the whole trigger (and separator);
        // the form submission then deletes it like listen-only would.
        if mode == PlanMode::Active {
            if let Some(text) = &swallowed {
                keys::retype(self.events(), text)?;
            }
        }
        let queued = {
            let mut core = lock(&self.core);
            core.preparing_form = true;
            core.preparing_interrupted = false;
            core.hold_since = None;
            core.reset_matcher();
            core.held.drain()
        };
        let interrupted = queued.iter().any(HeldEvent::is_key_down);
        for held in queued {
            self.events().post_held(held, SYNTHETIC_MARKER);
        }
        self.observe().sleep(SETTLE);
        let target = self.observe().frontmost_app();
        let form_token = {
            let mut core = lock(&self.core);
            core.preparing_form = false;
            let interrupted = interrupted || core.preparing_interrupted;
            // Typing already reached the document after the trigger, so deleting it would be
            // wrong. Leave the trigger rather than show a form we'd discard.
            if core.generation != token || interrupted {
                return Ok(());
            }
            let form_token = core.next_form_token;
            core.next_form_token += 1;
            core.pending_form = Some(PendingForm {
                token: form_token,
                snippet_id: snippet.id,
                kind: FormKind::Typed(trigger),
                target: target.clone(),
                interrupted: false,
                panel_focused: false,
                selection: None,
            });
            form_token
        };
        if !self.allowed(Guard::Typed) {
            lock(&self.core).pending_form = None;
            return Ok(());
        }
        self.host.form_requested(FormRequest {
            token: form_token,
            snippet_id: snippet.id,
            snippet_name: snippet.name.clone(),
            fields,
            caret: self.observe().caret_rect(),
            target_app: target,
            selection: None,
        });
        Ok(())
    }

    fn submit(&self, token: u64, values: HashMap<String, String>) {
        let form = {
            let mut core = lock(&self.core);
            match core.pending_form.take() {
                Some(form) if form.token == token => form,
                other => {
                    core.pending_form = other;
                    return;
                }
            }
        };
        if form.interrupted && matches!(form.kind, FormKind::Typed(_)) {
            return;
        }
        if !self.explicit_allowed() {
            return;
        }
        let target = form
            .target
            .clone()
            .or_else(|| self.observe().frontmost_app());
        if let Some(app) = &target {
            if !self.observe().activate_and_wait(app, ACTIVATION_TIMEOUT) {
                log::warn!("form target {} did not come to the front", app.bundle_id);
                self.recorder.resolve(activity::target_gone());
                return;
            }
        }
        let generation = self.begin_explicit();
        self.observe().sleep(SETTLE);
        let result = self.insert_submitted(generation, &form, target.as_ref(), values);
        if let Err(error) = result {
            log::warn!("form insertion failed: {error}");
            self.recorder.resolve(activity::failed(&error));
        }
        self.finish(generation);
    }

    fn insert_submitted(
        &self,
        generation: u64,
        form: &PendingForm,
        target: Option<&AppInfo>,
        values: HashMap<String, String>,
    ) -> Result<(), InsertError> {
        if !self.is_current(generation) || !self.explicit_allowed() || !self.target_is_front(target)
        {
            return Ok(());
        }
        let library = self.library();
        let Some(snippet) = library.snippets.get(&form.snippet_id) else {
            return Ok(());
        };
        let values = library::with_defaults(&library.form_fields(snippet), values);
        let rendered = self.render(&library, snippet, values, form.selection.clone());
        match &form.kind {
            // The trigger and any separator are in the document as typed.
            FormKind::Typed(trigger) => self.insert_expansion(
                generation,
                trigger,
                rendered,
                PlanMode::ListenOnly,
                None,
                Guard::Explicit,
            ),
            FormKind::Palette => {
                self.insert_rendered(generation, &library, snippet, rendered, target)
            }
        }
    }

    fn target_is_front(&self, target: Option<&AppInfo>) -> bool {
        match target {
            Some(app) => self
                .observe()
                .resolve_pid(app)
                .is_some_and(|pid| self.observe().frontmost_pid() == Some(pid)),
            None => true,
        }
    }

    fn palette_insert(&self, snippet_id: Uuid, target: Option<AppInfo>, selection: Option<String>) {
        if !self.explicit_allowed() {
            return;
        }
        let library = self.library();
        let Some(snippet) = library.snippets.get(&snippet_id) else {
            return;
        };
        let target = target.or_else(|| self.observe().frontmost_app());
        // Global exclusions (password managers, login window) apply to explicit inserts too.
        if self.note_excluded(&library, target.as_ref().map(|t| t.bundle_id.as_str())) {
            return;
        }
        self.invalidate();
        let fields = library.form_fields(snippet);
        if !fields.is_empty() {
            let target_front = target
                .as_ref()
                .is_some_and(|t| self.target_is_front(Some(t)));
            let form_token = {
                let mut core = lock(&self.core);
                let form_token = core.next_form_token;
                core.next_form_token += 1;
                core.pending_form = Some(PendingForm {
                    token: form_token,
                    snippet_id,
                    kind: FormKind::Palette,
                    target: target.clone(),
                    interrupted: false,
                    panel_focused: false,
                    selection: selection.clone(),
                });
                form_token
            };
            self.host.form_requested(FormRequest {
                token: form_token,
                snippet_id,
                snippet_name: snippet.name.clone(),
                fields,
                caret: if target_front {
                    self.observe().caret_rect()
                } else {
                    None
                },
                target_app: target,
                selection,
            });
            return;
        }
        if let Some(app) = &target {
            if !self.observe().activate_and_wait(app, ACTIVATION_TIMEOUT) {
                log::warn!("palette target {} did not come to the front", app.bundle_id);
                self.recorder.resolve(activity::target_gone());
                return;
            }
        }
        let generation = self.begin_explicit();
        self.observe().sleep(SETTLE);
        let result = if self.is_current(generation)
            && self.explicit_allowed()
            && self.target_is_front(target.as_ref())
        {
            let rendered = self.render(&library, snippet, HashMap::new(), selection);
            self.insert_rendered(generation, &library, snippet, rendered, target.as_ref())
        } else {
            Ok(())
        };
        if let Err(error) = result {
            log::warn!("palette insertion failed: {error}");
            self.recorder.resolve(activity::failed(&error));
        }
        self.finish(generation);
    }

    /// Inserts a rendered snippet at the caret without deleting anything (palette).
    fn insert_rendered(
        &self,
        generation: u64,
        library: &Library,
        snippet: &SnippetRecord,
        rendered: RenderResult,
        target: Option<&AppInfo>,
    ) -> Result<(), InsertError> {
        let method = library.insertion_for(
            snippet,
            target
                .map(|t| t.bundle_id.as_str())
                .filter(|b| !b.is_empty()),
        );
        if !rendered.actions.is_empty() {
            let sequence = Sequence::new(
                &rendered.text,
                rendered.formatting.as_deref(),
                &rendered.actions,
                rendered.cursor_offset,
            );
            if self.insert_sequence(generation, Guard::Explicit, None, method, sequence)? {
                self.host.expanded(ExpansionEvent {
                    snippet_id: snippet.id,
                    chars_saved: plan::chars_saved(&rendered.text, ""),
                });
            }
            return Ok(());
        }
        let html = rendered
            .formatting
            .as_ref()
            .map(|spans| kudzu_core::formatting::html(&rendered.text, spans));
        let has_images = method != InsertionMethod::Type
            && rendered.formatting.as_ref().is_some_and(|s| {
                s.iter()
                    .any(|s| matches!(s.style, kudzu_core::formatting::FormatStyle::Image { .. }))
            });
        let caret = has_images
            .then(|| self.observe().caret_snapshot())
            .flatten();
        self.deliver(&rendered.text, html.as_deref(), method)?;
        if !self.is_current(generation) || !self.explicit_allowed() {
            return Ok(());
        }
        let delivered = if has_images {
            plan::image_delivery(
                &rendered.text,
                rendered.formatting.as_deref().unwrap_or_default(),
                rendered.cursor_offset,
                caret.and_then(|c| c.inserted_utf16()),
            )
        } else {
            Some((rendered.text.clone(), rendered.cursor_offset))
        };
        if let Some((text, Some(offset))) = delivered {
            keys::move_left(self.events(), plan::graphemes_after_utf16(&text, offset))?;
        }
        self.host.expanded(ExpansionEvent {
            snippet_id: snippet.id,
            chars_saved: plan::chars_saved(&rendered.text, ""),
        });
        Ok(())
    }
}
