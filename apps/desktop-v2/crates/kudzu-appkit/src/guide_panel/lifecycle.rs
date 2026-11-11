//! RUNE-GAP(#574): delete when Rune owns the permission guide's panel lifecycle.
use crate::geometry::Rect;

/// Opaque token to reject callbacks belonging to a closed or superseded guide.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Generation(u64);

#[derive(Clone, Copy, Debug)]
pub struct Observation {
    pub visible: bool,
    /// Only this app and System Settings are relevant foreground apps.
    pub relevant_front_app: bool,
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
    pub fn begin(&mut self, now: u64) -> Generation {
        self.stop();
        self.state = State::Waiting {
            due: now + 450,
            attempt: 0,
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
            State::Waiting { due, attempt } => {
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
                    || !input.relevant_front_app
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
