//! Seam: what the engine observes about the system, and when.
//!
//! [`Clock`] is time: the current instant, the settle and restore waits, and the poll timer.
//! [`Observe`] adds the frontmost app (and bringing one forward), the permission and Secure
//! Input state, and the caret. [`SystemObserve`] asks macOS. The protocol tests substitute a
//! scripted desktop, so they control every observation and never wait in real time.
//!
//! The form panel's focus is the one observation that comes from the host instead:
//! [`Engine::form_panel_focused`](crate::Engine::form_panel_focused).

use std::io;
use std::thread;
use std::time::{Duration, Instant};

use super::ffi::CGWindowID;
use super::{apps, ax};
use crate::{AppInfo, PermissionStatus, Rect};

/// Time as the engine sees it.
pub(crate) trait Clock: Send + Sync {
    fn now(&self) -> Instant;

    fn sleep(&self, duration: Duration);

    /// Calls `tick` every `period` on a thread named `name` until it returns false.
    fn every(
        &self,
        name: &str,
        period: Duration,
        tick: Box<dyn FnMut() -> bool + Send>,
    ) -> io::Result<()>;
}

/// The system state the engine reads, and the one change it makes: bringing an app forward.
pub(crate) trait Observe: Clock {
    /// Pid of the frontmost application. Called from the tap callback, so it must be cheap.
    fn frontmost_pid(&self) -> Option<i32>;
    fn frontmost_app(&self) -> Option<AppInfo>;
    /// The running process `pid`.
    fn app_of(&self, pid: i32) -> Option<AppInfo>;
    fn bundle_id_of(&self, pid: i32) -> Option<String>;
    fn name_of(&self, pid: i32) -> Option<String>;
    /// The process that owns window `window` (from a mouse-down's window under the pointer).
    fn window_owner(&self, window: CGWindowID) -> Option<i32>;
    /// The pid `app` resolves to when running (by pid, else bundle id).
    fn resolve_pid(&self, app: &AppInfo) -> Option<i32>;
    /// Brings `app` to the front and waits (≤ `timeout`) until it is frontmost.
    fn activate_and_wait(&self, app: &AppInfo, timeout: Duration) -> bool;

    /// Accessibility and Input Monitoring grants.
    fn permissions(&self) -> PermissionStatus;
    /// Whether synthetic events may be posted (Accessibility).
    fn post_event_allowed(&self) -> bool;
    /// Whether Secure Input is on, and the owning pid when the session reports it.
    fn secure_input(&self) -> (bool, Option<i32>);

    /// Caret bounds of the focused text element, else the element's frame.
    fn caret_rect(&self) -> Option<Rect>;
    /// The focused element's collapsed caret offset, to measure what an insertion added.
    fn caret_snapshot(&self) -> Option<Box<dyn CaretMark>>;
}

/// A caret position taken before an insertion.
pub(crate) trait CaretMark {
    /// UTF-16 units the caret moved forward since, in the same element. `None` when the app
    /// reports no caret, focus moved, or the caret went backwards.
    fn inserted_utf16(&self) -> Option<usize>;
}

/// The real system: NSWorkspace, Accessibility and CoreGraphics.
pub(crate) struct SystemObserve;

impl Clock for SystemObserve {
    fn now(&self) -> Instant {
        Instant::now()
    }

    fn sleep(&self, duration: Duration) {
        thread::sleep(duration);
    }

    fn every(
        &self,
        name: &str,
        period: Duration,
        mut tick: Box<dyn FnMut() -> bool + Send>,
    ) -> io::Result<()> {
        thread::Builder::new()
            .name(name.into())
            .spawn(move || loop {
                thread::sleep(period);
                if !tick() {
                    break;
                }
            })?;
        Ok(())
    }
}

impl Observe for SystemObserve {
    fn frontmost_pid(&self) -> Option<i32> {
        apps::frontmost_pid()
    }

    fn frontmost_app(&self) -> Option<AppInfo> {
        apps::frontmost_app()
    }

    fn app_of(&self, pid: i32) -> Option<AppInfo> {
        apps::app_of(pid)
    }

    fn bundle_id_of(&self, pid: i32) -> Option<String> {
        apps::bundle_id_of(pid)
    }

    fn name_of(&self, pid: i32) -> Option<String> {
        apps::name_of(pid)
    }

    fn window_owner(&self, window: CGWindowID) -> Option<i32> {
        apps::window_owner(window)
    }

    fn resolve_pid(&self, app: &AppInfo) -> Option<i32> {
        apps::resolve_pid(app)
    }

    fn activate_and_wait(&self, app: &AppInfo, timeout: Duration) -> bool {
        apps::activate_and_wait(app, timeout)
    }

    fn permissions(&self) -> PermissionStatus {
        ax::permissions()
    }

    fn post_event_allowed(&self) -> bool {
        ax::post_event_allowed()
    }

    fn secure_input(&self) -> (bool, Option<i32>) {
        ax::secure_input()
    }

    fn caret_rect(&self) -> Option<Rect> {
        ax::caret_rect()
    }

    fn caret_snapshot(&self) -> Option<Box<dyn CaretMark>> {
        ax::caret_snapshot().map(|caret| Box::new(caret) as Box<dyn CaretMark>)
    }
}

impl CaretMark for ax::CaretSnapshot {
    fn inserted_utf16(&self) -> Option<usize> {
        ax::CaretSnapshot::inserted_utf16(self)
    }
}
