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
use std::sync::atomic::Ordering;
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex, Weak};
use std::thread;
use std::time::Duration;

use kudzu_core::matching::{ExpansionMatcher, TriggerIndex};
use objc2::rc::autoreleasepool;
use uuid::Uuid;

use super::events::{Events, Listener};
use super::input_source;
use super::keys::{KeyError, REPLAY_MARKER};
use super::observe::Observe;
use super::pasteboard::{PasteError, Pasteboard};
use super::Platform;
use crate::activity::Recorder;
use crate::hold::HoldReplayQueue;
use crate::input::KeyContext;
use crate::library::Library;
use crate::{lock, AppInfo, EngineHost, EngineLibrary, EngineStatus, KudzuWindow, Submission};

mod activity;
mod explicit;
mod form_flow;
mod insert;
mod match_flow;
mod replace;
mod session;

use match_flow::Matched;
use session::{Core, Flags, Front};

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
/// The locale of a library that comes without one. The host passes the system locale.
const DEFAULT_LOCALE: &str = "en_US";

/// Which guard an insertion re-checks between steps.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Guard {
    /// A typed trigger: blocked in our own windows and while suppressed.
    Typed,
    /// Something the user explicitly asked for (form submit, palette).
    Explicit,
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
    /// Kudzu's own windows, as the host last said; read by the listener on every click.
    kudzu_windows: Mutex<Arc<[KudzuWindow]>>,
    /// The frontmost app as the poll last saw it, for the tap callback, which never asks
    /// NSWorkspace itself (see `session::Front`).
    front: Mutex<Front>,
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
            let front = Front::read(observe.as_ref());
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
                kudzu_windows: Mutex::new(Arc::from([])),
                front: Mutex::new(front),
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

    pub fn reload(&self, library: EngineLibrary) {
        let new = Library::new(&library, || DEFAULT_LOCALE.to_string());
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

    pub fn set_suppressed(&self, suppressed: bool) {
        self.flags.suppressed.store(suppressed, Ordering::SeqCst);
        lock(&self.core).reset_matcher();
    }

    pub fn status(&self) -> EngineStatus {
        self.current_status()
    }

    /// Replaces the snapshot of Kudzu's windows the listener compares clicks with.
    pub fn set_kudzu_windows(&self, windows: &[KudzuWindow]) {
        *lock(&self.kudzu_windows) = Arc::from(windows);
    }

    /// Accepts the submission of form `token` now, on the caller's thread, so input from now on
    /// counts against it; the worker does the rest.
    pub fn submit_form(&self, token: u64, values: HashMap<String, String>, submission: Submission) {
        if self.accept_submission(token, submission) {
            self.send(Job::Submit { token, values });
        }
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
}
