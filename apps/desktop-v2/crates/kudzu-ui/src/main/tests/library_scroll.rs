//! §1.7 "Virtual list scroll, 5,000 rows: 0 view commits during scroll". The main window on
//! `library-5000`, its `view` counted, wheeled down and back up over 600 frames at 120 Hz.
//! Rune remounts entering rows without running `view` (`CommitWhy::VIRTUAL`), so any view run
//! here is the app's doing: a poll that reports a change, or a row read that moves.
//!
//! Frame CPU is measured on the release binary (`--scene library-5000 --script --trace`); see
//! the S50 report. This test pins the commit half in CI.

use std::cell::Cell;
use std::sync::Arc;

use kudzu_app::fixtures::{hotkey_registrar::FakeHotkeys, Fixtures};
use kudzu_app::ports::HotkeyRegistrar;
use rune::{Cx, El, Env, Runtime};

use super::{build_kudzu, Scratch};
use crate::main::library::scenes;
use crate::shell::windows::{parts, Launch};
use crate::shell::{Identity, MainState};

thread_local! {
    static VIEWS: Cell<usize> = const { Cell::new(0) };
}

fn counted(s: &MainState, cx: &mut Cx<MainState>) -> El {
    VIEWS.with(|v| v.set(v.get() + 1));
    crate::main::view(s, cx)
}

fn views() -> usize {
    VIEWS.with(Cell::get)
}

#[test]
fn wheel_scrolling_five_thousand_rows_runs_no_view() {
    let scratch = Scratch::new();
    let fixtures = Fixtures::new().unwrap();
    let kudzu = build_kudzu(&scratch.0, &fixtures);
    kudzu
        .settings
        .update(|s| s.onboarding_completed = true)
        .unwrap();
    let hotkeys: Arc<dyn HotkeyRegistrar> = Arc::new(FakeHotkeys::default());
    let p = parts(
        &kudzu,
        Launch {
            identity: Identity {
                app_name: "Kudzu Next",
                dev: true,
            },
            native: false,
            hidden: false,
        },
        Some(hotkeys),
    );
    let mut rt = Runtime::new(p.main, counted, (1180.0, 760.0), Env::new(true, 1.0));
    rt.poll = Some(crate::shell::lifecycle::poll);
    rt.on_key = Some(crate::shell::lifecycle::on_key);
    rt.update(|st, _| scenes::library_5000(st));
    rt.settle_all();
    for _ in 0..30 {
        rt.frame(1.0 / 120.0);
    }
    let first_row = |rt: &Runtime<MainState>| {
        rt.elements()
            .into_iter()
            .filter(|e| e.role == "option" && e.rect[1] >= 138.0)
            .map(|e| e.label.unwrap_or_default())
            .next()
    };
    let top = first_row(&rt);
    assert_eq!(
        rt.state.library.listing(&rt.state).map(|l| l.ids.len()),
        Some(5_000)
    );

    let before = views();
    for i in 0..600 {
        let dy = if i < 300 { -160.0 } else { 160.0 };
        rt.wheel(380.0, 420.0, dy);
        rt.frame(1.0 / 120.0);
        if i == 299 {
            assert_ne!(first_row(&rt), top, "the wheel scrolled the list");
        }
    }
    assert_eq!(views() - before, 0, "a view ran while the list scrolled");
    let _ = kudzu.shutdown();
}
