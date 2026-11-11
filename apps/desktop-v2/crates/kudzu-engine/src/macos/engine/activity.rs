//! Activity notes for the macOS engine ("Why didn't this expand?").
//!
//! Everything the engine needs to describe an attempt lives here, so the tap callback and the
//! insertion path only carry one-line calls. See `crate::activity` for how attempts are tracked
//! and `kudzu_core::activity` for what an event may hold.

use std::sync::atomic::Ordering;

use kudzu_core::activity::{
    ActivityApp, ActivityOutcome, ActivitySource, InsertFailure, Permission, RuleLevel,
    TapDisabledCause,
};
use kudzu_core::matching::{MatcherEvent, MatchingConfig, SkipReason, SkippedMatch};
use uuid::Uuid;

use super::super::ffi::{kCGEventTapDisabledByTimeout, CGEventRef, CGEventType};
use super::super::keys::KeyError;
use super::super::pasteboard::PasteError;
use super::super::tap;
use super::match_flow::Matched;
use super::session::Core;
use super::{Imp, InsertError};
use crate::activity::{activity_app, Subject};
use crate::library::Library;
use crate::AppInfo;

/// Something worth reporting that the tap callback saw. Built under the `core` lock, so it holds
/// only ids; the app is looked up when it is emitted on the worker.
pub(super) enum Note {
    Health(ActivityOutcome),
    Trigger {
        snippet_id: Uuid,
        trigger: String,
        pid: Option<i32>,
        outcome: ActivityOutcome,
    },
}

impl Note {
    /// macOS disabled the tap; the tap thread has already re-enabled it.
    pub fn tap_disabled(kind: CGEventType) -> Option<Note> {
        let cause = if kind == kCGEventTapDisabledByTimeout {
            TapDisabledCause::Timeout
        } else {
            TapDisabledCause::UserInput
        };
        Some(Note::Health(ActivityOutcome::TapDisabled {
            cause,
            reenabled: true,
        }))
    }

    /// A trigger typed in full that is turned off or not allowed in this app.
    pub fn skipped(skipped: Option<SkippedMatch>, pid: Option<i32>) -> Option<Note> {
        let SkippedMatch {
            snippet_id,
            trigger,
            cause,
        } = skipped?;
        let outcome = match cause {
            SkipReason::SnippetOff => ActivityOutcome::SnippetOff,
            SkipReason::GroupOff { group_id } => ActivityOutcome::GroupOff { group_id },
            SkipReason::Rule { level, group_id } => ActivityOutcome::AppRule { level, group_id },
        };
        Some(Note::Trigger {
            snippet_id,
            trigger,
            pid,
            outcome,
        })
    }

    /// A trigger that fired while another insertion was still in flight, so it was dropped.
    pub fn dropped(event: &MatcherEvent, pid: Option<i32>) -> Option<Note> {
        let MatcherEvent::Expand(m) = event else {
            return None;
        };
        Some(Note::Trigger {
            snippet_id: m.snippet_id,
            trigger: m.trigger.clone(),
            pid,
            outcome: ActivityOutcome::Interrupted,
        })
    }
}

/// The insertion error as a code. Never includes what was being inserted.
pub(super) fn failed(error: &InsertError) -> ActivityOutcome {
    let key = |e: &KeyError| match e {
        KeyError::NotAllowed => InsertFailure::NotAllowed,
        KeyError::EventCreation => InsertFailure::EventCreation,
    };
    let error = match error {
        InsertError::Key(e) => key(e),
        InsertError::Paste(PasteError::Key(e)) => key(e),
        InsertError::Paste(PasteError::Unavailable) => InsertFailure::PasteUnavailable,
    };
    ActivityOutcome::InsertionFailed { error }
}

/// The target app never came to the front.
pub(super) fn target_gone() -> ActivityOutcome {
    ActivityOutcome::InsertionFailed {
        error: InsertFailure::TargetGone,
    }
}

/// `bundle_id` is an app where nothing expands: built in, or added by the user in Settings.
pub(super) fn excluded(bundle_id: &str) -> ActivityOutcome {
    let level = if MatchingConfig::default_denied_bundle_ids().contains(bundle_id) {
        RuleLevel::Default
    } else {
        RuleLevel::AppSetting
    };
    ActivityOutcome::AppRule {
        level,
        group_id: None,
    }
}

impl Imp {
    fn activity_app_of(&self, pid: Option<i32>) -> Option<ActivityApp> {
        pid.and_then(|pid| self.observe().app_of(pid))
            .map(|app| activity_app(&app))
    }

    /// Why nothing may be inserted right now, in the order the guards check it. Flag reads
    /// only, so it is safe under the `core` lock; the Secure Input owner is filled in later.
    fn blocked(&self) -> Option<ActivityOutcome> {
        if self.flags.paused.load(Ordering::SeqCst) {
            Some(ActivityOutcome::Paused)
        } else if self.flags.secure_input.load(Ordering::SeqCst) {
            Some(ActivityOutcome::SecureInput { owner: None })
        } else if !self.flags.post_event.load(Ordering::SeqCst) {
            Some(ActivityOutcome::MissingPermission {
                permission: Permission::Accessibility,
            })
        } else {
            None
        }
    }

    /// Names the app holding Secure Input, when macOS reports one.
    fn with_owner(&self, outcome: ActivityOutcome) -> ActivityOutcome {
        match outcome {
            ActivityOutcome::SecureInput { owner: None } => ActivityOutcome::SecureInput {
                owner: self
                    .observe()
                    .secure_input()
                    .1
                    .and_then(|pid| self.observe().name_of(pid)),
            },
            other => other,
        }
    }

    /// The outcome of an attempt that ended without saying why: whatever blocks insertion now,
    /// else the user's own typing, click or app switch got in first.
    pub(super) fn unresolved(&self) -> ActivityOutcome {
        self.blocked()
            .map_or(ActivityOutcome::Interrupted, |outcome| {
                self.with_owner(outcome)
            })
    }

    /// The attempt a matched trigger starts; `None` for an undo.
    pub(super) fn typed_subject(&self, m: &Matched) -> Option<Subject> {
        let MatcherEvent::Expand(trigger) = &m.event else {
            return None;
        };
        Some(Subject {
            source: ActivitySource::Typed,
            snippet_id: trigger.snippet_id,
            trigger: Some(trigger.trigger.clone()),
            app: self.activity_app_of(m.pid),
        })
    }

    /// The attempt a Quick Search insert starts.
    pub(super) fn palette_subject(
        &self,
        snippet_id: Uuid,
        target: Option<&AppInfo>,
    ) -> Option<Subject> {
        let app = target
            .cloned()
            .or_else(|| self.observe().frontmost_app())
            .map(|app| activity_app(&app));
        Some(Subject {
            source: ActivitySource::QuickSearch,
            snippet_id,
            trigger: None,
            app,
        })
    }

    /// Resolves the open attempt when `bundle_id` is excluded. Returns whether it is.
    pub(super) fn note_excluded(&self, library: &Library, bundle_id: Option<&str>) -> bool {
        let denied = library.is_denied(bundle_id);
        if let (true, Some(bundle_id)) = (denied, bundle_id) {
            self.recorder.resolve(excluded(bundle_id));
        }
        denied
    }

    /// A key arrived while typed triggers may not expand. In Kudzu's own windows the context is
    /// simply dropped, as before. While paused or without permission to type, the matcher keeps
    /// following the keys for one purpose: to note a trigger that would otherwise have fired.
    /// Nothing is swallowed, inserted or stored, and only a full match produces a note.
    pub(super) fn observe_blocked(
        &self,
        core: &mut Core,
        key_down: bool,
        event: CGEventRef,
    ) -> Option<Note> {
        let outcome = self
            .blocked()
            .filter(|_| !self.flags.suppressed.load(Ordering::SeqCst));
        let Some(outcome) = outcome else {
            core.reset_matcher();
            return None;
        };
        if !key_down {
            return None;
        }
        let input = tap::classify(event)?;
        let (pid, bundle) = self.front_for_tap(core.frontmost_pid);
        if pid != core.frontmost_pid {
            core.frontmost_pid = pid;
            core.frontmost_bundle = bundle;
            core.reset_matcher();
        }
        if pid == Some(self.own_pid) {
            core.reset_matcher();
            return None;
        }
        let bundle = core.frontmost_bundle.clone();
        match core.matcher.handle(input, bundle.as_deref())? {
            MatcherEvent::Expand(m) => Some(Note::Trigger {
                snippet_id: m.snippet_id,
                trigger: m.trigger,
                pid,
                outcome,
            }),
            MatcherEvent::Undo(_) => {
                core.reset_matcher();
                None
            }
        }
    }

    /// Reports a tap-thread note. Runs on the worker, where looking up the app is cheap enough.
    pub(super) fn emit(&self, note: Note) {
        match note {
            Note::Health(outcome) => self.recorder.health(outcome),
            Note::Trigger {
                snippet_id,
                trigger,
                pid,
                outcome,
            } => {
                let subject = Subject {
                    source: ActivitySource::Typed,
                    snippet_id,
                    trigger: Some(trigger),
                    app: self.activity_app_of(pid),
                };
                self.recorder.note(subject, self.with_owner(outcome));
            }
        }
    }
}
