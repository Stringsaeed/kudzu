use super::{plain, Rig};
use kudzu_app::onboarding::Step;

#[test]
fn walks_setup_with_back_skip_and_persistent_completion() {
    let mut r = Rig::new(false);
    r.click_key("onboarding-start");
    assert_eq!(r.kudzu.onboarding.step(), Step::Access);
    r.click_key("onboarding-back");
    assert_eq!(r.kudzu.onboarding.step(), Step::Welcome);
    r.click_key("onboarding-start");
    r.click_key("onboarding-skip");
    assert_eq!(r.kudzu.onboarding.step(), Step::Import);
    r.click_key("onboarding-skip");
    assert_eq!(r.kudzu.onboarding.step(), Step::TryIt);
    r.click_key("onboarding-finish");
    wait(&mut r, |r| {
        r.kudzu.settings.watch().get().onboarding_completed
    });
}

#[test]
fn practice_expands_case_insensitive_suffix_locally() {
    let mut r = Rig::new(false);
    r.click_key("onboarding-start");
    r.click_key("onboarding-skip");
    r.click_key("onboarding-skip");
    r.click_key("onboarding-practice");
    for ch in ";KUDZU".chars() {
        r.press(plain(ch, false));
    }
    assert!(r.rt.state.onboarding.practice.text == "Hello from Kudzu!");
    assert!(!r.kudzu.settings.watch().get().onboarding_completed);
    assert!(r.rect("onboarding-open-library").is_some());
}

#[test]
fn busy_import_blocks_navigation_and_cancel_restores_focus() {
    let mut r = Rig::new(false);
    r.click_key("onboarding-start");
    r.click_key("onboarding-skip");
    r.click_key("onboarding-raycast");
    assert!(r.kudzu.onboarding.snapshot().picking);
    assert_eq!(
        r.rt.file_dialog().unwrap().1.title,
        "Import Raycast snippets"
    );
    r.click_key("onboarding-back");
    assert_eq!(r.kudzu.onboarding.step(), Step::Import);
    r.rt.resolve_file_dialog(None).unwrap();
    r.settle();
    assert!(!r.kudzu.onboarding.snapshot().busy());
    assert_eq!(r.focused_key().as_deref(), Some("onboarding-raycast"));
    assert_eq!(r.kudzu.onboarding.snapshot().imported, 0);
}

#[test]
fn failed_finish_keeps_practice_and_library_recoverable() {
    let mut r = Rig::new(false);
    r.click_key("onboarding-start");
    r.click_key("onboarding-skip");
    r.click_key("onboarding-skip");
    r.kudzu.shutdown().durable.unwrap();
    r.click_key("onboarding-finish");
    wait(&mut r, |r| r.kudzu.onboarding.snapshot().error.is_some());
    assert_eq!(r.kudzu.onboarding.step(), Step::TryIt);
    assert!(!r.kudzu.settings.watch().get().onboarding_completed);
    assert_eq!(
        r.kudzu.onboarding.snapshot().error.as_deref(),
        Some(kudzu_app::onboarding::FINISH_ERROR)
    );
    assert!(r.rect("onboarding-finish").is_some());
}

#[track_caller]
fn wait(r: &mut Rig, ready: impl Fn(&Rig) -> bool) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    while !ready(r) {
        assert!(
            std::time::Instant::now() < deadline,
            "setup did not settle: working {:?}, picking {}, added {}, errors {}",
            r.kudzu.onboarding.snapshot().working,
            r.kudzu.onboarding.snapshot().picking,
            r.kudzu.onboarding.snapshot().imported,
            r.kudzu.onboarding.snapshot().failures.len()
        );
        std::thread::sleep(std::time::Duration::from_millis(5));
        r.settle();
    }
    r.settle();
}

#[test]
fn imports_through_the_real_data_service_and_continues_on_net_additions() {
    let mut r = Rig::new(false);
    let path = r._scratch.0.join("snippets.json");
    std::fs::write(
        &path,
        r#"[{"name":"Onboarding fixture","keyword":";onbfixture","text":"fixture"}]"#,
    )
    .unwrap();
    r.click_key("onboarding-start");
    r.click_key("onboarding-skip");
    r.click_key("onboarding-raycast");
    r.rt.resolve_file_dialog(Some(path)).unwrap();
    r.settle();
    wait(&mut r, |r| !r.kudzu.onboarding.snapshot().busy());
    assert_eq!(r.kudzu.onboarding.snapshot().imported, 1);
    r.click_key("onboarding-continue");
    assert_eq!(r.kudzu.onboarding.step(), Step::TryIt);
}

#[test]
fn access_continue_waits_for_real_readiness() {
    let mut r = Rig::new(false);
    let mut denied = r.kudzu.expansion.status().get().as_ref().clone();
    denied.can_post_events = false;
    denied.permissions.accessibility = false;
    denied.tap = kudzu_core::system::TapMode::Off;
    r.fixtures.engine.emit_status(denied);
    r.settle();
    r.click_key("onboarding-start");
    r.click_key("onboarding-continue");
    assert_eq!(r.kudzu.onboarding.step(), Step::Access);
    let mut status = r.kudzu.expansion.status().get().as_ref().clone();
    status.permissions.accessibility = true;
    status.can_post_events = true;
    status.can_expand = true;
    status.tap = kudzu_core::system::TapMode::Active;
    r.fixtures.engine.emit_status(status);
    r.settle();
    r.click_key("onboarding-continue");
    assert_eq!(r.kudzu.onboarding.step(), Step::Import);
}

#[test]
fn espanso_retry_keeps_partial_count_and_skips_completed_folders() {
    use kudzu_app::onboarding::{DetectedFolder, ImportSource};
    let mut r = Rig::new(false);
    r.click_key("onboarding-start");
    r.click_key("onboarding-skip");
    wait(&mut r, |r| {
        !matches!(
            r.kudzu.onboarding.snapshot().detection,
            kudzu_app::onboarding::Detection::Looking
        )
    });
    let first = r._scratch.0.join("work.yml");
    let second = r._scratch.0.join("personal.json");
    std::fs::write(
        &first,
        "matches:\n  - trigger: ';fixturework'\n    replace: 'fixture'\n",
    )
    .unwrap();
    std::fs::write(&second, "{").unwrap();
    r.kudzu.onboarding.record_detection(Ok(vec![
        DetectedFolder {
            path: first.clone(),
            snippet_count: 1,
        },
        DetectedFolder {
            path: second.clone(),
            snippet_count: 1,
        },
    ]));
    r.settle();
    r.click_key("onboarding-espanso");
    wait(&mut r, |r| {
        r.kudzu
            .onboarding
            .snapshot()
            .failures
            .contains_key(&ImportSource::Espanso)
    });
    assert_eq!(r.kudzu.onboarding.snapshot().imported, 1);
    assert_eq!(r.kudzu.onboarding.pending_espanso_folders().len(), 1);
    std::fs::remove_file(&first).unwrap();
    std::fs::write(
        &second,
        r#"[{"name":"Retry fixture","keyword":";fixturepersonal","text":"second fixture"}]"#,
    )
    .unwrap();
    r.click_key("onboarding-espanso");
    wait(&mut r, |r| !r.kudzu.onboarding.snapshot().busy());
    assert_eq!(r.kudzu.onboarding.snapshot().imported, 2);
    assert!(r.kudzu.onboarding.pending_espanso_folders().is_empty());
}

#[test]
fn reference_setup_regions_hold_in_both_themes() {
    use crate::main::onboarding::scenes;
    use kudzu_app::settings::Theme;
    type SetupRegion = (fn(&mut crate::shell::MainState), f32);
    let scenes: [SetupRegion; 7] = [
        (scenes::welcome, 573.0),
        (scenes::access, 552.0),
        (scenes::import, 676.0),
        (scenes::practice, 492.0),
        (scenes::espanso, 660.0),
        (scenes::espanso_retry, 724.0),
        (scenes::finish_retry, 564.0),
    ];
    let mut failures = Vec::new();
    for theme in [Theme::Light, Theme::Dark] {
        for (scene, height) in scenes {
            let mut r = Rig::themed(true, theme);
            r.rt.update(move |s, _| scene(s));
            r.settle();
            let rect = r.rect("onboarding-card").unwrap();
            let expected = [350.0, (760.0 - height) / 2.0, 480.0, height];
            if !rect.iter().zip(expected).all(|(a, b)| (a - b).abs() <= 2.0) {
                failures.push(format!("setup region {rect:?}, expected {expected:?}"));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
