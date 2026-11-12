//! The §1.7 scroll trace for the picker: 5,000 clips scrolled by the wheel. The panel's view
//! must not run while the list scrolls (rows hold no hover state, handlers say nothing to the
//! view, `poll` reports no change). Rune itself commits to shift the mounted window of a long
//! `virtual_rows` list; those commits remount rows and do not run `view`, so the trace counts
//! runs of the panel's view, not `Runtime` commits. `KUDZU_SCROLL_BUDGET_MS` sets the frame
//! bound (4 ms p95 in a release build; loose in a debug one).

use std::time::{Duration, Instant};

use kudzu_app::settings::Theme;

use super::Rig;
use crate::main::clipboard::scenes::history_5000;
use crate::picker::view::VIEWS;

const FRAMES: usize = 240;

#[test]
fn scrolling_five_thousand_clips_runs_no_view_and_stays_inside_the_frame_budget() {
    let mut rig = Rig::themed(Theme::Light);
    // A concealed row keeps hover in state (its text shows under the pointer), so crossing one
    // rebuilds the view; the trace is of ordinary rows.
    let mut history = history_5000();
    history.entries.retain(|e| !e.is_secret());
    rig.rt
        .update(|s, _| crate::picker::scenes::clips_5000_with(s, history));
    rig.settle();
    assert!(rig.state().model.picker.clips.list.len() >= 5000);
    // Image and file rows load a thumbnail the first time they scroll into view, which
    // redraws once; pass over the whole list first so the trace measures scrolling itself.
    let (x, y) = (200.0, 300.0);
    rig.rt.pointer_move(x, y);
    for direction in [-1.0_f32, 1.0] {
        for _ in 0..400 {
            rig.rt.wheel(x, y, direction * 400.0);
            rig.rt.frame(0.016);
        }
        for _ in 0..400 {
            rig.settle();
            if rig
                .state()
                .model
                .picker
                .clips
                .media
                .as_ref()
                .is_none_or(|m| m.idle())
            {
                break;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }
    rig.rt.frame(0.016);
    let idle = VIEWS.get();
    for _ in 0..20 {
        rig.rt.frame(0.016);
    }
    assert_eq!(VIEWS.get(), idle, "an idle list runs no view");
    let views = VIEWS.get();
    let commits = rig.rt.stats.commits;
    let mut costs = Vec::with_capacity(FRAMES);
    for i in 0..FRAMES {
        let started = Instant::now();
        rig.rt.wheel(x, y, if i % 40 < 30 { -48.0 } else { 36.0 });
        rig.rt.frame(0.016);
        costs.push(started.elapsed().as_secs_f64() * 1000.0);
    }
    let ran = VIEWS.get() - views;
    costs.sort_by(f64::total_cmp);
    let p95 = costs[costs.len() * 95 / 100];
    let median = costs[costs.len() / 2];
    eprintln!(
        "picker scroll trace: {FRAMES} frames, {ran} view runs, {} window remounts, median {median:.2} ms, p95 {p95:.2} ms",
        rig.rt.stats.commits - commits
    );
    assert_eq!(ran, 0, "scrolling ran the picker's view {ran} times");
    let bound: f64 = std::env::var("KUDZU_SCROLL_BUDGET_MS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(if cfg!(debug_assertions) { 60.0 } else { 4.0 });
    assert!(
        p95 <= bound,
        "p95 frame cost {p95:.2} ms is over {bound} ms"
    );
}
