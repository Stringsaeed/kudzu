use super::*;
use crate::fixtures::clock::FakeClock;
use crate::fixtures::engine::FakeEngine;
use crate::persist::{PersistError, Revision, WriteTarget, Writer};
use crate::settings::{Settings, SettingsService, Variant};
use kudzu_core::system::{EngineStatus, PermissionStatus, TapMode};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

struct Memory;
impl WriteTarget<Settings> for Memory {
    fn write(&self, _: &Settings) -> Result<(), PersistError> {
        Ok(())
    }
}

struct Harness {
    onboarding: Onboarding,
    settings: Arc<SettingsService>,
    engine: Arc<FakeEngine>,
    writer: Writer,
}

impl Harness {
    fn new() -> Self {
        let clock = Arc::new(FakeClock::default());
        let writer = Writer::new(clock, Duration::from_secs(30), Duration::from_secs(1)).unwrap();
        let persisted = writer.register(Memory, Revision(0)).unwrap();
        let settings =
            Arc::new(SettingsService::new(Settings::for_variant(Variant::Dev), persisted).unwrap());
        let engine = Arc::new(FakeEngine::default());
        let onboarding = Onboarding::new(settings.clone(), engine.clone());
        Self {
            onboarding,
            settings,
            engine,
            writer,
        }
    }

    fn grant(&self) {
        self.engine.state.lock().unwrap().status = EngineStatus {
            can_post_events: true,
            can_expand: true,
            paused: false,
            tap: TapMode::Active,
            permissions: PermissionStatus {
                accessibility: true,
                input_monitoring: true,
            },
            secure_input: false,
            secure_input_owner: None,
            summary: "Ready".into(),
        };
    }
}

impl Drop for Harness {
    fn drop(&mut self) {
        let _ = self.writer.shutdown();
    }
}

fn tally(imported: usize, merged: usize, path: Option<&str>) -> ImportTally {
    ImportTally {
        imported_snippets: imported,
        merged_snippets: merged,
        warnings: 0,
        first_warning: None,
        path: path.map(PathBuf::from),
    }
}

#[test]
fn walks_welcome_access_import_try_it_with_back_and_skip() {
    let h = Harness::new();
    assert_eq!(h.onboarding.step(), Step::Welcome);
    h.onboarding.advance().unwrap();
    assert_eq!(h.onboarding.step(), Step::Access);
    assert_eq!(h.onboarding.advance(), Err(OnboardingError::NotReady));
    h.onboarding.skip().unwrap();
    assert_eq!(h.onboarding.step(), Step::Import);
    assert_eq!(h.onboarding.snapshot().direction, 1);
    h.onboarding.back().unwrap();
    assert_eq!(h.onboarding.step(), Step::Access);
    assert_eq!(h.onboarding.snapshot().direction, -1);
    h.grant();
    h.onboarding.advance().unwrap();
    assert_eq!(h.onboarding.step(), Step::Import);
    assert_eq!(h.onboarding.advance(), Err(OnboardingError::NotReady));
    h.onboarding.skip().unwrap();
    assert_eq!(h.onboarding.step(), Step::TryIt);
    h.onboarding.back().unwrap();
    assert_eq!(h.onboarding.step(), Step::Import);
}

#[test]
fn detection_covers_looking_found_not_found_and_failed() {
    let h = Harness::new();
    assert_eq!(h.onboarding.snapshot().detection, Detection::Looking);
    h.onboarding.record_detection(Ok(Vec::new()));
    assert_eq!(h.onboarding.snapshot().detection, Detection::NotFound);
    h.onboarding.record_detection(Err(()));
    assert_eq!(h.onboarding.snapshot().detection, Detection::Failed);
    h.onboarding.record_detection(Ok(vec![DetectedFolder {
        path: PathBuf::from("/espanso"),
        snippet_count: 4,
    }]));
    assert!(matches!(
        h.onboarding.snapshot().detection,
        Detection::Found { ref folders } if folders.len() == 1
    ));
}

#[test]
fn retry_skips_imported_folders_and_keeps_partial_counts() {
    let h = Harness::new();
    h.onboarding.record_detection(Ok(vec![
        DetectedFolder {
            path: PathBuf::from("/one"),
            snippet_count: 3,
        },
        DetectedFolder {
            path: PathBuf::from("/two"),
            snippet_count: 5,
        },
    ]));
    h.onboarding.begin_import(ImportSource::Espanso).unwrap();
    h.onboarding
        .record_import(ImportSource::Espanso, tally(3, 1, Some("/one")), true);
    assert!(h.onboarding.snapshot().busy());
    h.onboarding.fail_import(
        ImportSource::Espanso,
        "Imported 1 of 2 folders; retry brings in the rest. disk".into(),
    );
    assert_eq!(h.onboarding.snapshot().imported, 2);
    assert_eq!(
        h.onboarding.snapshot().outcomes[&ImportSource::Espanso].count,
        2
    );
    assert_eq!(
        h.onboarding.pending_espanso_folders(),
        [DetectedFolder {
            path: PathBuf::from("/two"),
            snippet_count: 5,
        }]
    );
    h.onboarding.begin_import(ImportSource::Espanso).unwrap();
    h.onboarding
        .record_import(ImportSource::Espanso, tally(5, 0, Some("/two")), true);
    h.onboarding.end_import(ImportSource::Espanso);
    assert_eq!(h.onboarding.snapshot().imported, 7);
    assert_eq!(
        h.onboarding.snapshot().outcomes[&ImportSource::Espanso].count,
        7
    );
    assert!(h.onboarding.pending_espanso_folders().is_empty());
}

#[test]
fn continue_requires_net_additions() {
    let h = Harness::new();
    h.onboarding.skip().unwrap();
    h.onboarding.skip().unwrap();
    h.onboarding.record_import(
        ImportSource::Raycast,
        ImportTally {
            imported_snippets: 4,
            merged_snippets: 4,
            warnings: 1,
            first_warning: Some("kept as text".into()),
            path: None,
        },
        false,
    );
    assert_eq!(h.onboarding.snapshot().imported, 0);
    assert!(!h.onboarding.can_continue());
    assert_eq!(h.onboarding.advance(), Err(OnboardingError::NotReady));
    h.onboarding
        .record_import(ImportSource::TextExpander, tally(2, 0, None), false);
    assert_eq!(h.onboarding.snapshot().imported, 2);
    assert!(h.onboarding.can_continue());
    h.onboarding.advance().unwrap();
    assert_eq!(h.onboarding.step(), Step::TryIt);
}

#[test]
fn a_failed_finish_stays_recoverable() {
    let h = Harness::new();
    h.onboarding.skip().unwrap();
    h.onboarding.skip().unwrap();
    h.onboarding.skip().unwrap();
    assert_eq!(h.onboarding.step(), Step::TryIt);
    h.writer.shutdown().unwrap();
    let error = h.onboarding.finish().unwrap_err();
    assert_eq!(error, OnboardingError::Finish);
    assert_eq!(error.to_string(), FINISH_ERROR);
    assert!(!h.settings.watch().get().onboarding_completed);
    assert_eq!(h.onboarding.step(), Step::TryIt);
    assert_eq!(h.onboarding.snapshot().error.as_deref(), Some(FINISH_ERROR));
    assert!(!h.onboarding.snapshot().finishing);
}

#[test]
fn finish_records_completion() {
    let h = Harness::new();
    h.onboarding.skip().unwrap();
    h.onboarding.skip().unwrap();
    h.onboarding.skip().unwrap();
    h.onboarding.finish().unwrap();
    assert!(h.settings.watch().get().onboarding_completed);
}

#[test]
fn busy_guards_block_navigation() {
    let h = Harness::new();
    h.onboarding.begin_import(ImportSource::Espanso).unwrap();
    assert_eq!(h.onboarding.advance(), Err(OnboardingError::Busy));
    assert_eq!(h.onboarding.back(), Err(OnboardingError::Busy));
    assert_eq!(h.onboarding.skip(), Err(OnboardingError::Busy));
}

#[test]
fn busy_holds_between_folders() {
    let h = Harness::new();
    h.onboarding.begin_import(ImportSource::Espanso).unwrap();
    assert!(h.onboarding.snapshot().busy());
    h.onboarding
        .record_import(ImportSource::Espanso, tally(3, 1, Some("/one")), true);
    assert!(h.onboarding.snapshot().busy());
    h.onboarding
        .record_import(ImportSource::Espanso, tally(5, 0, Some("/two")), true);
    assert!(h.onboarding.snapshot().busy());
    h.onboarding.end_import(ImportSource::Espanso);
    assert!(!h.onboarding.snapshot().busy());
}

#[test]
fn begin_import_takes_over_picking() {
    let h = Harness::new();
    h.onboarding.begin_pick().unwrap();
    assert!(h.onboarding.snapshot().picking);
    h.onboarding.begin_import(ImportSource::Espanso).unwrap();
    let snapshot = h.onboarding.snapshot();
    assert_eq!(snapshot.working, Some(ImportSource::Espanso));
    assert!(!snapshot.picking);
    assert_eq!(
        h.onboarding.begin_import(ImportSource::Raycast),
        Err(OnboardingError::Busy)
    );
}

#[test]
fn can_continue_matches_each_step() {
    let h = Harness::new();
    assert!(h.onboarding.can_continue());
    h.onboarding.advance().unwrap();
    assert_eq!(h.onboarding.step(), Step::Access);
    assert!(!h.onboarding.can_continue());
    h.grant();
    assert!(h.onboarding.can_continue());
}

#[test]
fn finish_does_not_reenter_while_finishing() {
    let h = Harness::new();
    h.onboarding.skip().unwrap();
    h.onboarding.skip().unwrap();
    h.onboarding.skip().unwrap();
    h.onboarding.edit(|live| live.snapshot.finishing = true);
    assert_eq!(h.onboarding.finish(), Err(OnboardingError::Busy));
    assert!(h.onboarding.snapshot().finishing);
    assert!(!h.settings.watch().get().onboarding_completed);
}

#[test]
fn navigation_keeps_a_finish_failure_until_a_new_finish_attempt() {
    let h = Harness::new();
    h.onboarding.skip().unwrap();
    h.onboarding.skip().unwrap();
    h.onboarding.skip().unwrap();
    h.writer.shutdown().unwrap();
    h.onboarding.finish().unwrap_err();
    h.onboarding.back().unwrap();
    assert_eq!(h.onboarding.snapshot().error.as_deref(), Some(FINISH_ERROR));
    h.onboarding.skip().unwrap();
    assert_eq!(h.onboarding.snapshot().error.as_deref(), Some(FINISH_ERROR));
}

#[test]
fn restarting_setup_clears_the_previous_session() {
    let h = Harness::new();
    h.onboarding.skip().unwrap();
    h.onboarding.skip().unwrap();
    h.onboarding
        .record_import(ImportSource::Espanso, tally(3, 0, Some("completed")), true);
    h.onboarding
        .fail_import(ImportSource::Espanso, "Fixture failure".into());
    h.onboarding.skip().unwrap();
    h.onboarding.restart().unwrap();
    let state = h.onboarding.snapshot();
    assert_eq!(state.step, Step::Welcome);
    assert_eq!(state.direction, 0);
    assert_eq!(state.detection, Detection::Looking);
    assert_eq!(state.imported, 0);
    assert!(state.outcomes.is_empty() && state.failures.is_empty() && state.error.is_none());
    h.onboarding.begin_pick().unwrap();
    assert_eq!(h.onboarding.restart(), Err(OnboardingError::Busy));
}

#[test]
fn a_failed_finish_worker_leaves_a_retryable_setup() {
    let h = Harness::new();
    h.onboarding.skip().unwrap();
    h.onboarding.skip().unwrap();
    h.onboarding.skip().unwrap();
    h.onboarding.fail_finish();
    assert_eq!(h.onboarding.snapshot().error.as_deref(), Some(FINISH_ERROR));
    assert!(!h.onboarding.snapshot().busy());
    h.onboarding.finish().unwrap();
    assert!(h.settings.watch().get().onboarding_completed);
}
