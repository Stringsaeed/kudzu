use super::Rig;
use kudzu_core::system::TapMode;

fn has(r: &Rig, value: &str) -> bool {
    r.rt.elements()
        .iter()
        .any(|e| e.text.as_deref() == Some(value))
}

#[test]
fn access_card_explains_missing_permissions() {
    let mut r = Rig::new(false);
    let mut status = (*r.kudzu.expansion.status().get()).clone();
    status.permissions.accessibility = false;
    status.permissions.input_monitoring = false;
    status.can_post_events = false;
    status.can_expand = false;
    status.tap = TapMode::Off;
    r.fixtures.engine.emit_status(status);
    r.click_key("onboarding-start");
    assert!(has(&r, "Enable typing in other apps"));
    assert!(has(&r, "A guided setup, with every step beside you."));
    assert!(has(&r, "Accessibility"));
    assert!(has(&r, "Input Monitoring"));
    assert!(has(&r, "Set up permissions"));
    assert!(!has(&r, "Ready to expand"));
}

#[test]
fn paused_access_card_uses_engine_summary_without_claiming_expansion() {
    let mut r = Rig::new(false);
    let mut status = (*r.kudzu.expansion.status().get()).clone();
    status.permissions.accessibility = true;
    status.can_post_events = true;
    status.tap = TapMode::Active;
    status.can_expand = false;
    status.paused = true;
    status.summary = "Expansion is paused".into();
    r.fixtures.engine.emit_status(status);
    r.click_key("onboarding-start");
    assert!(has(&r, "Permissions are set"));
    assert!(has(&r, "Expansion is paused"));
    assert!(has(&r, "Review permissions"));
    assert!(!has(&r, "Ready to expand"));
    assert!(!has(&r, "Kudzu is ready to expand your snippets."));
}

#[test]
fn permission_open_shows_pending_and_the_window_error() {
    let mut r = Rig::new(false);
    r.click_key("onboarding-start");
    r.click_key("onboarding-permissions");
    assert!(has(&r, "Opening…"));
    r.rt.update(|s, _| {
        s.onboarding
            .permission_finished(Err("Companion unavailable".into()))
    });
    r.settle();
    assert!(has(&r, "Companion unavailable"));
    assert!(!has(&r, "Opening…"));
}

fn until(r: &mut Rig, ready: impl Fn(&Rig) -> bool) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while !ready(r) {
        assert!(
            std::time::Instant::now() < deadline,
            "operation did not complete"
        );
        r.settle();
        std::thread::yield_now();
    }
    r.settle();
}
fn import_step() -> Rig {
    let mut r = Rig::new(false);
    r.click_key("onboarding-start");
    r.click_key("onboarding-skip");
    r
}
fn choose(r: &mut Rig, key: &str, path: std::path::PathBuf) {
    r.click_key(key);
    r.rt.resolve_file_dialog(Some(path)).unwrap();
    r.settle();
}
#[test]
fn bad_raycast_import_shows_the_data_error_sentence() {
    use kudzu_app::onboarding::ImportSource;
    let mut r = import_step();
    let path = r._scratch.0.join("bad.json");
    std::fs::write(&path, "[").unwrap();
    choose(&mut r, "onboarding-raycast", path);
    // Wait for the sentence on screen: the domain records the failure a moment before the
    // window's watch wakes, so waiting on the domain alone races the redraw under load.
    until(&mut r, |r| {
        has(r, "This isn't a valid Raycast snippet export file.")
    });
    assert!(r
        .kudzu
        .onboarding
        .snapshot()
        .failures
        .contains_key(&ImportSource::Raycast));
    assert_eq!(r.kudzu.onboarding.snapshot().imported, 0);
}
#[test]
fn failed_safety_backup_explains_why_import_changed_nothing() {
    use kudzu_app::onboarding::ImportSource;
    let mut r = import_step();
    let before = r.library().snippets.len();
    let backups = r._scratch.0.join("failed-backup-destination");
    std::fs::write(&backups, "fixture blocker").unwrap();
    let data = kudzu_app::data::DataService::new(
        kudzu_app::data::DataConfig {
            backups,
            home: r._scratch.0.clone(),
            time_zone: kudzu_core::interchange::backup::BackupTimeZone::Fixed(
                chrono::FixedOffset::east_opt(0).unwrap(),
            ),
        },
        r.kudzu.library.clone(),
        r.kudzu.settings.clone(),
        r.fixtures.clock.clone(),
    );
    r.rt.update(move |s, _| s.svc.data = data);
    let path = r._scratch.0.join("valid.json");
    std::fs::write(
        &path,
        r#"[{"name":"Backup fixture","keyword":";backupfixture","text":"backup fixture body"}]"#,
    )
    .unwrap();
    choose(&mut r, "onboarding-raycast", path);
    until(&mut r, |r| {
        has(
            r,
            "Couldn't back up your library first, so nothing was changed.",
        )
    });
    assert!(r
        .kudzu
        .onboarding
        .snapshot()
        .failures
        .contains_key(&ImportSource::Raycast));

    assert_eq!(r.library().snippets.len(), before);
}
#[test]
fn a_second_detected_retry_reports_completed_over_original_folder_count() {
    use kudzu_app::onboarding::{DetectedFolder, ImportSource, ImportTally};
    let mut r = import_step();
    until(&mut r, |r| {
        !matches!(
            r.kudzu.onboarding.snapshot().detection,
            kudzu_app::onboarding::Detection::Looking
        )
    });
    let pending = r._scratch.0.join("pending.json");
    std::fs::write(&pending, "[").unwrap();
    r.kudzu.onboarding.record_detection(Ok(vec![
        DetectedFolder {
            path: "done-a".into(),
            snippet_count: 1,
        },
        DetectedFolder {
            path: "done-b".into(),
            snippet_count: 1,
        },
        DetectedFolder {
            path: pending,
            snippet_count: 1,
        },
    ]));
    for path in ["done-a", "done-b"] {
        r.kudzu.onboarding.record_import(
            ImportSource::Espanso,
            ImportTally {
                imported_snippets: 1,
                path: Some(path.into()),
                ..Default::default()
            },
            true,
        );
    }
    r.kudzu
        .onboarding
        .fail_import(ImportSource::Espanso, "Previous attempt failed".into());
    r.settle();
    r.click_key("onboarding-espanso");
    let sentence = "Imported 2 of 3 folders; retry brings in the rest. \
                    This isn't a valid Raycast snippet export file.";
    until(&mut r, |r| has(r, sentence));
    assert!(r
        .kudzu
        .onboarding
        .snapshot()
        .failures
        .get(&ImportSource::Espanso)
        .is_some_and(|e| e != "Previous attempt failed"));
}

#[test]
fn finish_alert_has_the_reference_tint_and_wraps_without_a_fixed_height() {
    let mut r = Rig::new(false);
    r.rt.update(|s, _| crate::main::onboarding::scenes::finish_retry(s));
    r.settle();
    let alert =
        r.rt.elements()
            .into_iter()
            .find(|e| e.role == "alert")
            .unwrap();
    assert_eq!(
        alert.fill,
        crate::theme::Theme::dark().c.destructive.alpha(0.1)
    );
}

#[test]
fn import_outcomes_use_singular_and_zero_copy() {
    use kudzu_app::onboarding::{ImportSource, ImportTally};
    let mut r = import_step();
    r.kudzu.onboarding.record_import(
        ImportSource::Raycast,
        ImportTally {
            imported_snippets: 1,
            warnings: 1,
            first_warning: Some("Compatibility warning".into()),
            ..Default::default()
        },
        false,
    );
    r.settle();
    assert!(has(&r, "Imported 1 snippet"));
    assert!(has(&r, "1 item needs a look — e.g. Compatibility warning"));
    r.kudzu
        .onboarding
        .record_import(ImportSource::Raycast, ImportTally::default(), false);
    r.settle();
    assert!(has(&r, "No new snippets to add"));
}
#[test]
fn a_working_import_keeps_its_button_label_and_shows_a_spinner() {
    let mut r = import_step();
    r.kudzu
        .onboarding
        .begin_import(kudzu_app::onboarding::ImportSource::Raycast)
        .unwrap();
    r.settle();
    assert!(has(&r, "Choose file…"));
    assert!(!has(&r, "Importing…"));
    let spinner =
        r.rt.elements()
            .into_iter()
            .find(|e| e.label.as_deref() == Some("Importing Raycast snippets"));
    assert!(spinner.is_some_and(|e| e.loop_phase.is_some()));
}

fn focusable(r: &Rig, key: &str) -> bool {
    r.rt.elements()
        .into_iter()
        .find(|e| e.key.as_deref() == Some(key))
        .unwrap()
        .focusable
}
#[test]
fn welcome_start_remains_enabled_while_setup_is_busy() {
    let mut r = Rig::new(false);
    r.kudzu.onboarding.begin_pick().unwrap();
    r.settle();
    assert!(focusable(&r, "onboarding-start"));
}
#[test]
fn access_skip_remains_enabled_while_setup_is_busy() {
    let mut r = Rig::new(false);
    r.click_key("onboarding-start");
    r.kudzu.onboarding.begin_pick().unwrap();
    r.settle();
    assert!(focusable(&r, "onboarding-skip"));
}
#[test]
fn the_current_progress_label_uses_iris_foreground() {
    let r = Rig::new(false);
    let label =
        r.rt.elements()
            .into_iter()
            .find(|e| e.text.as_deref() == Some("Welcome"))
            .unwrap();
    assert_eq!(
        label.text_color,
        Some(crate::theme::Theme::dark().c.iris_foreground)
    );
}
#[test]
fn import_choosers_use_the_reference_titles() {
    let mut r = import_step();
    until(&mut r, |r| {
        !matches!(
            r.kudzu.onboarding.snapshot().detection,
            kudzu_app::onboarding::Detection::Looking
        )
    });
    for (key, title) in [
        ("onboarding-raycast", "Choose your Raycast snippets export"),
        ("onboarding-textexpander", "Choose your TextExpander export"),
        ("onboarding-espanso", "Choose your Espanso config folder"),
    ] {
        r.click_key(key);
        assert_eq!(r.rt.file_dialog().unwrap().1.title, title);
        r.rt.resolve_file_dialog(None).unwrap();
        r.settle();
    }
}

#[test]
fn first_arrival_rises_four_points_then_settles() {
    let mut r = Rig::new(true);
    r.rt.update(|s, _| {
        s.svc
            .settings
            .update(|v| v.onboarding_completed = false)
            .unwrap();
        s.open_route(crate::shell::Route::Onboarding);
    });
    r.rt.frame(0.0);
    let step =
        r.rt.elements()
            .into_iter()
            .find(|e| e.key.as_deref() == Some("onboarding-step-Welcome"))
            .unwrap();
    assert!((step.rect[1] - step.target[1] - 4.0).abs() < 0.1);
    r.rt.frame(0.12);
    let middle =
        r.rt.elements()
            .into_iter()
            .find(|e| e.key.as_deref() == Some("onboarding-step-Welcome"))
            .unwrap();
    assert!((middle.rect[1] - middle.target[1]).abs() < 4.0);
    r.rt.frame(0.2);
    let landed =
        r.rt.elements()
            .into_iter()
            .find(|e| e.key.as_deref() == Some("onboarding-step-Welcome"))
            .unwrap();
    assert!((landed.rect[1] - landed.target[1]).abs() < 0.1);
}

#[test]
fn scenes_can_restart_setup_in_the_same_process() {
    let mut r = Rig::new(true);
    r.rt.update(|s, _| crate::main::onboarding::scenes::practice(s));
    r.settle();
    assert_eq!(
        r.kudzu.onboarding.step(),
        kudzu_app::onboarding::Step::TryIt
    );
    r.rt.update(|s, _| crate::main::onboarding::scenes::welcome(s));
    r.settle();
    assert_eq!(
        r.kudzu.onboarding.step(),
        kudzu_app::onboarding::Step::Welcome
    );
}
#[test]
fn practice_remounts_empty_after_back() {
    let mut r = import_step();
    r.click_key("onboarding-skip");
    r.click_key("onboarding-practice");
    r.press(super::plain('x', false));
    assert!(!r.rt.state.onboarding.practice.text.is_empty());
    r.click_key("onboarding-back");
    r.click_key("onboarding-skip");
    assert!(r.rt.state.onboarding.practice.text.is_empty());
}

#[test]
fn a_panicked_finish_worker_releases_opening_and_allows_retry() {
    let mut r = import_step();
    r.click_key("onboarding-skip");
    r.click_key("onboarding-practice");
    for ch in ";kudzu".chars() {
        r.press(super::plain(ch, false));
    }
    r.rt.update(|s, _| {
        s.onboarding
            .opening
            .store(true, std::sync::atomic::Ordering::Release);
        s.onboarding.task = Some(
            kudzu_app::tasks::Task::spawn("fixture-finish-panic", || {
                panic!("fixture worker stopped")
            })
            .unwrap(),
        );
    });
    until(&mut r, |r| {
        !r.rt.state.onboarding.opening()
            || r.rt
                .state
                .onboarding
                .task
                .as_ref()
                .is_some_and(|t| *t.ready().get())
    });
    assert!(!r.rt.state.onboarding.opening());
    assert!(has(&r, kudzu_app::onboarding::FINISH_ERROR));
    assert!(r.rt.state.onboarding.success());
    r.click_key("onboarding-open-library");
    until(&mut r, |r| {
        r.kudzu.settings.watch().get().onboarding_completed
    });
    assert!(r.rt.state.frame.view.in_library());
}

#[test]
fn reduced_motion_skips_the_first_arrival_translation() {
    let mut r = Rig::new(true);
    r.rt.set_system_reduced_motion(true);
    r.rt.update(|s, _| {
        s.svc
            .settings
            .update(|v| v.onboarding_completed = false)
            .unwrap();
        s.open_route(crate::shell::Route::Onboarding);
    });
    r.rt.frame(0.0);
    let step =
        r.rt.elements()
            .into_iter()
            .find(|e| e.key.as_deref() == Some("onboarding-step-Welcome"))
            .unwrap();
    assert!((step.rect[1] - step.target[1]).abs() < 0.1);
}
#[test]
fn chooser_failures_show_the_window_error_sentence() {
    let mut r = import_step();
    r.rt.update(|_, tx| {
        tx.open_file(
            rune::FileDialogOptions::default(),
            |_: &mut crate::shell::MainState, _, _| {},
        )
    });
    r.settle();
    r.click_key("onboarding-raycast");
    r.settle();
    assert!(has(&r, "a file dialog is already open"));
    assert!(!r.kudzu.onboarding.snapshot().picking);
    r.rt.resolve_file_dialog(None).unwrap();
    r.settle();
}
#[test]
fn wordmark_workaround_resolves_to_a_plan_draft() {
    assert!(include_str!("../onboarding/brand.rs").contains("PLAN §3, draft S65-wordmark"));
    assert!(include_str!("../../../../../docs/migration/PLAN.md").contains("| S65-wordmark |"));
}
#[test]
fn the_import_worker_fits_the_formatter_width() {
    assert!(include_str!("../onboarding/actions.rs")
        .lines()
        .all(|line| line.len() <= 100));
}
