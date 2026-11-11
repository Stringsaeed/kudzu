//! The tap callback, and the worker's half of a typed match: an expansion, an undo, or a form.

use std::collections::HashMap;
use std::sync::atomic::Ordering;

use kudzu_core::matching::{KeyInput, MatcherEvent, TriggerMatch};

use super::super::ffi::{
    kCGEventKeyDown, kCGEventKeyUp, kCGEventTapDisabledByTimeout, kCGEventTapDisabledByUserInput,
    CGEventRef, CGEventType,
};
use super::super::input_source;
use super::super::keys::{self, REPLAY_MARKER, SYNTHETIC_MARKER};
use super::super::tap::{self, HeldEvent};
use super::replace::{Live, TypedTrigger};
use super::session::Core;
use super::{activity, Guard, Imp, InsertError, Job, LISTEN_ONLY_SETTLE};
use crate::hold::HoldState;
use crate::input::{self, Follow};
use crate::lock;
use crate::plan::PlanMode;
use crate::TapMode;

pub(super) struct Matched {
    pub(super) generation: u64,
    pub(super) event: MatcherEvent,
    pub(super) mode: PlanMode,
    pub(super) undo_caret: usize,
    pub(super) swallowed: Option<String>,
    pub(super) pid: Option<i32>,
}

/// Effects of a tap callback, performed after `core` is unlocked.
#[derive(Default)]
struct Effects {
    post_unmatched: Vec<HeldEvent>,
    job: Option<Job>,
    replay: bool,
    note: Option<activity::Note>,
}

impl Imp {
    // MARK: - Tap callback

    pub(super) fn receive(&self, kind: CGEventType, event: CGEventRef, mode: TapMode) -> bool {
        let mut effects = Effects::default();
        // Where a click lands is decided before the engine lock is taken: a field of the event
        // against the host's snapshot of Kudzu's windows, with no window-server call.
        let clicked = kind != kCGEventKeyDown && kind != kCGEventKeyUp;
        let on_kudzu = clicked && self.clicked_on_kudzu(event);
        let swallow = {
            let mut core = lock(&self.core);
            self.receive_locked(&mut core, kind, event, mode, on_kudzu, &mut effects)
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

    /// Whether a mouse-down lands in one of Kudzu's own windows: the window the session says
    /// will receive it is in the host's snapshot and doesn't pass clicks through. Unknown counts
    /// as no.
    fn clicked_on_kudzu(&self, event: CGEventRef) -> bool {
        let Some(window) = tap::window_receiving(event) else {
            return false;
        };
        let windows = lock(&self.kudzu_windows).clone();
        windows
            .iter()
            .any(|w| w.number == window && !w.passes_clicks_through)
    }

    fn receive_locked(
        &self,
        core: &mut Core,
        kind: CGEventType,
        event: CGEventRef,
        mode: TapMode,
        on_kudzu: bool,
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
            let suppressed = self.flags.suppressed.load(Ordering::SeqCst);
            core.note_form_click(on_kudzu);
            // A click moves the caret or focus: the typed context is gone, and any insertion in
            // flight stops. Suppression only spares clicks in Kudzu's own windows (form,
            // palette), which don't touch the target document; it never hides a click
            // elsewhere from an insertion (whoever holds the suppression).
            if !suppressed || !on_kudzu {
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
            let elsewhere = tap::target_pid(event).is_some_and(|pid| pid != self.own_pid);
            core.note_form_key_down(tap::stamp(event), tap::key_code(event) as u16, elsewhere);
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

    pub(super) fn perform_matched(&self, m: Matched) -> Result<(), InsertError> {
        let token = m.generation;
        if m.mode == PlanMode::ListenOnly {
            // Listen-only can't hold input: let the final key reach the app before deleting.
            self.observe().sleep(LISTEN_ONLY_SETTLE);
        }
        let typed_live = Live {
            generation: token,
            guard: Guard::Typed,
            pid: m.pid,
            listen_only: true,
            form: None,
        };
        // The key an active listener withheld, for a match cancelled before anything happened.
        let withheld = m
            .swallowed
            .as_deref()
            .filter(|_| m.mode == PlanMode::Active);
        let cancelled = |imp: &Imp, trigger: &TriggerMatch| {
            let typed = TypedTrigger {
                trigger,
                mode: m.mode,
                swallowed: withheld,
                live: typed_live,
            };
            imp.cancelled(&typed, 0);
        };
        if !self.is_current(token)
            || !self.allowed(Guard::Typed)
            || self.observe().frontmost_pid() != m.pid
        {
            if let MatcherEvent::Expand(trigger) = &m.event {
                cancelled(self, trigger);
            }
            return Ok(());
        }
        if m.mode == PlanMode::ListenOnly && lock(&self.core).listen_only_interrupted {
            lock(&self.core).reset_matcher();
            return Ok(());
        }
        match &m.event {
            MatcherEvent::Expand(trigger) => {
                let trigger = trigger.clone();
                let library = self.library();
                let Some(snippet) = library.snippets.get(&trigger.snippet_id) else {
                    cancelled(self, &trigger);
                    return Ok(());
                };
                let fields = library.form_fields(snippet);
                if !fields.is_empty() {
                    return self.request_typed_form(
                        typed_live,
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
                let typed = TypedTrigger {
                    trigger: &trigger,
                    mode: m.mode,
                    swallowed,
                    live: typed_live,
                };
                self.insert_expansion(typed, rendered)
            }
            MatcherEvent::Undo(undo) => {
                // A listen-only backspace already reached the document.
                let already = usize::from(m.mode == PlanMode::ListenOnly);
                let presses = [
                    keys::right_presses(m.undo_caret),
                    keys::backspace_presses(undo.delete_count.saturating_sub(already)),
                    keys::retype_presses(&undo.retype),
                ]
                .concat();
                // Each press re-checks that nothing changed: a click or another app in front
                // stops the undo where it is, never deleting or typing at the new place.
                if self.post_checked(&typed_live, &presses)? {
                    self.recorder.undone();
                }
                Ok(())
            }
        }
    }
}
