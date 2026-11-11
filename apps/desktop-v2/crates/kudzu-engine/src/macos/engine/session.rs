//! The typing session and the engine's lifecycle.
//!
//! [`Core`] is the state the tap callback touches: the matcher, held input, the generation that
//! cancels in-flight work, the pending form. The rest is the lifecycle around it: start, the
//! 100 ms poll, attaching or upgrading the listener, publishing status, and stopping.

use std::sync::atomic::{AtomicBool, AtomicU64, AtomicU8, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use kudzu_core::matching::ExpansionMatcher;
use objc2::rc::autoreleasepool;

use super::super::input_source;
use super::super::keys::SYNTHETIC_MARKER;
use super::super::tap::{self, HeldEvent};
use super::form_flow::{FormKind, PendingForm};
use super::{Imp, HOLD_LIMIT, POLL_TICK, STATUS_EVERY_TICKS};
use crate::hold::{HoldReplayQueue, HoldState};
use crate::input::KeyContext;
use crate::library::Library;
use crate::status::{self, StatusInputs};
use crate::{lock, EngineStatus, EventStamp, TapMode};

/// State the tap callback touches. Keep every operation on it short.
pub(super) struct Core {
    pub(super) matcher: ExpansionMatcher,
    pub(super) library: Arc<Library>,
    pub(super) held: HoldReplayQueue<HeldEvent>,
    pub(super) hold_since: Option<Instant>,
    /// Extra time the current step of a key sequence may hold input (a `{{delay}}`).
    pub(super) hold_extra: Duration,
    /// Bumped whenever in-flight work must stop (click, pause, deadline, new insertion).
    pub(super) generation: u64,
    /// An insertion is in flight.
    pub(super) busy: bool,
    pub(super) listen_only_interrupted: bool,
    pub(super) preparing_form: bool,
    /// A key reached the target while the form was being prepared.
    pub(super) preparing_interrupted: bool,
    pub(super) pending_form: Option<PendingForm>,
    pub(super) next_form_token: u64,
    /// Left-arrow presses after the last expansion's cursor placement (undo moves back first).
    pub(super) undo_caret_left: usize,
    pub(super) frontmost_pid: Option<i32>,
    pub(super) frontmost_bundle: Option<String>,
    /// A pending dead key or held key, which changes what the next key-down means.
    pub(super) keys: KeyContext,
    /// The input source epoch the match buffer was typed under.
    pub(super) input_epoch: u64,
}

impl Core {
    pub(super) fn reset_matcher(&mut self) {
        self.matcher.reset();
        self.undo_caret_left = 0;
    }

    pub(super) fn start_hold(&mut self, now: Instant) {
        if self.held.state() != HoldState::Inserting {
            self.hold_since = Some(now);
        }
        self.hold_extra = Duration::ZERO;
        self.held.begin();
    }

    /// Stops in-flight work and returns held input, to be posted unmatched.
    pub(super) fn invalidate(&mut self) -> Vec<HeldEvent> {
        self.generation += 1;
        self.busy = false;
        self.preparing_form = false;
        self.hold_since = None;
        self.reset_matcher();
        self.keys.clear();
        self.held.drain()
    }

    /// Next held event to replay once nothing is in flight.
    pub(super) fn next_replay(&mut self) -> Option<HeldEvent> {
        if self.busy {
            return None;
        }
        let next = self.held.next();
        if next.is_none() {
            self.hold_since = None;
        }
        next
    }

    pub(super) fn hold_expired(&self, now: Instant) -> bool {
        self.held.state() == HoldState::Inserting
            && self
                .hold_since
                .is_some_and(|t| now.saturating_duration_since(t) > HOLD_LIMIT + self.hold_extra)
    }

    /// FRM-4 (D9): a key-down that reached the session, for the pending form. Suppression
    /// doesn't matter here: it stops matching, not this.
    pub(super) fn note_form_key_down(&mut self, stamp: EventStamp, key_code: u16, elsewhere: bool) {
        if let Some(form) = self.pending_form.as_mut() {
            form.input.key_down(stamp.0, key_code, elsewhere);
        }
    }

    /// FRM-4 (D9): a click that reached the session, for the pending form. `on_kudzu` says
    /// whether it lands in one of Kudzu's own windows; it is only asked for while a typed form
    /// is pending, since it costs a window-server lookup.
    pub(super) fn note_form_click(&mut self, on_kudzu: impl FnOnce() -> bool) {
        if let Some(form) = self.pending_form.as_mut() {
            if matches!(form.kind, FormKind::Typed(_)) {
                form.input.click(on_kudzu());
            }
        }
    }
}

#[derive(Default)]
pub(super) struct Flags {
    pub(super) running: AtomicBool,
    pub(super) paused: AtomicBool,
    pub(super) suppressed: AtomicBool,
    pub(super) secure_input: AtomicBool,
    pub(super) post_event: AtomicBool,
    pub(super) input_monitoring: AtomicBool,
    pub(super) tap_mode: AtomicU8,
    pub(super) poll_epoch: AtomicU64,
}

pub(super) fn encode(mode: TapMode) -> u8 {
    match mode {
        TapMode::Off => 0,
        TapMode::ListenOnly => 1,
        TapMode::Active => 2,
    }
}

pub(super) fn decode(raw: u8) -> TapMode {
    match raw {
        1 => TapMode::ListenOnly,
        2 => TapMode::Active,
        _ => TapMode::Off,
    }
}

impl Imp {
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

    pub fn set_paused(&self, paused: bool) {
        self.flags.paused.store(paused, Ordering::SeqCst);
        self.invalidate();
        self.publish_status();
    }

    // MARK: - Status

    pub(super) fn current_status(&self) -> EngineStatus {
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

    pub(super) fn invalidate(&self) {
        let drained = lock(&self.core).invalidate();
        // Never leave physical input trapped after focus, permission or pause changes.
        for held in drained {
            self.events().post_held(held, SYNTHETIC_MARKER);
        }
    }
}
