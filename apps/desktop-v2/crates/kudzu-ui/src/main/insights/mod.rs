//! Insights screen (`PLAN.md` S64, INS-2/3/4).
//!
//! Count-up is a [`LiveText`] quartic interpolator (Rune `Tween` is linear). Bars grow by
//! width on `soft_spring(0.480)` after a 60 + i×40 ms stagger; `Beat` cannot `scale_x`.
//! Tile hover `-translate-y-px` is a RUNE-GAP (no `hover_offset`).

mod count_up;
mod data;
mod view;

use std::cell::Cell;

use rune::{El, LiveText, Tx};

use crate::main::state::MainView;
use crate::shell::MainState;
use crate::theme::Theme;

const BAR_DELAY: f64 = 0.060;
const BAR_STAGGER: f64 = 0.040;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Visit {
    #[default]
    Idle,
    Arriving,
    Landed,
}

/// Per-screen state owned by Insights.
#[derive(Clone, Debug)]
pub struct State {
    visit: Cell<Visit>,
    started: Cell<f64>,
    bars_started: Cell<u32>,
    last_wpm: Cell<u32>,
    scene: Cell<bool>,
    tiles: [LiveText; 4],
}

impl Default for State {
    fn default() -> Self {
        Self {
            visit: Cell::new(Visit::Idle),
            started: Cell::new(0.0),
            bars_started: Cell::new(0),
            last_wpm: Cell::new(0),
            scene: Cell::new(false),
            tiles: [
                LiveText::new("0"),
                LiveText::new("0"),
                LiveText::new("0"),
                LiveText::new("0"),
            ],
        }
    }
}

impl State {
    pub fn is_composing(&self) -> bool {
        false
    }

    pub fn is_editing_text(&self) -> bool {
        false
    }

    pub fn blur_inputs(&mut self) {}

    pub fn tile_text(&self, index: usize) -> String {
        self.tiles[index].get().to_string()
    }

    pub fn arriving(&self) -> bool {
        self.visit.get() == Visit::Arriving
    }

    pub fn landed(&self) -> bool {
        self.visit.get() == Visit::Landed
    }

    pub fn bar_started(&self, index: usize) -> bool {
        index < 32 && self.bars_started.get() & (1 << index) != 0
    }

    /// Headless `--scene insights`: final numbers and full bars, no arrival motion.
    pub fn finish_arrival(&self) {
        self.scene.set(true);
        self.visit.set(Visit::Landed);
        self.bars_started.set(u32::MAX);
    }

    fn reset(&self) {
        self.visit.set(Visit::Idle);
        self.started.set(0.0);
        self.bars_started.set(0);
        self.last_wpm.set(0);
        self.scene.set(false);
        for tile in &self.tiles {
            tile.set("0");
        }
    }

    fn observe(&self, clock: f64, reduced: bool, data: &data::Data) {
        match self.visit.get() {
            Visit::Idle => {
                self.last_wpm.set(data.wpm);
                if reduced || self.scene.get() {
                    self.land(data);
                } else {
                    self.started.set(clock);
                    self.visit.set(Visit::Arriving);
                    self.bars_started.set(0);
                    self.snap_progress(data, 0.0);
                }
            }
            Visit::Arriving => {}
            Visit::Landed => {
                if data.wpm != self.last_wpm.get() {
                    self.last_wpm.set(data.wpm);
                    self.snap(data);
                }
            }
        }
    }

    fn land(&self, data: &data::Data) {
        self.visit.set(Visit::Landed);
        self.bars_started.set(u32::MAX);
        self.last_wpm.set(data.wpm);
        self.snap(data);
    }

    fn snap(&self, data: &data::Data) {
        for (live, tile) in self.tiles.iter().zip(data.tiles.iter()) {
            live.set(tile.value.as_str());
        }
    }

    fn snap_progress(&self, data: &data::Data, progress: f64) {
        for (live, tile) in self.tiles.iter().zip(data.tiles.iter()) {
            live.set(count_up::interpolate(&tile.value, progress));
        }
    }
}

/// True when a bar's delay elapsed this frame (so its width spring can start). Count-up ticks
/// [`LiveText`] and returns false, so started springs are not rebuilt from zero.
pub fn poll(s: &mut MainState, tx: &mut Tx) -> bool {
    if !matches!(s.frame.view, MainView::Insights) {
        if s.insights.visit.get() != Visit::Idle {
            s.insights.reset();
        }
        return false;
    }
    if s.insights.visit.get() != Visit::Arriving {
        return false;
    }
    let clock = s.frame.clock_secs;
    let data = data::from_state(s);
    let elapsed = (clock - s.insights.started.get()).max(0.0);
    let progress = (elapsed / count_up::DURATION).clamp(0.0, 1.0);
    s.insights.snap_progress(&data, progress);
    if progress >= 1.0 {
        s.insights.visit.set(Visit::Landed);
        s.insights.last_wpm.set(data.wpm);
    }
    let n_bars = if data.top.is_empty() {
        3
    } else {
        data.top.len()
    };
    let mut bits = s.insights.bars_started.get();
    let mut started_new = false;
    for i in 0..n_bars.min(32) {
        let delay = BAR_DELAY + i as f64 * BAR_STAGGER;
        let mask = 1u32 << i;
        if elapsed + 1e-6 >= delay && bits & mask == 0 {
            bits |= mask;
            started_new = true;
        }
    }
    s.insights.bars_started.set(bits);
    tx.wake_at(clock + 1.0 / 120.0);
    started_new
}

pub fn view(s: &MainState, theme: &Theme, reduced: bool) -> El {
    view::tree(s, theme, reduced)
}
