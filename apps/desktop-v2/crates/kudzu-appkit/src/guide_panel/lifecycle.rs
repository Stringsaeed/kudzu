//! RUNE-GAP(#574): delete when Rune owns the permission guide's panel lifecycle.
use crate::geometry::Rect;

/// Opaque token to reject callbacks belonging to a closed or superseded guide.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Generation(u64);

/// Frontmost app, as far as placement cares. The pure machine never reads AppKit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrontApp {
    /// This Kudzu process. `begin` starts here.
    ThisApp,
    /// System Settings (`com.apple.systempreferences`).
    SystemSettings,
    /// Any other frontmost app.
    Other,
}

#[derive(Clone, Copy, Debug)]
pub struct Observation {
    pub visible: bool,
    pub front_app: FrontApp,
    pub dragging: bool,
    pub granted: bool,
    pub reduce_motion: bool,
    pub settings: Option<Rect>,
    pub companion: Rect,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Step {
    Stale,
    Stop,
    WaitUntil(u64),
    Ready(Rect),
    /// Placement is pending; this result requests no wake.
    Placing,
    Draw(f64),
}

#[derive(Default)]
pub struct Lifecycle {
    generation: Generation,
    state: State,
}

#[derive(Default)]
enum State {
    #[default]
    Idle,
    Waiting {
        due: u64,
        attempt: u8,
        /// Companion frame recorded at `begin`. When `frame_pending`, the first observation
        /// supplies it because the caller had no window yet.
        companion: Rect,
        frame_pending: bool,
        /// Last front app accepted for this wait. A change to anything other than System
        /// Settings cancels placement.
        front: FrontApp,
    },
    Ready {
        deadline: u64,
    },
    Running {
        start: u64,
        settings: Rect,
        companion: Rect,
    },
}

impl Lifecycle {
    /// Arm one placement watchdog when Ready is returned; Placing requests no further wake.
    pub const PLACEMENT_TIMEOUT_MS: u64 = 200;
    pub fn is_current(&self, generation: Generation) -> bool {
        self.generation == generation && !matches!(self.state, State::Idle)
    }
    /// `now` and every later timestamp use the same monotonic millisecond clock.
    /// `companion` is the frame at `begin`. `None` means the first observation records it.
    pub fn begin(&mut self, now: u64, companion: Option<Rect>) -> Generation {
        self.stop();
        self.state = State::Waiting {
            due: now + 450,
            attempt: 0,
            companion: companion.unwrap_or_default(),
            frame_pending: companion.is_none(),
            front: FrontApp::ThisApp,
        };
        self.generation
    }

    pub fn stop(&mut self) {
        self.generation.0 = self.generation.0.wrapping_add(1);
        self.state = State::Idle;
    }

    /// Drive at the returned deadline, or at 30 Hz while drawing. No native timer or window
    /// is retained here. The UI calls stop + panel.hide immediately on a drag/grant/close.
    pub fn poll(&mut self, generation: Generation, now: u64, input: Observation) -> Step {
        if generation != self.generation {
            return Step::Stale;
        }
        if !input.visible || input.dragging {
            self.stop();
            return Step::Stop;
        }
        match &mut self.state {
            State::Idle => Step::Stop,
            State::Ready { deadline } => {
                if now >= *deadline {
                    self.stop();
                    Step::Stop
                } else {
                    Step::Placing
                }
            }
            State::Waiting {
                due,
                attempt,
                companion,
                frame_pending,
                front,
            } => {
                // A companion move since the frame recorded at `begin` abandons placement.
                if *frame_pending {
                    *companion = input.companion;
                    *frame_pending = false;
                } else if *companion != input.companion {
                    self.stop();
                    return Step::Stop;
                }
                // A front-app change to anything other than System Settings abandons placement.
                // Staying on this app, or System Settings coming forward, continues the wait.
                let previous = *front;
                let next = input.front_app;
                if previous != next && next != FrontApp::SystemSettings {
                    self.stop();
                    return Step::Stop;
                }
                if next == FrontApp::SystemSettings {
                    *front = FrontApp::SystemSettings;
                }
                if now < *due {
                    return Step::WaitUntil(*due);
                }
                if let Some(settings) = input.settings {
                    self.state = State::Ready {
                        deadline: now + Self::PLACEMENT_TIMEOUT_MS,
                    };
                    Step::Ready(settings)
                } else if *attempt < 12 {
                    *attempt += 1;
                    *due = now + 200;
                    Step::WaitUntil(*due)
                } else {
                    self.stop();
                    Step::Stop
                }
            }
            State::Running {
                start,
                settings,
                companion,
            } => {
                if now.saturating_sub(*start) > 4800
                    || input.front_app == FrontApp::Other
                    || input.granted
                    || input.reduce_motion
                    || input.settings != Some(*settings)
                    || input.companion != *companion
                {
                    self.stop();
                    Step::Stop
                } else {
                    Step::Draw((now.saturating_sub(*start) % 2400) as f64 / 2400.)
                }
            }
        }
    }

    /// Call after placement, using the actual frames. Overlap skips the ghost entirely.
    pub fn started(
        &mut self,
        generation: Generation,
        now: u64,
        settings: Rect,
        companion: Rect,
    ) -> bool {
        if generation != self.generation {
            return false;
        }
        let State::Ready { deadline } = &self.state else {
            return false;
        };
        if now >= *deadline || settings.intersects(companion) {
            self.stop();
            return false;
        }
        self.state = State::Running {
            start: now,
            settings,
            companion,
        };
        true
    }
}

#[cfg(test)]
#[path = "lifecycle_tests.rs"]
mod tests;
