use super::*;
use crate::error::PortError;
use crate::fixtures::{
    clock::FakeClock, fetch::FakeFetch, relaunch::FakeRelauncher, update_install::FakeInstaller,
};
use crate::ports::InstallProbe;
use crate::schedule::Scheduler;
use crate::tasks::{Task, TaskError};
use crate::watch::{Publisher, Watch};
use minisign_verify::PublicKey;
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;

const TEST_PUB: &str = include_str!("fixtures/test.pub");
const ARCHIVE: &[u8] = include_bytes!("fixtures/archive.bin");
const ARCHIVE_SIG: &str = include_str!("fixtures/archive.bin.minisig");
const ARCHIVE_URL: &str =
    "https://github.com/Stringsaeed/kudzu/releases/download/v9.9.9/Kudzu_9.9.9_universal.app.tar.gz";

struct Harness {
    updates: Updates,
    fetch: Arc<FakeFetch>,
    installer: Arc<FakeInstaller>,
    relauncher: Arc<FakeRelauncher>,
    clock: Arc<FakeClock>,
    _prefs: Publisher<Prefs>,
    _scheduler: Arc<Scheduler>,
}

fn test_key() -> PublicKey {
    PublicKey::decode(TEST_PUB).expect("test public key")
}

fn test_config(release_build: bool, updates_allowed: bool) -> Config {
    Config {
        current_version: "0.7.0".into(),
        bundle_id: "sh.saeed.kudzu".into(),
        updates_allowed,
        release_build,
        public_key: test_key(),
    }
}

fn prefs(auto_check: bool, channel: Channel) -> Prefs {
    Prefs {
        auto_check,
        channel,
    }
}

fn harness(config: Config, prefs: Prefs) -> Harness {
    let fetch = Arc::new(FakeFetch::default());
    let installer = Arc::new(FakeInstaller::default());
    let relauncher = Arc::new(FakeRelauncher::default());
    let clock = Arc::new(FakeClock::default());
    let scheduler = Arc::new(Scheduler::new(clock.clone()).unwrap());
    let prefs_pub = Publisher::new(prefs);
    let updates = Updates::new(
        config,
        prefs_pub.watch(),
        UpdatePorts {
            fetch: fetch.clone(),
            installer: installer.clone(),
            relauncher: relauncher.clone(),
            clock: clock.clone(),
            scheduler: scheduler.clone(),
        },
    )
    .unwrap();
    Harness {
        updates,
        fetch,
        installer,
        relauncher,
        clock,
        _prefs: prefs_pub,
        _scheduler: scheduler,
    }
}

fn feed_json(version: &str, url: &str, signature: &str) -> Vec<u8> {
    serde_json::json!({
        "version": version,
        "notes": "Features",
        "pub_date": "2026-11-11T12:00:00.000Z",
        "platforms": {
            "darwin-aarch64": { "signature": signature, "url": url },
            "darwin-x86_64": { "signature": signature, "url": url },
        }
    })
    .to_string()
    .into_bytes()
}

fn offer_signed(h: &Harness, feed: &str) {
    h.fetch
        .respond(feed, Ok(feed_json("9.9.9", ARCHIVE_URL, ARCHIVE_SIG)));
    h.fetch.respond(ARCHIVE_URL, Ok(ARCHIVE.to_vec()));
}

fn wait_task<T: Send + 'static>(task: &Task<T>) -> T {
    let (tx, rx) = mpsc::channel();
    let _sub = task.ready().subscribe(move || {
        let _ = tx.send(());
    });
    if let Some(done) = task.take() {
        return unwrap_task(done);
    }
    rx.recv_timeout(Duration::from_secs(2))
        .expect("update task ready");
    unwrap_task(task.take().expect("update task result"))
}

fn unwrap_task<T>(result: Result<T, TaskError>) -> T {
    result.expect("update task")
}

fn wait_relaunch(relauncher: &FakeRelauncher) {
    let deadline = std::time::Instant::now() + Duration::from_secs(2);
    while std::time::Instant::now() < deadline {
        if *relauncher.calls.lock().unwrap() == 1 {
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("relaunch was not called");
}

#[test]
fn development_variant_cannot_use_production_updates() {
    assert!(update_allowed("sh.saeed.kudzu").is_ok());
    assert!(update_allowed("sh.saeed.kudzu.dev").is_err());
}

#[test]
fn minisign_cli_fixture_verifies_with_minisign_verify() {
    verify::verify(&test_key(), ARCHIVE, ARCHIVE_SIG).unwrap();
}

#[test]
fn production_key_decodes() {
    PublicKey::decode(PRODUCTION_PUBKEY).unwrap();
}

#[test]
fn unknown_total_displays_as_thirty_percent() {
    assert_eq!(display_percent(0, None), 30);
    assert_eq!(display_percent(1_000_000, None), 30);
    assert_eq!(display_percent(50, Some(100)), 50);
    assert_eq!(display_percent(0, Some(100)), 0);
}

#[test]
fn user_check_walks_checking_downloading_ready() {
    let h = harness(test_config(false, true), prefs(true, Channel::Stable));
    offer_signed(&h, STABLE_FEED);
    let seen = Arc::new(Mutex::new(Vec::new()));
    let watch = h.updates.state();
    let _sub = watch.subscribe({
        let seen = seen.clone();
        let watch = watch.clone();
        move || {
            seen.lock().unwrap().push((*watch.get()).clone());
        }
    });
    let ready = wait_task(&h.updates.check(Trigger::User));
    assert!(matches!(
        ready,
        UpdateState::Ready { ref version, .. } if version == "9.9.9"
    ));
    let phases: Vec<_> = seen
        .lock()
        .unwrap()
        .iter()
        .map(|state| match state {
            UpdateState::Idle => "idle",
            UpdateState::Checking => "checking",
            UpdateState::UpToDate { .. } => "upToDate",
            UpdateState::Downloading { .. } => "downloading",
            UpdateState::Ready { .. } => "ready",
            UpdateState::Restarting { .. } => "restarting",
            UpdateState::Error { .. } => "error",
        })
        .collect();
    assert!(
        phases.contains(&"checking")
            && phases.contains(&"downloading")
            && phases.contains(&"ready"),
        "{phases:?}"
    );
}

#[test]
fn one_check_at_a_time() {
    let h = harness(test_config(false, true), prefs(true, Channel::Stable));
    offer_signed(&h, STABLE_FEED);
    let (started, release) = h.fetch.block_next();
    let first = h.updates.check(Trigger::User);
    started
        .recv_timeout(Duration::from_secs(2))
        .expect("first fetch started");
    assert!(matches!(*h.updates.state().get(), UpdateState::Checking));
    let second = wait_task(&h.updates.check(Trigger::User));
    assert!(matches!(second, UpdateState::Checking));
    assert_eq!(h.fetch.requests.lock().unwrap().len(), 1);
    release.send(()).unwrap();
    let ready = wait_task(&first);
    assert!(matches!(ready, UpdateState::Ready { .. }));
}

#[test]
fn schedule_runs_only_when_enabled_release_and_production() {
    let off = harness(test_config(true, true), prefs(false, Channel::Stable));
    offer_signed(&off, STABLE_FEED);
    off.clock
        .advance(FIRST_CHECK_DELAY + Duration::from_secs(1));
    std::thread::sleep(Duration::from_millis(150));
    assert!(off.fetch.requests.lock().unwrap().is_empty());

    let debug = harness(test_config(false, true), prefs(true, Channel::Stable));
    offer_signed(&debug, STABLE_FEED);
    debug
        .clock
        .advance(FIRST_CHECK_DELAY + Duration::from_secs(1));
    std::thread::sleep(Duration::from_millis(150));
    assert!(debug.fetch.requests.lock().unwrap().is_empty());

    let dev = harness(test_config(true, false), prefs(true, Channel::Stable));
    offer_signed(&dev, STABLE_FEED);
    dev.clock
        .advance(FIRST_CHECK_DELAY + Duration::from_secs(1));
    std::thread::sleep(Duration::from_millis(150));
    assert!(dev.fetch.requests.lock().unwrap().is_empty());

    let on = harness(test_config(true, true), prefs(true, Channel::Stable));
    on.fetch.respond(
        STABLE_FEED,
        Ok(feed_json("0.7.0", ARCHIVE_URL, ARCHIVE_SIG)),
    );
    on.clock.advance(FIRST_CHECK_DELAY);
    wait_state(&on.updates.state(), |state| {
        matches!(state, UpdateState::UpToDate { .. })
    });
    assert_eq!(on.fetch.requests.lock().unwrap()[0].url, STABLE_FEED);
    on.clock.advance(CHECK_INTERVAL);
    wait_until(&on.fetch, |fetch| fetch.requests.lock().unwrap().len() >= 2);
}

#[test]
fn stable_and_beta_feeds() {
    let stable = harness(test_config(false, true), prefs(true, Channel::Stable));
    offer_signed(&stable, STABLE_FEED);
    wait_task(&stable.updates.check(Trigger::User));
    assert_eq!(stable.fetch.requests.lock().unwrap()[0].url, STABLE_FEED);

    let beta = harness(test_config(false, true), prefs(true, Channel::Beta));
    offer_signed(&beta, BETA_FEED);
    wait_task(&beta.updates.check(Trigger::User));
    assert_eq!(beta.fetch.requests.lock().unwrap()[0].url, BETA_FEED);
}

#[test]
fn never_a_downgrade() {
    let h = harness(test_config(false, true), prefs(true, Channel::Stable));
    h.fetch.respond(
        STABLE_FEED,
        Ok(feed_json("0.1.0", ARCHIVE_URL, ARCHIVE_SIG)),
    );
    let state = wait_task(&h.updates.check(Trigger::User));
    assert!(matches!(state, UpdateState::UpToDate { .. }));
    assert!(h
        .fetch
        .requests
        .lock()
        .unwrap()
        .iter()
        .all(|r| r.url != ARCHIVE_URL));
}

#[test]
fn bad_signature_drops_bytes() {
    let h = harness(test_config(false, true), prefs(true, Channel::Stable));
    h.fetch.respond(
        STABLE_FEED,
        Ok(feed_json("9.9.9", ARCHIVE_URL, ARCHIVE_SIG)),
    );
    h.fetch
        .respond(ARCHIVE_URL, Ok(b"not-the-signed-bytes".to_vec()));
    let state = wait_task(&h.updates.check(Trigger::User));
    assert!(matches!(
        state,
        UpdateState::Error { ref message, needs_move: false }
            if *message == UpdateError::Signature.to_string()
    ));
    h.updates.install_on_quit();
    assert!(h.installer.installed.lock().unwrap().is_empty());
}

#[test]
fn quiet_check_error_keeps_previous_state() {
    let h = harness(test_config(false, true), prefs(true, Channel::Stable));
    h.fetch.respond(
        STABLE_FEED,
        Ok(feed_json("0.7.0", ARCHIVE_URL, ARCHIVE_SIG)),
    );
    let first = wait_task(&h.updates.check(Trigger::User));
    assert!(matches!(first, UpdateState::UpToDate { .. }));
    h.fetch.respond(STABLE_FEED, Err(PortError::Unavailable));
    let second = wait_task(&h.updates.check(Trigger::Scheduled));
    assert!(matches!(second, UpdateState::UpToDate { .. }));
}

#[test]
fn quiet_download_error_returns_to_idle() {
    let h = harness(test_config(false, true), prefs(true, Channel::Stable));
    h.fetch.respond(
        STABLE_FEED,
        Ok(feed_json("9.9.9", ARCHIVE_URL, ARCHIVE_SIG)),
    );
    h.fetch.respond(ARCHIVE_URL, Err(PortError::TimedOut));
    let state = wait_task(&h.updates.check(Trigger::Scheduled));
    assert_eq!(state, UpdateState::Idle);
}

#[test]
fn user_error_is_reported() {
    let h = harness(test_config(false, true), prefs(true, Channel::Stable));
    h.fetch.respond(STABLE_FEED, Err(PortError::Unavailable));
    let state = wait_task(&h.updates.check(Trigger::User));
    assert!(matches!(
        state,
        UpdateState::Error {
            needs_move: false,
            ..
        }
    ));
}

#[test]
fn needs_move_comes_from_installer_probe() {
    let h = harness(test_config(false, true), prefs(true, Channel::Stable));
    *h.installer.probe.lock().unwrap() = InstallProbe { needs_move: true };
    let state = wait_task(&h.updates.check(Trigger::User));
    assert!(matches!(
        state,
        UpdateState::Error {
            needs_move: true,
            ref message
        } if *message == UpdateError::NeedsMove.to_string()
    ));
    assert!(h.fetch.requests.lock().unwrap().is_empty());
}

#[test]
fn install_on_quit_swaps_without_relaunch() {
    let h = harness(test_config(false, true), prefs(true, Channel::Stable));
    offer_signed(&h, STABLE_FEED);
    wait_task(&h.updates.check(Trigger::User));
    h.updates.install_on_quit();
    let installed = h.installer.installed.lock().unwrap();
    assert_eq!(installed.len(), 1);
    assert_eq!(installed[0].version, "9.9.9");
    assert_eq!(installed[0].bundle_id, "sh.saeed.kudzu");
    assert_eq!(*h.relauncher.calls.lock().unwrap(), 0);
    assert!(matches!(
        *h.updates.state().get(),
        UpdateState::Restarting { ref version } if version == "9.9.9"
    ));
}

#[test]
fn restart_waits_400ms_after_install() {
    let h = harness(test_config(false, true), prefs(true, Channel::Stable));
    offer_signed(&h, STABLE_FEED);
    wait_task(&h.updates.check(Trigger::User));
    wait_task(&h.updates.restart_to_update());
    assert_eq!(h.installer.installed.lock().unwrap().len(), 1);
    assert_eq!(*h.relauncher.calls.lock().unwrap(), 0);
    h.clock.advance(RESTART_DELAY);
    wait_relaunch(&h.relauncher);
}

#[test]
fn downloads_have_time_limit_and_size_cap() {
    let h = harness(test_config(false, true), prefs(true, Channel::Stable));
    offer_signed(&h, STABLE_FEED);
    wait_task(&h.updates.check(Trigger::User));
    let requests = h.fetch.requests.lock().unwrap();
    let feed = requests.iter().find(|r| r.url == STABLE_FEED).unwrap();
    assert_eq!(feed.timeout, FEED_TIMEOUT);
    assert_eq!(feed.max_bytes, FEED_MAX_BYTES);
    let archive = requests.iter().find(|r| r.url == ARCHIVE_URL).unwrap();
    assert_eq!(archive.timeout, ARCHIVE_TIMEOUT);
    assert_eq!(archive.max_bytes, ARCHIVE_MAX_BYTES);
}

#[test]
fn unknown_total_progress_uses_thirty_percent() {
    let h = harness(test_config(false, true), prefs(true, Channel::Stable));
    offer_signed(&h, STABLE_FEED);
    *h.fetch.unknown_total.lock().unwrap() = true;
    let seen = Arc::new(Mutex::new(Vec::new()));
    let watch = h.updates.state();
    let _sub = watch.subscribe({
        let seen = seen.clone();
        let watch = watch.clone();
        move || {
            if let UpdateState::Downloading {
                downloaded, total, ..
            } = &*watch.get()
            {
                seen.lock().unwrap().push((*downloaded, *total));
            }
        }
    });
    wait_task(&h.updates.check(Trigger::User));
    let events = seen.lock().unwrap().clone();
    assert!(
        events.iter().any(|(_, total)| total.is_none()),
        "{events:?}"
    );
    assert!(events.iter().any(|(downloaded, total)| {
        display_percent(*downloaded, *total) == UNKNOWN_TOTAL_PERCENT
    }));
}

#[test]
fn cancel_mid_download_drops_the_package() {
    let h = harness(test_config(false, true), prefs(true, Channel::Stable));
    offer_signed(&h, STABLE_FEED);
    let (progressed, resume) = h.fetch.pause_after_progress_url(Some(ARCHIVE_URL));
    let task = h.updates.check(Trigger::User);
    progressed
        .recv_timeout(Duration::from_secs(2))
        .expect("download progress");
    h.updates.cancel();
    resume.send(()).unwrap();
    let state = wait_task(&task);
    assert!(!matches!(state, UpdateState::Ready { .. }), "{state:?}");
    h.updates.install_on_quit();
    assert!(h.installer.installed.lock().unwrap().is_empty());
}

#[test]
fn dev_identity_reports_the_unavailable_error() {
    let h = harness(test_config(false, false), prefs(true, Channel::Stable));
    offer_signed(&h, STABLE_FEED);
    let state = wait_task(&h.updates.check(Trigger::User));
    assert!(matches!(
        state,
        UpdateState::Error { ref message, needs_move: false }
            if message == "Updates are unavailable in Kudzu Dev."
    ));
    assert!(h.fetch.requests.lock().unwrap().is_empty());
}

fn wait_state(
    watch: &Watch<UpdateState>,
    pred: impl Fn(&UpdateState) -> bool + Send + Sync + 'static,
) -> Arc<UpdateState> {
    let pred = Arc::new(pred);
    let (tx, rx) = mpsc::channel();
    let _sub = watch.subscribe({
        let watch = watch.clone();
        let tx = tx.clone();
        let pred = pred.clone();
        move || {
            let snap = watch.get();
            if pred(&snap) {
                let _ = tx.send(snap);
            }
        }
    });
    let snap = watch.get();
    if pred(&snap) {
        return snap;
    }
    rx.recv_timeout(Duration::from_secs(2))
        .expect("update state")
}

fn wait_until(fetch: &FakeFetch, pred: impl Fn(&FakeFetch) -> bool) {
    let deadline = std::time::Instant::now() + Duration::from_secs(2);
    while std::time::Instant::now() < deadline {
        if pred(fetch) {
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("fetch did not reach the expected request count");
}
