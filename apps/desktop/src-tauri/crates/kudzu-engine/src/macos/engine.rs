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

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicU8, Ordering};
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex, Weak};
use std::thread;
use std::time::{Duration, Instant};

use kudzu_core::matching::{case_propagation, ExpansionMatcher, KeyInput, MatcherEvent, TriggerIndex, TriggerMatch};
use kudzu_core::template::RenderResult;
use kudzu_core::types::{InsertionMethod, SnippetRecord};
use objc2::rc::autoreleasepool;
use uuid::Uuid;

use super::ffi::{CGEventRef, CGEventType, kCGEventKeyDown, kCGEventKeyUp, kCGEventTapDisabledByTimeout, kCGEventTapDisabledByUserInput};
use super::keys::{self, KeyError, REPLAY_MARKER, SYNTHETIC_MARKER};
use super::pasteboard::{self, PasteError};
use super::tap::{self, HeldEvent, TapHandle};
use super::{apps, ax};
use crate::hold::{HoldReplayQueue, HoldState};
use crate::library::{self, Library};
use crate::plan::{self, InsertionPlan, PlanMode};
use crate::status::{self, StatusInputs};
use crate::{lock, AppInfo, EngineHost, EngineLibrary, EngineStatus, ExpansionEvent, FormRequest, TapMode};

/// Physical input is never held longer than this, whatever the target app does.
const HOLD_LIMIT: Duration = Duration::from_secs(1);
/// How long to wait for a reactivated app to become frontmost.
const ACTIVATION_TIMEOUT: Duration = Duration::from_millis(500);
/// Lets focus changes and flushed input reach the target before we act on it.
const SETTLE: Duration = Duration::from_millis(50);
/// Listen-only: let the final trigger key reach the document before deleting it.
const LISTEN_ONLY_SETTLE: Duration = Duration::from_millis(30);
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
    Submit { token: u64, values: HashMap<String, String> },
    Insert { snippet_id: Uuid, target: Option<AppInfo> },
    Replay,
}

/// State the tap callback touches. Keep every operation on it short.
struct Core {
    matcher: ExpansionMatcher,
    library: Arc<Library>,
    held: HoldReplayQueue<HeldEvent>,
    hold_since: Option<Instant>,
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
}

impl Core {
    fn reset_matcher(&mut self) {
        self.matcher.reset();
        self.undo_caret_left = 0;
    }

    fn start_hold(&mut self) {
        if self.held.state() != HoldState::Inserting {
            self.hold_since = Some(Instant::now());
        }
        self.held.begin();
    }

    /// Stops in-flight work and returns held input, to be posted unmatched.
    fn invalidate(&mut self) -> Vec<HeldEvent> {
        self.generation += 1;
        self.busy = false;
        self.preparing_form = false;
        self.hold_since = None;
        self.reset_matcher();
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

    fn hold_expired(&self) -> bool {
        self.held.state() == HoldState::Inserting && self.hold_since.is_some_and(|t| t.elapsed() > HOLD_LIMIT)
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
    host: Arc<dyn EngineHost>,
    weak: Weak<Imp>,
    core: Mutex<Core>,
    flags: Flags,
    tap: Mutex<Option<TapHandle>>,
    jobs: Mutex<Option<Sender<Job>>>,
    last_status: Mutex<Option<EngineStatus>>,
    own_pid: i32,
}

impl Imp {
    pub fn new(host: Arc<dyn EngineHost>) -> Arc<Imp> {
        Arc::new_cyclic(|weak: &Weak<Imp>| {
            let (sender, receiver) = mpsc::channel::<Job>();
            let worker = weak.clone();
            let spawned = thread::Builder::new().name("kudzu-engine-worker".into()).spawn(move || {
                for job in receiver {
                    let Some(imp) = worker.upgrade() else { break };
                    autoreleasepool(|_| imp.run(job));
                }
            });
            if let Err(error) = &spawned {
                log::error!("engine worker failed to start: {error}");
            }
            let flags = Flags::default();
            flags.post_event.store(ax::post_event_allowed(), Ordering::Relaxed);
            flags.secure_input.store(ax::secure_input().0, Ordering::Relaxed);
            flags.input_monitoring.store(ax::permissions().input_monitoring, Ordering::Relaxed);
            Imp {
                host,
                weak: weak.clone(),
                core: Mutex::new(Core {
                    matcher: ExpansionMatcher::new(),
                    library: Arc::new(Library::default()),
                    held: HoldReplayQueue::default(),
                    hold_since: None,
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
                }),
                flags,
                tap: Mutex::new(None),
                jobs: Mutex::new(spawned.ok().map(|_| sender)),
                last_status: Mutex::new(None),
                own_pid: std::process::id() as i32,
            }
        })
    }

    // MARK: - Public surface

    pub fn start(&self) {
        if self.flags.running.swap(true, Ordering::SeqCst) {
            return;
        }
        autoreleasepool(|_| self.refresh());
        let epoch = self.flags.poll_epoch.fetch_add(1, Ordering::SeqCst) + 1;
        let weak = self.weak.clone();
        let spawned = thread::Builder::new().name("kudzu-engine-poll".into()).spawn(move || {
            let mut tick: u32 = 0;
            loop {
                thread::sleep(POLL_TICK);
                let Some(imp) = weak.upgrade() else { break };
                if imp.flags.poll_epoch.load(Ordering::SeqCst) != epoch || !imp.flags.running.load(Ordering::SeqCst) {
                    break;
                }
                tick = tick.wrapping_add(1);
                if tick.is_multiple_of(STATUS_EVERY_TICKS) {
                    autoreleasepool(|_| imp.refresh());
                }
                imp.enforce_hold_deadline();
            }
        });
        if let Err(error) = spawned {
            log::error!("engine poll failed to start: {error}");
        }
    }

    pub fn stop(&self) {
        self.flags.running.store(false, Ordering::SeqCst);
        self.flags.poll_epoch.fetch_add(1, Ordering::SeqCst);
        let old = lock(&self.tap).take();
        drop(old);
        self.flags.tap_mode.store(encode(TapMode::Off), Ordering::SeqCst);
        self.invalidate();
        pasteboard::restore_pending();
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

    pub fn cancel_form(&self, token: u64) {
        let mut core = lock(&self.core);
        if core.pending_form.as_ref().is_some_and(|f| f.token == token) {
            core.pending_form = None;
        }
    }

    pub fn insert_snippet(&self, snippet_id: Uuid, target: Option<AppInfo>) {
        self.send(Job::Insert { snippet_id, target });
    }

    // MARK: - Status

    fn current_status(&self) -> EngineStatus {
        let permissions = ax::permissions();
        let (secure_input, owner_pid) = ax::secure_input();
        status::build(StatusInputs {
            paused: self.flags.paused.load(Ordering::SeqCst),
            tap: decode(self.flags.tap_mode.load(Ordering::SeqCst)),
            permissions,
            post_event: ax::post_event_allowed(),
            secure_input,
            secure_input_owner: owner_pid.and_then(apps::name_of),
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
        let permissions = ax::permissions();
        let post_event = ax::post_event_allowed();
        let secure = ax::secure_input().0;
        let was_secure = self.flags.secure_input.swap(secure, Ordering::SeqCst);
        let could_post = self.flags.post_event.swap(post_event, Ordering::SeqCst);
        let could_listen = self.flags.input_monitoring.swap(permissions.input_monitoring, Ordering::SeqCst);
        if was_secure != secure || (could_post && !post_event) || (could_listen && !permissions.input_monitoring) {
            self.invalidate();
        }
        if self.flags.running.load(Ordering::SeqCst) {
            self.attach_tap(permissions.accessibility, permissions.input_monitoring);
        }
        self.publish_status();
    }

    fn tap_handler(&self, mode: TapMode) -> tap::Handler {
        let weak = self.weak.clone();
        Box::new(move |kind, event| weak.upgrade().is_some_and(|imp| imp.receive(kind, event, mode)))
    }

    fn attach_tap(&self, accessibility: bool, input_monitoring: bool) {
        let desired = if accessibility { TapMode::Active } else { TapMode::ListenOnly };
        let mut slot = lock(&self.tap);
        let current = slot.as_ref().map(TapHandle::mode);
        if !accessibility && !input_monitoring {
            if slot.take().is_some() {
                drop(slot);
                self.flags.tap_mode.store(encode(TapMode::Off), Ordering::SeqCst);
                self.invalidate();
            }
            return;
        }
        if current == Some(desired) {
            return;
        }
        let mut created = TapHandle::start(desired, self.tap_handler(desired));
        if created.is_none() && current.is_none() && desired == TapMode::Active && input_monitoring {
            created = TapHandle::start(TapMode::ListenOnly, self.tap_handler(TapMode::ListenOnly));
        }
        let Some(new) = created else { return };
        // Stop the old tap before enabling the new one so no key is matched twice.
        slot.take();
        new.enable();
        self.flags.tap_mode.store(encode(new.mode()), Ordering::SeqCst);
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
            Guard::Typed => self.typed_allowed() && apps::frontmost_pid() != Some(self.own_pid),
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
            held.post(SYNTHETIC_MARKER);
        }
        if let Some(job) = effects.job {
            self.send(job);
        }
        if effects.replay {
            self.send(Job::Replay);
        }
        swallow
    }

    fn receive_locked(&self, core: &mut Core, kind: CGEventType, event: CGEventRef, mode: TapMode, effects: &mut Effects) -> bool {
        if kind == kCGEventTapDisabledByTimeout || kind == kCGEventTapDisabledByUserInput {
            effects.post_unmatched = core.invalidate();
            return false;
        }
        let marker = tap::marker(event);
        if marker == SYNTHETIC_MARKER {
            return false;
        }
        if kind != kCGEventKeyDown && kind != kCGEventKeyUp {
            // A click moves the caret or focus: the typed context is gone. Clicks in our own
            // focused windows (form, palette) don't affect the target document.
            if !self.flags.suppressed.load(Ordering::SeqCst) {
                effects.post_unmatched = core.invalidate();
            }
            return false;
        }
        let replay = marker == REPLAY_MARKER;
        let key_down = kind == kCGEventKeyDown;
        if core.hold_expired() {
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
        let suppressed = self.flags.suppressed.load(Ordering::SeqCst);
        // Typing that reaches another app while a form is open changes the document the
        // trigger sits in. Typing into our own windows (the form itself) doesn't.
        if key_down && !suppressed && core.pending_form.is_some() && apps::frontmost_pid() != Some(self.own_pid) {
            if let Some(form) = core.pending_form.as_mut() {
                form.interrupted = true;
            }
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
            core.reset_matcher();
            effects.replay = replay;
            return false;
        }
        if !key_down {
            effects.replay = replay;
            return false;
        }
        let Some(input) = tap::classify(event) else {
            effects.replay = replay;
            return false;
        };
        let pid = apps::frontmost_pid();
        if pid != core.frontmost_pid {
            core.frontmost_pid = pid;
            core.frontmost_bundle = pid.and_then(apps::bundle_id_of);
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
            effects.replay = replay;
            return false;
        };
        if core.busy {
            core.reset_matcher();
            return false;
        }
        core.busy = true;
        core.listen_only_interrupted = false;
        if mode == TapMode::Active {
            core.start_hold();
        }
        core.generation += 1;
        let plan_mode = if mode == TapMode::Active { PlanMode::Active } else { PlanMode::ListenOnly };
        effects.job = Some(Job::Matched(Matched { generation: core.generation, event, mode: plan_mode, undo_caret, swallowed, pid }));
        mode == TapMode::Active
    }

    fn enforce_hold_deadline(&self) {
        let expired = {
            let mut core = lock(&self.core);
            if core.hold_expired() { core.invalidate() } else { Vec::new() }
        };
        for held in expired {
            held.post(SYNTHETIC_MARKER);
        }
    }

    fn invalidate(&self) {
        let drained = lock(&self.core).invalidate();
        // Never leave physical input trapped after focus, permission or pause changes.
        for held in drained {
            held.post(SYNTHETIC_MARKER);
        }
    }

    fn send(&self, job: Job) {
        if let Some(sender) = lock(&self.jobs).as_ref() {
            let _ = sender.send(job);
        }
    }

    // MARK: - Worker

    fn run(&self, job: Job) {
        match job {
            Job::Matched(matched) => {
                let generation = matched.generation;
                if let Err(error) = self.perform_matched(matched) {
                    log::warn!("expansion failed: {error}");
                    let mut core = lock(&self.core);
                    if core.generation == generation {
                        core.reset_matcher();
                    }
                }
                self.finish(generation);
            }
            Job::Submit { token, values } => self.submit(token, values),
            Job::Insert { snippet_id, target } => self.palette_insert(snippet_id, target),
            Job::Replay => {
                let next = lock(&self.core).next_replay();
                if let Some(held) = next {
                    // One event at a time: its callback can start another expansion before we
                    // release the rest (including triggers typed during a paste).
                    held.post(REPLAY_MARKER);
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
            held.post(REPLAY_MARKER);
        }
    }

    /// Starts an explicit insertion: holds typing (active tap) and returns its generation.
    fn begin_explicit(&self) -> u64 {
        let active = decode(self.flags.tap_mode.load(Ordering::SeqCst)) == TapMode::Active;
        let mut core = lock(&self.core);
        core.generation += 1;
        core.busy = true;
        core.listen_only_interrupted = false;
        core.reset_matcher();
        if active {
            core.start_hold();
        }
        core.generation
    }

    fn perform_matched(&self, m: Matched) -> Result<(), InsertError> {
        let token = m.generation;
        if m.mode == PlanMode::ListenOnly {
            // Listen-only can't hold input: let the final key reach the app before deleting.
            thread::sleep(LISTEN_ONLY_SETTLE);
        }
        if !self.is_current(token) || !self.allowed(Guard::Typed) || apps::frontmost_pid() != m.pid {
            return Ok(());
        }
        if m.mode == PlanMode::ListenOnly && lock(&self.core).listen_only_interrupted {
            lock(&self.core).reset_matcher();
            return Ok(());
        }
        match m.event {
            MatcherEvent::Expand(trigger) => {
                let library = self.library();
                let Some(snippet) = library.snippets.get(&trigger.snippet_id) else { return Ok(()) };
                let fields = library.form_fields(snippet);
                if !fields.is_empty() {
                    return self.request_typed_form(token, trigger, snippet, fields, m.mode, m.swallowed);
                }
                let rendered = self.render(&library, snippet, HashMap::new());
                let swallowed = if m.mode == PlanMode::Active { m.swallowed.as_deref() } else { None };
                self.insert_expansion(token, &trigger, rendered, m.mode, swallowed, Guard::Typed)
            }
            MatcherEvent::Undo(undo) => {
                keys::move_right(m.undo_caret)?;
                // A listen-only backspace already reached the document.
                let already = usize::from(m.mode == PlanMode::ListenOnly);
                keys::delete_backward(undo.delete_count.saturating_sub(already))?;
                keys::type_text(&undo.retype)?;
                Ok(())
            }
        }
    }

    fn render(&self, library: &Library, snippet: &SnippetRecord, values: HashMap<String, String>) -> RenderResult {
        // Read before any paste of ours replaces the clipboard, and only when the template needs it.
        let clipboard = if library.uses_clipboard(snippet) { pasteboard::read_string() } else { None };
        library.render(snippet, library::now(), clipboard, values)
    }

    /// Delivers text by typing or pasting.
    fn deliver(&self, text: &str, html: Option<&str>, method: InsertionMethod) -> Result<(), InsertError> {
        if text.is_empty() {
            return Ok(());
        }
        if plan::should_type(method, text) && (html.is_none() || method == InsertionMethod::Type) {
            keys::type_text(text)?;
        } else {
            pasteboard::paste_text(text, html)?;
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
        let offset = rendered.cursor_offset.map(|o| plan::cased_cursor_offset(trigger.case_style, &rendered.text, o));
        let spans = rendered.formatting.as_ref().map(|spans| plan::cased_formatting(trigger.case_style, &rendered.text, spans));
        let html = spans.as_ref().map(|spans| kudzu_core::formatting::html(&text, spans));
        let has_images = trigger.insertion != InsertionMethod::Type && spans.as_ref().is_some_and(|s| s.iter().any(|s| matches!(s.style, kudzu_core::formatting::FormatStyle::Image { .. })));
        let mut plan = InsertionPlan::new(trigger, text, offset, mode, swallowed);
        keys::delete_backward(plan.delete_count)?;
        let caret = has_images.then(ax::caret_snapshot).flatten();
        self.deliver(&plan.text, html.as_deref(), trigger.insertion)?;
        let mut can_undo = true;
        if has_images {
            if let Some((text, cursor)) = plan::image_delivery(&plan.text, spans.as_deref().unwrap_or_default(), offset, caret.and_then(|c| c.inserted_utf16())) {
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
            keys::type_text(separator)?;
        }
        keys::move_left(plan.caret_left_count)?;
        {
            let mut core = lock(&self.core);
            if core.generation == token && can_undo {
                // The undo tracker accounts for the re-emitted separator itself.
                core.matcher.did_expand(trigger, plan::grapheme_count(&plan.text));
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
                keys::type_text(text)?;
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
            held.post(SYNTHETIC_MARKER);
        }
        thread::sleep(SETTLE);
        let target = apps::frontmost_app();
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
            caret: ax::caret_rect(),
            target_app: target,
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
        let target = form.target.clone().or_else(apps::frontmost_app);
        if let Some(app) = &target {
            if !apps::activate_and_wait(app, ACTIVATION_TIMEOUT) {
                log::warn!("form target {} did not come to the front", app.bundle_id);
                return;
            }
        }
        let generation = self.begin_explicit();
        thread::sleep(SETTLE);
        let result = self.insert_submitted(generation, &form, target.as_ref(), values);
        if let Err(error) = result {
            log::warn!("form insertion failed: {error}");
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
        if !self.is_current(generation) || !self.explicit_allowed() || !self.target_is_front(target) {
            return Ok(());
        }
        let library = self.library();
        let Some(snippet) = library.snippets.get(&form.snippet_id) else { return Ok(()) };
        let values = library::with_defaults(&library.form_fields(snippet), values);
        let rendered = self.render(&library, snippet, values);
        match &form.kind {
            // The trigger and any separator are in the document as typed.
            FormKind::Typed(trigger) => {
                self.insert_expansion(generation, trigger, rendered, PlanMode::ListenOnly, None, Guard::Explicit)
            }
            FormKind::Palette => self.insert_rendered(generation, &library, snippet, rendered, target),
        }
    }

    fn target_is_front(&self, target: Option<&AppInfo>) -> bool {
        match target {
            Some(app) => apps::resolve_pid(app).is_some_and(|pid| apps::frontmost_pid() == Some(pid)),
            None => true,
        }
    }

    fn palette_insert(&self, snippet_id: Uuid, target: Option<AppInfo>) {
        if !self.explicit_allowed() {
            return;
        }
        let library = self.library();
        let Some(snippet) = library.snippets.get(&snippet_id) else { return };
        let target = target.or_else(apps::frontmost_app);
        // Global exclusions (password managers, login window) apply to explicit inserts too.
        if library.is_denied(target.as_ref().map(|t| t.bundle_id.as_str())) {
            return;
        }
        self.invalidate();
        let fields = library.form_fields(snippet);
        if !fields.is_empty() {
            let target_front = target.as_ref().is_some_and(|t| self.target_is_front(Some(t)));
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
                });
                form_token
            };
            self.host.form_requested(FormRequest {
                token: form_token,
                snippet_id,
                snippet_name: snippet.name.clone(),
                fields,
                caret: if target_front { ax::caret_rect() } else { None },
                target_app: target,
            });
            return;
        }
        if let Some(app) = &target {
            if !apps::activate_and_wait(app, ACTIVATION_TIMEOUT) {
                log::warn!("palette target {} did not come to the front", app.bundle_id);
                return;
            }
        }
        let generation = self.begin_explicit();
        thread::sleep(SETTLE);
        let result = if self.is_current(generation) && self.explicit_allowed() && self.target_is_front(target.as_ref()) {
            let rendered = self.render(&library, snippet, HashMap::new());
            self.insert_rendered(generation, &library, snippet, rendered, target.as_ref())
        } else {
            Ok(())
        };
        if let Err(error) = result {
            log::warn!("palette insertion failed: {error}");
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
        let method = library.insertion_for(snippet, target.map(|t| t.bundle_id.as_str()).filter(|b| !b.is_empty()));
        let html = rendered.formatting.as_ref().map(|spans| kudzu_core::formatting::html(&rendered.text, spans));
        let has_images = method != InsertionMethod::Type && rendered.formatting.as_ref().is_some_and(|s| s.iter().any(|s| matches!(s.style, kudzu_core::formatting::FormatStyle::Image { .. })));
        let caret = has_images.then(ax::caret_snapshot).flatten();
        self.deliver(&rendered.text, html.as_deref(), method)?;
        if !self.is_current(generation) || !self.explicit_allowed() {
            return Ok(());
        }
        let delivered = if has_images {
            plan::image_delivery(&rendered.text, rendered.formatting.as_deref().unwrap_or_default(), rendered.cursor_offset, caret.and_then(|c| c.inserted_utf16()))
        } else { Some((rendered.text.clone(), rendered.cursor_offset)) };
        if let Some((text, Some(offset))) = delivered {
            keys::move_left(plan::graphemes_after_utf16(&text, offset))?;
        }
        self.host.expanded(ExpansionEvent { snippet_id: snippet.id, chars_saved: plan::chars_saved(&rendered.text, "") });
        Ok(())
    }
}
