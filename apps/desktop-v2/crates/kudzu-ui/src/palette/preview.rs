//! When the preview of the active row starts, dims, and gives way.
//!
//! Time is the frame clock in seconds, so a headless script and a test agree.
//! The previous render stays up until the new one lands. It dims only after 150 ms
//! of waiting, and the first load (nothing retained) shimmers instead.

use uuid::Uuid;

/// Delay before a preview render is asked for.
pub const START_AFTER: f64 = 0.040;
/// How long the retained preview stays full opacity while a new one is in flight.
pub const DIM_AFTER: f64 = 0.150;
/// How long "Copied" stays before the palette hides.
pub const COPY_HIDE_AFTER: f64 = 0.260;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PreviewTick {
    /// Ask for a render of `want` now.
    pub start: bool,
    pub want: Option<Uuid>,
    /// The retained render. `None` until the first one lands.
    pub shown: Option<Uuid>,
    /// The retained render is stale and has been waiting longer than [`DIM_AFTER`].
    pub dim: bool,
    /// Nothing is retained for the current row, so the pane shimmers.
    pub shimmer: bool,
}

/// The 40 ms / 150 ms preview clock. `now` is `Tx::clock`.
#[derive(Clone, Debug)]
pub struct PreviewClock {
    want: Option<Uuid>,
    changed_at: f64,
    started: bool,
    shown: Option<Uuid>,
}

impl Default for PreviewClock {
    fn default() -> Self {
        Self {
            want: None,
            changed_at: 0.0,
            started: false,
            shown: None,
        }
    }
}

impl PreviewClock {
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// The active snippet changed. The same id does not restart the timer.
    pub fn set_active(&mut self, id: Option<Uuid>, now: f64) {
        if self.want == id {
            return;
        }
        self.want = id;
        self.changed_at = now;
        self.started = false;
    }

    pub fn tick(&self, now: f64) -> PreviewTick {
        let waiting = now - self.changed_at;
        let start = self.want.is_some() && !self.started && waiting >= START_AFTER;
        let stale = self.shown.is_some() && self.shown != self.want;
        PreviewTick {
            start,
            want: self.want,
            shown: self.shown,
            dim: stale && waiting > DIM_AFTER,
            shimmer: self.want.is_some() && self.shown.is_none(),
        }
    }

    /// Mark the render requested, so a later tick does not ask again.
    pub fn started(&mut self) {
        self.started = true;
    }

    /// A render landed. An older answer than the current row is kept only as the
    /// retained preview until the current one arrives; the caller drops those.
    pub fn landed(&mut self, id: Uuid) {
        self.shown = Some(id);
    }

    pub fn shown(&self) -> Option<Uuid> {
        self.shown
    }

    pub fn want(&self) -> Option<Uuid> {
        self.want
    }

    /// The session arrived after the timer started: ask again, keeping what is shown.
    pub fn kick(&mut self, now: f64) {
        if self.want.is_some() {
            self.changed_at = now;
            self.started = false;
        }
    }

    /// The next moment `tick` would change something, so `poll` can `wake_at` it.
    pub fn next_wake(&self, now: f64) -> Option<f64> {
        let want = self.want?;
        let start_at = self.changed_at + START_AFTER;
        if !self.started && now < start_at {
            return Some(start_at);
        }
        let dim_at = self.changed_at + DIM_AFTER + 0.001;
        if self.shown.is_some() && self.shown != Some(want) && now < dim_at {
            return Some(dim_at);
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(n: u128) -> Uuid {
        Uuid::from_u128(n)
    }

    #[test]
    fn the_preview_waits_40ms_keeps_the_old_one_and_dims_after_150ms() {
        let mut clock = PreviewClock::default();
        clock.set_active(Some(id(1)), 0.0);
        let first = clock.tick(0.039);
        assert!(!first.start);
        assert!(first.shimmer);
        assert!(!first.dim);
        let due = clock.tick(0.040);
        assert!(due.start);
        clock.started();
        clock.landed(id(1));
        let shown = clock.tick(0.050);
        assert!(!shown.shimmer);
        assert_eq!(shown.shown, Some(id(1)));

        clock.set_active(Some(id(2)), 1.0);
        let early = clock.tick(1.03);
        assert!(!early.start);
        assert_eq!(early.shown, Some(id(1)));
        assert!(
            !early.dim,
            "the old preview stays full opacity until 150 ms"
        );
        let dimmed = clock.tick(1.151);
        assert!(dimmed.dim);
        assert!(dimmed.start);
        clock.started();
        clock.landed(id(2));
        let fresh = clock.tick(1.20);
        assert!(!fresh.dim);
        assert_eq!(fresh.shown, Some(id(2)));
    }

    #[test]
    fn the_same_row_does_not_restart_the_timer() {
        let mut clock = PreviewClock::default();
        clock.set_active(Some(id(1)), 0.0);
        clock.set_active(Some(id(1)), 10.0);
        assert!(clock.tick(0.040).start);
    }

    #[test]
    fn no_active_row_clears_the_request() {
        let mut clock = PreviewClock::default();
        clock.set_active(Some(id(1)), 0.0);
        clock.started();
        clock.landed(id(1));
        clock.set_active(None, 1.0);
        let tick = clock.tick(2.0);
        assert!(!tick.start);
        assert!(!tick.shimmer);
        assert!(tick.dim, "the retained render is no longer the current row");
    }
}
