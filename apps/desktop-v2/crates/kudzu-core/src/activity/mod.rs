//! Recent expansion attempts and why each did or didn't expand ("Why didn't this expand?").
//!
//! Privacy contract, which every caller must keep:
//! - an event exists only when a trigger **fully matched** (or for engine health); near-misses
//!   and surrounding keystrokes are never recorded;
//! - an event names the snippet, its **stored** trigger and the frontmost app. It never holds
//!   typed text, expanded content, form values or clipboard contents;
//! - the log lives in memory only: a bounded ring ([`CAPACITY`]) that is dropped on quit. It is
//!   deliberately not `Serialize`, so it cannot be written to disk, synced or handed to agents.
//!
//! `explain` turns an event into the sentence, tone and fix the Activity pane shows. The event
//! types serialize only in unit tests, preserving the copied contract assertions.

mod explain;

use std::collections::VecDeque;

use chrono::{DateTime, Utc};
#[cfg(test)]
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub use explain::{explain, ActivityEntry, Explanation, Fix, Tone};

/// How many events the log keeps.
pub const CAPACITY: usize = 100;

/// What started an expansion attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(test, derive(Serialize, Deserialize))]
#[cfg_attr(test, serde(rename_all = "camelCase"))]
pub enum ActivitySource {
    /// A trigger typed in another app.
    Typed,
    /// An insert from Quick Search.
    QuickSearch,
    /// A fill-in form that was submitted or cancelled.
    Form,
}

/// Which level of the rule chain decided (snippet ▸ group ▸ app setting ▸ default).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(test, derive(Serialize, Deserialize))]
#[cfg_attr(test, serde(rename_all = "camelCase"))]
pub enum RuleLevel {
    /// The snippet's own app rule.
    Snippet,
    /// A rule inherited from a group.
    Group,
    /// An app the user excluded in Settings ▸ Apps.
    AppSetting,
    /// A built-in exclusion (password managers, the login window).
    Default,
}

/// A macOS privacy permission.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(test, derive(Serialize, Deserialize))]
#[cfg_attr(test, serde(rename_all = "camelCase"))]
pub enum Permission {
    Accessibility,
    InputMonitoring,
}

/// Why macOS switched the keyboard tap off.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(test, derive(Serialize, Deserialize))]
#[cfg_attr(test, serde(rename_all = "camelCase"))]
pub enum TapDisabledCause {
    /// The callback took too long.
    Timeout,
    /// `kCGEventTapDisabledByUserInput`.
    UserInput,
}

/// Why an insertion failed. Codes only: never the text being inserted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(test, derive(Serialize, Deserialize))]
#[cfg_attr(test, serde(rename_all = "camelCase"))]
pub enum InsertFailure {
    /// macOS refused synthetic keystrokes (permission revoked or Secure Input turned on).
    NotAllowed,
    /// A keyboard event could not be created.
    EventCreation,
    /// The pasteboard could not be used.
    PasteUnavailable,
    /// The app to insert into quit or would not come to the front.
    TargetGone,
}

/// What happened to a fully matched trigger, or to the engine itself.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(test, derive(Serialize, Deserialize))]
#[cfg_attr(
    test,
    serde(
        tag = "kind",
        rename_all = "camelCase",
        rename_all_fields = "camelCase"
    )
)]
pub enum ActivityOutcome {
    Expanded,
    /// Backspace right after the expansion put the trigger back.
    Undone,
    Paused,
    SnippetOff,
    /// `group_id` is the nearest group that is turned off.
    GroupOff {
        group_id: Uuid,
    },
    /// `group_id` is the group whose rule decided, at [`RuleLevel::Group`].
    AppRule {
        level: RuleLevel,
        group_id: Option<Uuid>,
    },
    /// `owner` is the app holding Secure Input, when macOS reports it.
    SecureInput {
        owner: Option<String>,
    },
    MissingPermission {
        permission: Permission,
    },
    FormCancelled,
    InsertionFailed {
        error: InsertFailure,
    },
    /// Typing, a click or an app switch got in before the insertion could start.
    Interrupted,
    /// Health: macOS disabled the keyboard tap; `reenabled` when Kudzu turned it back on.
    TapDisabled {
        cause: TapDisabledCause,
        reenabled: bool,
    },
    /// Health: the keyboard tap was removed because its permissions went away.
    ListenerLost,
    /// Health: the engine was stopped.
    EngineStopped,
}

impl ActivityOutcome {
    /// Whether this describes the engine rather than one snippet.
    pub fn is_health(&self) -> bool {
        matches!(
            self,
            Self::TapDisabled { .. } | Self::ListenerLost | Self::EngineStopped
        )
    }
}

/// The app that was frontmost.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(test, derive(Serialize, Deserialize))]
#[cfg_attr(test, serde(rename_all = "camelCase"))]
pub struct ActivityApp {
    pub bundle_id: String,
    pub name: String,
    /// Bundle path, for the icon.
    #[cfg_attr(test, serde(default))]
    pub path: Option<String>,
}

/// One expansion attempt or health change. See the module docs for what it may hold.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(test, derive(Serialize, Deserialize))]
#[cfg_attr(test, serde(rename_all = "camelCase"))]
pub struct ActivityEvent {
    /// Assigned by [`ActivityLog::push`]; increases with time.
    pub id: u64,
    pub at: DateTime<Utc>,
    /// `None` for health events.
    pub source: Option<ActivitySource>,
    pub snippet_id: Option<Uuid>,
    /// The trigger as stored on the snippet (never as typed). `None` for Quick Search.
    pub trigger: Option<String>,
    pub app: Option<ActivityApp>,
    pub outcome: ActivityOutcome,
}

impl ActivityEvent {
    /// An attempt on `snippet_id`, stamped now.
    pub fn attempt(
        source: ActivitySource,
        snippet_id: Uuid,
        trigger: Option<String>,
        app: Option<ActivityApp>,
        outcome: ActivityOutcome,
    ) -> Self {
        Self {
            id: 0,
            at: Utc::now(),
            source: Some(source),
            snippet_id: Some(snippet_id),
            trigger,
            app,
            outcome,
        }
    }

    /// An engine health event, stamped now.
    pub fn health(outcome: ActivityOutcome) -> Self {
        Self {
            id: 0,
            at: Utc::now(),
            source: None,
            snippet_id: None,
            trigger: None,
            app: None,
            outcome,
        }
    }
}

/// A bounded, in-memory ring of recent events. Not serializable on purpose.
#[derive(Debug, Clone)]
pub struct ActivityLog {
    events: VecDeque<ActivityEvent>,
    capacity: usize,
    next_id: u64,
}

impl ActivityLog {
    /// A log holding [`CAPACITY`] events.
    pub fn new() -> Self {
        Self::with_capacity(CAPACITY)
    }

    /// A log holding at most `capacity` events (at least one).
    pub fn with_capacity(capacity: usize) -> Self {
        let capacity = capacity.max(1);
        Self {
            events: VecDeque::with_capacity(capacity),
            capacity,
            next_id: 1,
        }
    }

    /// Appends `event`, dropping the oldest when full. Returns it with its id.
    pub fn push(&mut self, mut event: ActivityEvent) -> ActivityEvent {
        event.id = self.next_id;
        self.next_id += 1;
        if self.events.len() == self.capacity {
            self.events.pop_front();
        }
        self.events.push_back(event.clone());
        event
    }

    /// Events, newest first.
    pub fn recent(&self) -> Vec<ActivityEvent> {
        self.events.iter().rev().cloned().collect()
    }

    /// Forgets everything. Ids keep increasing so a late listener can't confuse old and new.
    pub fn clear(&mut self) {
        self.events.clear();
    }

    pub fn len(&self) -> usize {
        self.events.len()
    }

    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }
}

impl Default for ActivityLog {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests;
