//! Insights screen (PLAN S64, INS-2/3/4). Uses the S02 mock library.

use super::{cmd, Rig};
use crate::main::state::{LibraryFilter, MainView};
use crate::shell::Route;
use chrono::{TimeZone, Utc};
use kudzu_app::settings::Theme;
use kudzu_core::{SnippetRecord, UsageRecord};
use rune::{Key, KeyEvent, Mods};

const HZ: f64 = 120.0;
const DT: f64 = 1.0 / HZ;

const TOLERANCE: f32 = 2.0;

fn seed_mock(rig: &mut Rig) {
    let sample = rig.fixtures.sample.library.clone();
    rig.kudzu
        .library
        .transact(move |lib| {
            *lib = sample;
            Ok(())
        })
        .unwrap();
}

fn mock() -> Rig {
    mock_themed(Theme::Dark)
}

fn mock_themed(theme: Theme) -> Rig {
    let mut rig = Rig::themed(true, theme);
    seed_mock(&mut rig);
    rig
}

fn open(rig: &mut Rig) {
    rig.rt.update(|st, _| st.navigate_to(MainView::Insights));
    rig.settle();
}

fn open_reduced(rig: &mut Rig) {
    rig.rt.set_system_reduced_motion(true);
    open(rig);
}

fn land(rig: &mut Rig) {
    for _ in 0..80 {
        rig.rt.frame(1.0 / 60.0);
    }
}

fn begin_arrival(rig: &mut Rig) {
    rig.rt.update(|st, _| st.navigate_to(MainView::Insights));
    rig.rt.settle_all();
    rig.rt.frame(0.0);
}

fn frames(rig: &mut Rig, n: usize) {
    for _ in 0..n {
        rig.rt.frame(DT);
    }
}

fn bar_delay_frames(index: usize) -> usize {
    ((0.060 + index as f64 * 0.040) / DT).ceil() as usize
}

fn text_on(rig: &Rig, needle: &str) -> bool {
    rig.rt
        .elements()
        .into_iter()
        .any(|e| e.text.as_deref() == Some(needle))
}

fn a11y(rig: &Rig) -> String {
    rig.rt.accessibility_json().to_string()
}

fn snippet_id(rig: &Rig, name: &str) -> uuid::Uuid {
    rig.library()
        .snippets
        .iter()
        .find(|s| s.name == name)
        .unwrap_or_else(|| panic!("no snippet named {name}"))
        .id
}

fn element(rig: &Rig, key: &str) -> rune::ElementInfo {
    rig.rt
        .elements()
        .into_iter()
        .find(|e| e.key.as_deref() == Some(key))
        .unwrap_or_else(|| panic!("`{key}` is not on screen"))
}

fn tile(rig: &Rig, i: usize) -> String {
    rig.rt.state.insights.tile_text(i)
}

/// INS-2: four tiles from `usage_by_id` / `totals` on the mock library.
#[test]
fn the_four_tiles_show_fixture_totals() {
    let mut rig = mock();
    open_reduced(&mut rig);
    assert_eq!(tile(&rig, 0), "471");
    assert_eq!(tile(&rig, 1), "27k");
    assert_eq!(tile(&rig, 2), "2.3 h");
    assert_eq!(tile(&rig, 3), "17");
    assert!(text_on(&rig, "Expansions"));
    assert!(text_on(&rig, "Characters saved"));
    assert!(text_on(&rig, "Time saved"));
    assert!(text_on(&rig, "Active snippets"));
    assert!(text_on(&rig, "12 snippets used"));
    assert!(text_on(&rig, "≈ 5.4k words"));
    assert!(text_on(&rig, "at 40 words per minute"));
    assert!(text_on(&rig, "1 turned off"));
}

/// INS-3: most used top 10, ranked by count, with relative bars.
#[test]
fn most_used_lists_the_top_ten_with_relative_bars() {
    let mut rig = mock();
    open_reduced(&mut rig);
    let tree = a11y(&rig);
    assert!(tree.contains("Most used"), "{tree}");
    for rank in 1..=10 {
        assert!(
            tree.contains(&format!("\"{rank}\"")),
            "missing rank {rank}: {tree}"
        );
        assert!(
            rig.rect(&format!("insights-top-{}", rank - 1)).is_some(),
            "missing row {rank}"
        );
        let bar = element(&rig, &format!("insights-bar-{}", rank - 1));
        assert!(bar.rect[2] > 0.0, "bar {rank} has no width: {:?}", bar.rect);
    }
    assert!(text_on(&rig, "Email signature"));
    assert!(text_on(&rig, "Git commit co-author"));
    assert!(!text_on(&rig, "Home address"));
}

/// INS-3: recent 6 by last-used, names and relative times.
#[test]
fn recent_lists_the_six_latest() {
    let mut rig = mock();
    open_reduced(&mut rig);
    for (i, name) in [
        "Email signature",
        "Meeting follow-up",
        "Out of office",
        "Today",
        "Current time",
        "Shrug",
    ]
    .into_iter()
    .enumerate()
    {
        let row = element(&rig, &format!("insights-recent-{i}"));
        assert_eq!(row.label.as_deref(), Some(name), "recent {i}");
    }
    assert!(text_on(&rig, "Just now"));
    assert!(!rig.rect("insights-recent-6").is_some());
}

/// INS-3: never-used enabled snippets that have a trigger, up to 18.
#[test]
fn never_used_lists_enabled_snippets_with_a_trigger() {
    let mut rig = mock();
    open_reduced(&mut rig);
    let tree = a11y(&rig);
    assert!(tree.contains("Never used 5"), "{tree}");
    for (i, name) in [
        "Thanks",
        "Calendar link",
        "Sign-up form",
        "Quote the selection",
        "Request ID",
    ]
    .into_iter()
    .enumerate()
    {
        let chip = element(&rig, &format!("insights-unused-{i}"));
        assert_eq!(chip.label.as_deref(), Some(name), "unused {i}");
    }
    assert!(!text_on(&rig, "Lorem ipsum"));
}

/// INS-3: a click goes to All with that snippet selected.
#[test]
fn a_click_opens_the_snippet_in_all() {
    let mut rig = mock();
    open_reduced(&mut rig);
    let id = snippet_id(&rig, "Email signature");
    rig.click_key("insights-top-0");
    assert_eq!(
        rig.rt.state.frame.view,
        MainView::Library(LibraryFilter::All)
    );
    assert_eq!(rig.rt.state.frame.selected_snippet, Some(id));

    open_reduced(&mut rig);
    let recent = snippet_id(&rig, "Meeting follow-up");
    rig.click_key("insights-recent-1");
    assert_eq!(rig.rt.state.frame.selected_snippet, Some(recent));

    open_reduced(&mut rig);
    let unused = snippet_id(&rig, "Thanks");
    rig.click_key("insights-unused-0");
    assert_eq!(rig.rt.state.frame.selected_snippet, Some(unused));
}

/// INS-3: empty most-used sketch and recent copy; never-used hides when empty.
#[test]
fn empty_states_show_the_sketch_and_copy() {
    let mut rig = mock();
    rig.kudzu
        .library
        .transact(|lib| {
            lib.usage.clear();
            Ok(())
        })
        .unwrap();
    rig.rt.update(|st, _| st.refresh_library_snapshot());
    open_reduced(&mut rig);
    assert!(text_on(&rig, "Expand a snippet and it'll show up here."));
    assert!(text_on(&rig, "Nothing yet."));
    assert!(rig.rect("insights-sketch-bar-0").is_some());
    assert!(a11y(&rig).contains("Never used"));

    for snippet in rig.library().snippets.iter() {
        if snippet.is_enabled && !snippet.triggers.is_empty() {
            let id = snippet.id;
            rig.kudzu
                .library
                .transact(move |lib| {
                    if let Some(row) = lib.snippets.iter_mut().find(|s| s.id == id) {
                        row.is_enabled = false;
                    }
                    Ok(())
                })
                .unwrap();
        }
    }
    rig.rt.update(|st, _| st.refresh_library_snapshot());
    open_reduced(&mut rig);
    assert!(rig.rect("insights-unused").is_none());
}

/// INS-4: WPM slider 10–150 step 5, default 40, saved at once; later changes are instant.
#[test]
fn the_wpm_slider_steps_persists_and_updates_time_saved() {
    let mut rig = mock();
    open_reduced(&mut rig);
    assert_eq!(rig.kudzu.settings.watch().get().typing_speed_wpm, 40);
    let tree = a11y(&rig);
    assert!(
        tree.contains("Typing speed in words per minute") && tree.contains("40 wpm"),
        "{tree}"
    );
    rig.rt.update(|_, tx| tx.focus("slider", true));
    rig.settle();
    rig.press(KeyEvent {
        key: Key::Right,
        mods: Mods::default(),
    });
    assert_eq!(rig.kudzu.settings.watch().get().typing_speed_wpm, 45);
    assert!(a11y(&rig).contains("45 wpm"));

    rig.rt.update(|st, _| {
        st.svc.settings.update(|s| s.typing_speed_wpm = 80).unwrap();
    });
    rig.settle();
    assert_eq!(tile(&rig, 2), "1.1 h");
    assert!(text_on(&rig, "at 80 words per minute"));
}

/// INS-4: count-up ends at 560 ms; each bar starts at 60 + i×40 ms and runs 480 ms.
#[test]
fn arrival_counts_up_over_560ms_and_later_wpm_is_instant() {
    let mut rig = mock();
    begin_arrival(&mut rig);
    assert_eq!(tile(&rig, 0), "0");
    assert!(!rig.rt.state.insights.bar_started(0));
    let bar0 = element(&rig, "insights-bar-0");
    assert!(
        bar0.rect[2] < 1.0,
        "bar 0 waits 60 ms before growing: {:?}",
        bar0.rect
    );

    let mut elapsed_frames = 0usize;
    for i in 0..10 {
        let delay = bar_delay_frames(i);
        frames(&mut rig, delay - elapsed_frames - 1);
        elapsed_frames = delay - 1;
        assert!(
            !rig.rt.state.insights.bar_started(i),
            "bar {i} still waits just before {} ms",
            60 + i * 40
        );
        frames(&mut rig, 1);
        elapsed_frames += 1;
        assert!(
            rig.rt.state.insights.bar_started(i),
            "bar {i} starts at {} ms",
            60 + i * 40
        );
        if i == 0 {
            let started = element(&rig, "insights-bar-0");
            assert!(
                started.target[2] > 8.0,
                "bar 0 targets its width once started: {:?}",
                started.target
            );
            let mid = tile(&rig, 0);
            assert_ne!(mid, "0");
            assert_ne!(mid, "471", "count-up is mid-flight at 60 ms: {mid}");
        }
    }

    let before_end = ((0.560 - 1e-9) / DT).floor() as usize;
    frames(&mut rig, before_end - elapsed_frames);
    elapsed_frames = before_end;
    assert!(
        rig.rt.state.insights.arriving(),
        "count-up still running at {} ms",
        (elapsed_frames as f64 * DT * 1000.0).round()
    );
    frames(&mut rig, 1);
    elapsed_frames += 1;
    assert!(rig.rt.state.insights.landed(), "count-up ends at 560 ms");
    assert_eq!(tile(&rig, 0), "471", "count-up ends at 560 ms");
    assert_eq!(tile(&rig, 2), "2.3 h");

    let bar0_start = bar_delay_frames(0);
    let after_bar0 = bar0_start + (0.480 / DT).round() as usize;
    if elapsed_frames < after_bar0 {
        frames(&mut rig, after_bar0 - elapsed_frames);
    }
    let track = element(&rig, "insights-bar-track-0");
    let settled = element(&rig, "insights-bar-0");
    assert!(
        (settled.rect[2] - track.rect[2]).abs() < track.rect[2] * 0.03,
        "bar 0 lasts 480 ms: {:?} vs track {:?}",
        settled.rect,
        track.rect
    );

    land(&mut rig);
    let counts = [129.0, 75.0, 53.0, 41.0, 33.0, 28.0, 24.0, 21.0, 19.0, 17.0];
    let max = 129.0f32;
    for (i, count) in counts.iter().enumerate() {
        let bar = element(&rig, &format!("insights-bar-{i}"));
        let track = element(&rig, &format!("insights-bar-track-{i}"));
        let pct = (*count as f32 / max).clamp(0.03, 1.0);
        let want = track.rect[2] * pct;
        assert!(
            (bar.rect[2] - want).abs() <= 2.0,
            "bar {i} proportional: {:?} want {want} (pct {pct})",
            bar.rect
        );
    }
    let landed0 = element(&rig, "insights-bar-0").rect[2];

    rig.rt.update(|st, _| {
        st.svc.settings.update(|s| s.typing_speed_wpm = 80).unwrap();
    });
    rig.rt.frame(0.0);
    assert_eq!(tile(&rig, 2), "1.1 h");
    let after = element(&rig, "insights-bar-0");
    assert!(
        (after.rect[2] - landed0).abs() < 2.0,
        "later changes leave bars in place: {:?}",
        after.rect
    );
}

/// INS-4: Reduce Motion skips count-up and bar stagger.
#[test]
fn reduce_motion_skips_count_up_and_bars() {
    let mut rig = mock();
    open_reduced(&mut rig);
    assert_eq!(tile(&rig, 0), "471");
    let bar0 = element(&rig, "insights-bar-0");
    assert!(bar0.rect[2] > 20.0, "bar is full at once: {:?}", bar0.rect);
}

/// Layout: at most 880 wide; columns 1.4 : 1. Same landmarks in both themes.
#[test]
fn the_page_is_at_most_880_with_one_point_four_to_one_columns() {
    for theme in [Theme::Dark, Theme::Light] {
        let mut rig = mock_themed(theme);
        open_reduced(&mut rig);
        assert_content_landmarks(&rig, theme);
    }
}

fn assert_rect(rig: &Rig, key: &str, expected: [f32; 4], theme: Theme) {
    let got = element(rig, key).rect;
    let ok = (0..4).all(|i| (got[i] - expected[i]).abs() <= TOLERANCE);
    assert!(
        ok,
        "{theme:?} {key} {:?}, want {expected:?}\n{}",
        got,
        dump_landmarks(rig)
    );
}

fn dump_landmarks(rig: &Rig) -> String {
    [
        "insights-page",
        "insights-heading",
        "insights-tile-0",
        "insights-most-used",
        "insights-top-0",
        "insights-top-9",
        "insights-recent",
        "insights-recent-0",
        "insights-recent-5",
        "insights-unused",
        "insights-unused-0",
        "insights-unused-4",
    ]
    .iter()
    .map(|key| format!("{key}: {:?}", element(rig, key).rect))
    .collect::<Vec<_>>()
    .join("\n")
}

fn assert_content_landmarks(rig: &Rig, theme: Theme) {
    let dump = dump_landmarks(rig);
    let top0 = element(rig, "insights-top-0");
    assert!(
        (top0.rect[3] - 42.0).abs() <= TOLERANCE,
        "{theme:?} most-used row 0 is 42 pt: {:?}\n{dump}",
        top0.rect
    );
    let top9 = element(rig, "insights-top-9");
    assert!(
        (top9.rect[3] - 42.0).abs() <= TOLERANCE,
        "{theme:?} most-used row 9 is 42 pt: {:?}\n{dump}",
        top9.rect
    );
    let recent0 = element(rig, "insights-recent-0");
    assert!(
        (recent0.rect[3] - 34.0).abs() <= TOLERANCE,
        "{theme:?} recent row 0 is 34 pt: {:?}\n{dump}",
        recent0.rect
    );
    let recent5 = element(rig, "insights-recent-5");
    assert!(
        (recent5.rect[3] - 34.0).abs() <= TOLERANCE,
        "{theme:?} recent row 5 is 34 pt: {:?}\n{dump}",
        recent5.rect
    );
    assert_rect(rig, "insights-heading", [248.0, 64.0, 50.0, 20.0], theme);
    assert_rect(rig, "insights-page", [258.0, 99.0, 880.0, 814.0], theme);
    assert_rect(rig, "insights-tile-0", [290.0, 131.0, 195.0, 102.0], theme);
    assert_rect(
        rig,
        "insights-most-used",
        [290.0, 265.0, 462.0, 470.0],
        theme,
    );
    assert_rect(rig, "insights-top-0", [290.0, 297.0, 462.0, 42.0], theme);
    assert_rect(rig, "insights-top-9", [290.0, 693.0, 462.0, 42.0], theme);
    assert_rect(rig, "insights-recent", [776.0, 265.0, 330.0, 246.0], theme);
    assert_rect(rig, "insights-recent-0", [776.0, 297.0, 330.0, 34.0], theme);
    assert_rect(rig, "insights-recent-5", [776.0, 477.0, 330.0, 34.0], theme);
    assert_rect(rig, "insights-unused", [776.0, 535.0, 330.0, 118.0], theme);
    assert_rect(rig, "insights-unused-0", [776.0, 567.0, 58.0, 24.0], theme);
    assert_rect(rig, "insights-unused-4", [909.0, 597.0, 78.0, 24.0], theme);
}

/// Leaving Insights and coming back replays the arrival.
#[test]
fn a_return_visit_replays_the_count_up() {
    let mut rig = mock();
    open(&mut rig);
    land(&mut rig);
    assert_eq!(tile(&rig, 0), "471");
    rig.press(cmd('1'));
    assert_eq!(
        rig.rt.state.frame.view,
        MainView::Library(LibraryFilter::All)
    );
    open(&mut rig);
    assert_ne!(tile(&rig, 0), "471");
}

/// ⌘4 is the Insights route (S41 already wired this; keep it on the screen).
#[test]
fn command_4_opens_insights() {
    let mut rig = Rig::new(true);
    rig.press(cmd('4'));
    assert_eq!(rig.rt.state.frame.view, MainView::Insights);
    rig.open_route(Route::Insights);
    assert_eq!(rig.rt.state.frame.view, MainView::Insights);
}

/// `finish_arrival` lands on final numbers so a later S73 PNG is not a mid-count-up frame.
#[test]
fn the_insights_scene_finishes_arrival() {
    let mut rig = mock();
    rig.rt.update(|st, _| {
        st.navigate_to(MainView::Insights);
        st.insights.finish_arrival();
    });
    rig.settle();
    assert_eq!(rig.rt.state.frame.view, MainView::Insights);
    assert_eq!(tile(&rig, 0), "471");
    assert_eq!(tile(&rig, 2), "2.3 h");
    let bar0 = element(&rig, "insights-bar-0");
    assert!(bar0.rect[2] > 20.0, "scene bars are full: {:?}", bar0.rect);
}

/// M1: after arrival, a new expansion and an enablement change refresh the tiles.
#[test]
fn landed_tiles_refresh_when_usage_and_enablement_change() {
    let mut rig = mock();
    rig.rt.update(|st, _| {
        st.navigate_to(MainView::Insights);
        st.insights.finish_arrival();
    });
    rig.settle();
    assert_eq!(tile(&rig, 0), "471");
    assert_eq!(tile(&rig, 3), "17");

    rig.kudzu
        .library
        .transact(|lib| {
            lib.usage[0].count += 10;
            if let Some(row) = lib.snippets.iter_mut().find(|s| s.name == "Thanks") {
                row.is_enabled = false;
            }
            Ok(())
        })
        .unwrap();
    rig.rt.update(|st, _| st.refresh_library_snapshot());
    rig.settle();
    assert_eq!(tile(&rig, 0), "481");
    assert_eq!(tile(&rig, 3), "16");
}

/// m5: heading is the eligible total; at most 18 chips.
#[test]
fn never_used_shows_the_total_and_at_most_eighteen_chips() {
    let mut rig = mock();
    rig.kudzu
        .library
        .transact(|lib| {
            for i in 0..20 {
                lib.snippets.push(SnippetRecord::new(
                    format!("Spare {i}"),
                    vec![format!(";spare{i}")],
                    "x",
                ));
            }
            Ok(())
        })
        .unwrap();
    rig.rt.update(|st, _| st.refresh_library_snapshot());
    open_reduced(&mut rig);
    let tree = a11y(&rig);
    assert!(tree.contains("Never used 25"), "{tree}");
    assert!(element(&rig, "insights-unused-17").rect[2] > 0.0);
    assert!(rig.rect("insights-unused-18").is_none());
}

/// m4: calendar fallback uses the injected offset; both sides of midnight and the 30-day cut.
#[test]
fn calendar_fallback_uses_the_injected_offset_across_midnight() {
    let west_of_midnight = 1_791_775_800_000;
    let east_of_midnight = 1_791_779_400_000;
    let exactly_thirty_days = 1_791_806_400_000;
    let just_inside_thirty = 1_791_806_400_001;

    assert_calendar(&mut mock(), west_of_midnight, -240, "Oct 11, 2026");
    assert_calendar(&mut mock(), east_of_midnight, -240, "Oct 12, 2026");
    assert_calendar(&mut mock(), exactly_thirty_days, 0, "Oct 12, 2026");
    assert_calendar(&mut mock(), just_inside_thirty, 0, "30 days ago");
}

fn assert_calendar(rig: &mut Rig, at_ms: i64, offset: i32, want: &str) {
    let id = snippet_id(rig, "Email signature");
    let at = Utc.timestamp_millis_opt(at_ms).single().unwrap();
    rig.kudzu
        .library
        .transact(move |lib| {
            lib.usage.clear();
            lib.usage.push(UsageRecord {
                snippet_id: id,
                device_id: "test".into(),
                count: 1,
                chars_saved: 10,
                last_used_at: Some(at),
            });
            Ok(())
        })
        .unwrap();
    rig.rt.update(|st, _| {
        st.refresh_library_snapshot();
        st.insights.set_offset_minutes(offset);
        st.navigate_to(MainView::Insights);
        st.insights.finish_arrival();
    });
    rig.settle();
    assert!(
        text_on(rig, want),
        "offset {offset} at {at_ms} should show {want}: {}",
        a11y(rig)
    );
}
