//! Insights screen (PLAN S64, INS-2/3/4). Uses the S02 mock library.

use super::{cmd, Rig};
use crate::main::state::{LibraryFilter, MainView};
use crate::shell::Route;
use kudzu_app::settings::Theme;
use rune::{Key, KeyEvent, Mods};

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

/// INS-4: count-up 560 ms quartic on arrival; a later WPM change snaps.
#[test]
fn arrival_counts_up_over_560ms_and_later_wpm_is_instant() {
    let mut rig = mock();
    open(&mut rig);
    let first = tile(&rig, 0);
    assert_ne!(first, "471", "first paint is still counting: {first}");
    let bar0 = element(&rig, "insights-bar-0");
    assert!(
        bar0.rect[2] < 8.0,
        "bar 0 waits 60 ms before growing: {:?}",
        bar0.rect
    );

    for _ in 0..8 {
        rig.rt.frame(1.0 / 60.0);
    }
    let mid = tile(&rig, 0);
    assert_ne!(mid, first);
    assert_ne!(mid, "471");
    let growing = element(&rig, "insights-bar-0");
    assert!(
        growing.target[2] > bar0.target[2],
        "bar 0 starts after its delay: {:?} then {:?}",
        bar0.target,
        growing.target
    );

    land(&mut rig);
    assert_eq!(tile(&rig, 0), "471");
    assert_eq!(tile(&rig, 2), "2.3 h");

    rig.rt.update(|st, _| {
        st.svc.settings.update(|s| s.typing_speed_wpm = 80).unwrap();
    });
    rig.rt.frame(0.0);
    assert_eq!(tile(&rig, 2), "1.1 h");
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

fn assert_content_landmarks(rig: &Rig, theme: Theme) {
    let page = element(rig, "insights-page");
    assert!(
        (page.rect[2] - 880.0).abs() <= TOLERANCE,
        "{theme:?} page {:?}",
        page.rect
    );
    let heading = element(rig, "insights-heading");
    assert!(
        (heading.rect[0] - 248.0).abs() <= TOLERANCE,
        "{theme:?} heading {:?}",
        heading.rect
    );
    assert!(
        (heading.rect[1] - 63.5).abs() <= TOLERANCE,
        "{theme:?} heading {:?}",
        heading.rect
    );
    let most = element(rig, "insights-most-used");
    let recent = element(rig, "insights-recent");
    assert!(
        (most.rect[0] - 290.0).abs() <= TOLERANCE,
        "{theme:?} most {:?}",
        most.rect
    );
    assert!(
        (most.rect[2] - 462.0).abs() <= TOLERANCE,
        "{theme:?} most width {:?}",
        most.rect
    );
    assert!(
        (recent.rect[0] - 776.0).abs() <= TOLERANCE,
        "{theme:?} recent {:?}",
        recent.rect
    );
    assert!(
        (recent.rect[2] - 330.0).abs() <= TOLERANCE,
        "{theme:?} recent width {:?}",
        recent.rect
    );
    for i in 0..4 {
        assert!(
            rig.rect(&format!("insights-tile-{i}")).is_some(),
            "{theme:?} missing tile {i}"
        );
    }
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
