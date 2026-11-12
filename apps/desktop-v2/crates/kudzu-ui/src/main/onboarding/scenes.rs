//! Fixture-only scenes force setup despite the fixture's completed setting.
use crate::shell::{MainState, Route};
use kudzu_app::onboarding::{DetectedFolder, ImportSource, ImportTally};
pub fn welcome(s: &mut MainState) {
    s.svc.onboarding.restart().unwrap();
    s.onboarding = super::State::default();
    s.svc
        .settings
        .update(|v| v.onboarding_completed = false)
        .unwrap();
    s.open_route(Route::Onboarding);
    s.onboarding.scene_static = true;
    super::brand::ensure(s);
    s.onboarding.brand.as_ref().unwrap().wait();
    s.onboarding.detecting = true;
    s.svc.onboarding.record_detection(Ok(vec![]));
}
pub fn access(s: &mut MainState) {
    welcome(s);
    let _ = s.svc.onboarding.advance();
}
pub fn import(s: &mut MainState) {
    access(s);
    let _ = s.svc.onboarding.skip();
}
pub fn practice(s: &mut MainState) {
    import(s);
    let _ = s.svc.onboarding.skip();
    s.onboarding.practice.focused = true;
}
pub fn espanso(s: &mut MainState) {
    import(s);
    s.svc.onboarding.record_detection(Ok(vec![
        DetectedFolder {
            path: "fixture-work".into(),
            snippet_count: 3,
        },
        DetectedFolder {
            path: "fixture-personal".into(),
            snippet_count: 2,
        },
    ]));
}
pub fn espanso_retry(s: &mut MainState) {
    espanso(s);
    s.svc.onboarding.record_import(
        ImportSource::Espanso,
        ImportTally {
            imported_snippets: 3,
            path: Some("fixture-work".into()),
            ..Default::default()
        },
        true,
    );
    s.svc.onboarding.fail_import(
        ImportSource::Espanso,
        "Imported 1 of 2 folders; retry brings in the rest. Couldn’t read the personal folder."
            .into(),
    );
}
pub fn finish_retry(s: &mut MainState) {
    practice(s);
    s.onboarding.scene_finish_error = true;
}
