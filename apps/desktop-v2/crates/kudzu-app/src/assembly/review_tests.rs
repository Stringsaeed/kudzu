//! Regressions from the S40 review: startup failures, the feed wait, durability before an
//! install, the trigger-lost route and relaunch ordering.
use super::bounds::{bounded, Timeouts};
use super::tests::{config, utc_time, wait_for, Hold, Scratch};
use super::*;
use crate::fixtures::{engine::FakeEngine, Fixtures};
use crate::persist::Revision;
use crate::ports::engine::AdmissionError;
use crate::ports::{EngineEvents, EngineLibrary, TriggerLost};
use kudzu_core::system::{AppInfo, EngineStatus};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Condvar, Mutex};
use std::time::Instant;

const SHORT: Timeouts = Timeouts {
    feed: Duration::from_millis(200),
    drain: Duration::from_millis(200),
};

/// The fake engine, with a reload that panics, or blocks until the started engine stops.
struct Stalled {
    inner: Arc<FakeEngine>,
    panic: bool,
    started: AtomicBool,
    released: (Mutex<bool>, Condvar),
}

impl Stalled {
    fn new(inner: Arc<FakeEngine>, panic: bool) -> Arc<Self> {
        Arc::new(Self {
            inner,
            panic,
            started: AtomicBool::new(false),
            released: (Mutex::new(false), Condvar::new()),
        })
    }
}

impl ExpansionEngine for Stalled {
    fn start(&self, events: Arc<dyn EngineEvents>) -> Result<(), crate::error::PortError> {
        self.started.store(true, Ordering::SeqCst);
        self.inner.start(events)
    }
    fn stop(&self) {
        // Composition stops any previous engine before `start`; that must not release it.
        if self.started.load(Ordering::SeqCst) {
            *self.released.0.lock().unwrap() = true;
            self.released.1.notify_all();
        }
        self.inner.stop();
    }
    fn reload(&self, _: EngineLibrary) {
        assert!(!self.panic, "the engine could not build its index");
        let mut released = self.released.0.lock().unwrap();
        while !*released {
            released = self.released.1.wait(released).unwrap();
        }
    }
    fn status(&self) -> EngineStatus {
        self.inner.status()
    }
    fn set_paused(&self, paused: bool) {
        self.inner.set_paused(paused);
    }
    fn set_suppressed(&self, suppressed: bool) {
        self.inner.set_suppressed(suppressed);
    }
    fn insert(
        &self,
        snippet: uuid::Uuid,
        target: Option<AppInfo>,
        selection: Option<String>,
    ) -> Result<(), AdmissionError> {
        self.inner.insert(snippet, target, selection)
    }
}

fn open_stalled(panic: bool) -> (Scratch, Fixtures, Result<Arc<Kudzu>, AssemblyError>) {
    let scratch = Scratch::new();
    let fixtures = Fixtures::new().unwrap();
    let mut config = config(&scratch, &fixtures, false, Duration::ZERO);
    config.host.ports.engine = Stalled::new(fixtures.engine.clone(), panic);
    let started = Instant::now();
    let result = Kudzu::open(config, SHORT);
    assert!(
        started.elapsed() < Duration::from_secs(4),
        "startup returned promptly"
    );
    (scratch, fixtures, result)
}

#[test]
fn a_feed_that_never_catches_up_is_a_startup_error_and_stops_the_engine() {
    let (_scratch, fixtures, result) = open_stalled(false);
    assert!(
        matches!(result, Err(AssemblyError::FeedTimeout)),
        "no windows open against it: {:?}",
        result.as_ref().err()
    );
    assert!(!fixtures.engine.state.lock().unwrap().running);
}

#[test]
fn a_feed_error_after_the_engine_starts_is_a_startup_error_and_stops_the_engine() {
    let (_scratch, fixtures, result) = open_stalled(true);
    assert!(matches!(
        result,
        Err(AssemblyError::Expansion(
            crate::expansion::ExpansionError::WorkerStopped
        ))
    ));
    assert!(!fixtures.engine.state.lock().unwrap().running);
}

fn hold() -> (Hold, Arc<(Mutex<bool>, Condvar)>) {
    let release = Arc::new((Mutex::new(false), Condvar::new()));
    let hold = Hold {
        entered: Arc::new((Mutex::new(false), Condvar::new())),
        release: release.clone(),
    };
    (hold, release)
}

#[test]
fn a_drain_that_times_out_skips_the_install_and_reports_it() {
    let scratch = Scratch::new();
    let fixtures = Fixtures::new().unwrap();
    let app = Kudzu::open(config(&scratch, &fixtures, false, Duration::ZERO), SHORT).unwrap();
    let (slow, release) = hold();
    app.writer()
        .register(slow, Revision(0))
        .unwrap()
        .submit(Revision(1), Arc::new(1))
        .unwrap();
    let outcome = app.shutdown();
    assert!(outcome.ran);
    assert_eq!(outcome.durable, Err(PersistError::Timeout));
    assert_eq!(app.installs(), 0, "no update installs over an unsaved quit");
    let again = app.shutdown();
    assert!(!again.ran);
    assert_eq!(again.durable, Err(PersistError::Timeout));
    *release.0.lock().unwrap() = true;
    release.1.notify_all();
}

#[test]
fn a_durable_drain_still_installs_on_quit() {
    let scratch = Scratch::new();
    let fixtures = Fixtures::new().unwrap();
    let app = Kudzu::new(config(&scratch, &fixtures, false, Duration::ZERO)).unwrap();
    assert_eq!(app.shutdown().durable, Ok(()));
    assert_eq!(app.installs(), 1);
}

#[test]
fn a_lost_trigger_reaches_expansion_first_then_activity_once() {
    let scratch = Scratch::new();
    let fixtures = Fixtures::new().unwrap();
    let app = Kudzu::new(config(&scratch, &fixtures, false, Duration::ZERO)).unwrap();
    let snippet = app.library.watch().get().snippets[0].id;
    let notice = app.expansion.lost_trigger_notice();
    let before = app.activity.watch().get().len();
    let ordered = Arc::new(AtomicBool::new(true));
    let seen = ordered.clone();
    let check = notice.clone();
    let _subscription = app.activity.watch().subscribe(move || {
        if check.get().is_none() {
            seen.store(false, Ordering::SeqCst);
        }
    });
    fixtures.engine.emit_trigger_lost(TriggerLost {
        snippet_id: snippet,
        trigger: "qq".into(),
    });
    wait_for(|| app.activity.watch().get().len() > before);
    std::thread::sleep(Duration::from_millis(50));
    assert_eq!(
        app.activity.watch().get().len(),
        before + 1,
        "recorded once"
    );
    assert!(
        notice.get().is_some(),
        "expansion holds the recovery notice"
    );
    assert!(
        ordered.load(Ordering::SeqCst),
        "expansion ran before Activity"
    );
}

#[test]
fn composition_never_relaunches_before_the_windows_exist() {
    // The relaunch helper's quit hook is bound when the windows are built. Nothing in
    // `Kudzu::new`, its reactions or its shutdown may relaunch before that.
    let scratch = Scratch::new();
    let fixtures = Fixtures::new().unwrap();
    let app = Kudzu::new(config(&scratch, &fixtures, true, Duration::ZERO)).unwrap();
    std::thread::sleep(Duration::from_millis(100));
    let _ = app.shutdown();
    assert_eq!(*fixtures.relauncher.calls.lock().unwrap(), 0);
}

#[test]
fn final_collection_is_bounded_in_time() {
    let started = Instant::now();
    let slow = bounded(Duration::from_millis(50), || {
        std::thread::sleep(Duration::from_secs(2));
        1
    });
    assert_eq!(slow, None);
    assert!(started.elapsed() < Duration::from_secs(1));
    assert_eq!(bounded(Duration::from_secs(1), || 2), Some(2));
    let _ = utc_time;
}

/// S41 review O3: a fixture host starts past onboarding, as the Tauri mock does
/// (`mock.ts:79`), while a native first run starts at it. The seed applies only to a new
/// settings file, so a stored choice is kept on the next launch.
#[test]
fn a_fixture_host_starts_past_onboarding_once() {
    let scratch = Scratch::new();
    let fixtures = Fixtures::new().unwrap();
    let fixture_config = || {
        let mut config = config(&scratch, &fixtures, false, Duration::ZERO);
        config.host = Host::fixtures(&fixtures, scratch.0.clone());
        config
    };
    let app = Kudzu::new(fixture_config()).unwrap();
    assert!(app.settings.watch().get().onboarding_completed);
    app.settings
        .update(|s| s.onboarding_completed = false)
        .unwrap();
    assert_eq!(app.shutdown().durable, Ok(()));
    drop(app);
    let again = Kudzu::new(fixture_config()).unwrap();
    assert!(!again.settings.watch().get().onboarding_completed);
    let _ = again.shutdown();

    let native = Scratch::new();
    let app = Kudzu::new(config(&native, &fixtures, false, Duration::ZERO)).unwrap();
    assert!(!app.settings.watch().get().onboarding_completed);
    let _ = app.shutdown();
}
