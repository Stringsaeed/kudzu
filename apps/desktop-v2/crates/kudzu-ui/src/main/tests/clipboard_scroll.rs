//! The §1.7 scroll trace: 5,000 clips scrolled by the wheel. The screen's view must not run
//! while the list scrolls (rows hold no hover state, handlers say nothing to the view, `poll`
//! reports no change). Rune itself commits to shift the mounted window of a long
//! `virtual_rows` list; those commits remount rows and do not run `view`, so the trace counts
//! runs of the screen's view, not `Runtime` commits. A frame must also stay cheap.
//! `KUDZU_SCROLL_BUDGET_MS` sets the frame bound (the budget is 4 ms p95 in a release build;
//! the default here is loose for a debug build, and `cargo test --release` checks the budget).

use std::time::Instant;

use kudzu_app::settings::Theme;

use super::Rig;
use crate::main::clipboard::{scenes, VIEWS};

const FRAMES: usize = 240;

#[test]
fn scrolling_five_thousand_clips_runs_no_view_and_stays_inside_the_frame_budget() {
    let mut rig = Rig::themed(true, Theme::Light);
    rig.rt.update(|st, _| scenes::clips_5000(st));
    rig.settle();
    assert!(rig.rt.state.clipboard.clips.list.len() >= 5000);
    // The pointer rests on the list, as a user's does while scrolling it.
    let (x, y) = (300.0, 400.0);
    rig.rt.pointer_move(x, y);
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
        "scroll trace: {FRAMES} frames, {ran} view runs, {} window remounts, median {median:.2} ms, p95 {p95:.2} ms",
        rig.rt.stats.commits - commits
    );
    assert_eq!(ran, 0, "scrolling ran the screen's view {ran} times");
    let bound: f64 = std::env::var("KUDZU_SCROLL_BUDGET_MS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(if cfg!(debug_assertions) { 60.0 } else { 4.0 });
    assert!(
        p95 <= bound,
        "p95 frame cost {p95:.2} ms is over {bound} ms"
    );
}
