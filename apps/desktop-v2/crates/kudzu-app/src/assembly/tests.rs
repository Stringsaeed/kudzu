//! Reactions and shutdown, on fixtures, outside the Cargo workspace.

use super::*;
use crate::expansion::InsertionOutcome;
use crate::fixtures::Fixtures;
use crate::library::SnippetDraft;
use crate::persist::{Revision, WriteTarget};
use crate::ports::{CloudBatch, Expansion};
use crate::settings::Theme;
use crate::sync::SyncStatus;
use crate::updates::UpdateState;
use chrono::{DateTime, FixedOffset, Utc};
use kudzu_core::activity::{ActivityEvent, ActivityOutcome};
use kudzu_core::sync::{RecordKind, RemoteRecord, META_RECORD};
use std::path::PathBuf;
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, SystemTime};

pub(super) struct Scratch(pub(super) PathBuf);
impl Scratch {
    pub(super) fn new() -> Self {
        // /tmp keeps the agent socket inside sun_path, and stays outside the Cargo workspace
        // even when TMPDIR points at the worktree.
        let path = PathBuf::from("/tmp").join(format!("kudzu-s40-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

pub(super) fn utc_time(time: SystemTime) -> DateTime<FixedOffset> {
    let utc: DateTime<Utc> = time.into();
    utc.with_timezone(&FixedOffset::east_opt(0).unwrap())
}

fn open(production: bool, debounce: Duration) -> (Scratch, Fixtures, Arc<Kudzu>) {
    open_prepared(production, debounce, |_| {})
}

pub(super) fn config(
    scratch: &Scratch,
    fixtures: &Fixtures,
    production: bool,
    debounce: Duration,
) -> Config {
    Config {
        paths: Paths {
            data: scratch.0.clone(),
            logs: scratch.0.join("logs"),
        },
        host: Host {
            ports: fixtures.ports(),
            locale: "en_US".into(),
            home: scratch.0.clone(),
            time_zone: BackupTimeZone::Fixed(FixedOffset::east_opt(0).unwrap()),
            local_time: utc_time,
        },
        variant: Variant::Dev,
        version: "0.7.0".into(),
        app_name: "Kudzu Next".into(),
        bundle_id: "sh.saeed.kudzu.next".into(),
        os_major: 15,
        mcp_name: "kudzu-next".into(),
        executable: scratch.0.join("Kudzu"),
        agent_args: vec!["--mcp".into()],
        bundled_skill_dir: None,
        production,
        cloud_and_updates: false,
        debounce,
        retry: Duration::from_millis(20),
    }
}

fn open_prepared(
    production: bool,
    debounce: Duration,
    prepare: impl FnOnce(&Fixtures),
) -> (Scratch, Fixtures, Arc<Kudzu>) {
    let scratch = Scratch::new();
    let fixtures = Fixtures::new().unwrap();
    prepare(&fixtures);
    let app = Kudzu::new(config(&scratch, &fixtures, production, debounce)).unwrap();
    (scratch, fixtures, app)
}

pub(super) fn wait_for(mut ready: impl FnMut() -> bool) {
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while !ready() {
        assert!(
            std::time::Instant::now() < deadline,
            "condition was not met"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn reactions_follow_the_library_settings_and_engine() {
    let (scratch, fixtures, app) = open(false, Duration::ZERO);
    assert!(app.first_run());
    assert!(matches!(
        *app.sync.status().get(),
        SyncStatus::Unavailable { .. }
    ));

    let reloads = fixtures.engine.state.lock().unwrap().reloads;
    app.library
        .create_snippet(SnippetDraft {
            name: "Note".into(),
            content: "hello".into(),
            ..SnippetDraft::default()
        })
        .unwrap();
    wait_for(|| fixtures.engine.state.lock().unwrap().reloads > reloads);

    let id = app.library.watch().get().snippets[0].id;
    app.library.delete_snippets(&[id]).unwrap();
    let revision = app.library.watch().revision();
    wait_for(|| app.forms.pruned().get().library == revision);

    app.settings
        .update(|settings| settings.clipboard_enabled = false)
        .unwrap();
    assert!(app.clipboard.history_text(0, None).is_none());

    app.settings
        .update(|settings| settings.agent_access = true)
        .unwrap();
    wait_for(|| app.agents.status().get().running);
    assert!(scratch.0.join("agent.sock").exists());

    app.settings
        .update(|settings| settings.launch_at_login = true)
        .unwrap();
    wait_for(|| *fixtures.login_item.enabled.lock().unwrap());
    assert!(app.login_notice.get().is_none());

    let snippet = app.library.watch().get().snippets[0].id;
    fixtures
        .engine
        .emit_activity(ActivityEvent::health(ActivityOutcome::EngineStopped));
    wait_for(|| {
        app.expansion
            .outcomes()
            .get()
            .as_ref()
            .is_some_and(|notice| notice.outcome == InsertionOutcome::EngineStopped)
    });
    let notice = app.expansion.outcomes().get();
    let notice = notice.as_ref().unwrap();
    assert_eq!((notice.source, notice.snippet_id), (None, None));

    fixtures.engine.emit_form(crate::ports::FormRequest {
        token: fixtures.engine.form_token(1),
        snippet_id: snippet,
        snippet_name: "Note".into(),
        fields: vec![],
        target: None,
        caret: None,
        selection: None,
    });
    wait_for(|| app.forms.pending().get().is_some());

    fixtures.engine.emit_expansion(Expansion {
        snippet_id: snippet,
        chars_saved: 4,
    });
    wait_for(|| !app.library.watch().get().usage.is_empty());
}

#[test]
fn a_newer_library_format_asks_for_one_quiet_update_check() {
    let (_scratch, fixtures, app) = open(true, Duration::ZERO);
    wait_for(|| !fixtures.cloud.state.lock().unwrap().starts.is_empty());
    let generation = fixtures.cloud.state.lock().unwrap().starts[0].0;
    fixtures.cloud.deliver(
        generation,
        CloudBatch {
            records: vec![RemoteRecord {
                kind: RecordKind::Meta,
                name: META_RECORD.into(),
                payload: r#"{"syncVersion":2}"#.into(),
            }],
            ..CloudBatch::default()
        },
    );
    wait_for(|| matches!(*app.sync.status().get(), SyncStatus::NeedsUpdate));
    wait_for(|| {
        matches!(
            *app.updates.state().get(),
            UpdateState::Error {
                needs_move: false,
                ..
            }
        )
    });
    assert_eq!(app.update_checks(), 1);
    std::thread::sleep(Duration::from_millis(150));
    assert_eq!(app.update_checks(), 1);
}

#[test]
fn a_failed_login_item_is_reported_once_until_the_setting_changes() {
    let (_scratch, fixtures, app) = open_prepared(false, Duration::ZERO, |fixtures| {
        *fixtures.login_item.failure.lock().unwrap() = Some(crate::error::PortError::Unavailable);
    });
    app.settings
        .update(|settings| settings.launch_at_login = true)
        .unwrap();
    wait_for(|| app.login_notice.get().as_deref() == Some(LOGIN_ITEM_UNAVAILABLE));
    assert!(!*fixtures.login_item.enabled.lock().unwrap());
    *fixtures.login_item.failure.lock().unwrap() = None;
    app.settings
        .update(|settings| settings.theme = Theme::Light)
        .unwrap();
    std::thread::sleep(Duration::from_millis(80));
    assert!(!*fixtures.login_item.enabled.lock().unwrap());
    assert_eq!(
        app.login_notice.get().as_deref(),
        Some(LOGIN_ITEM_UNAVAILABLE)
    );
    app.settings
        .update(|settings| settings.launch_at_login = false)
        .unwrap();
    wait_for(|| app.login_notice.get().is_none());
}

pub(super) struct Hold {
    pub(super) entered: Arc<(Mutex<bool>, Condvar)>,
    pub(super) release: Arc<(Mutex<bool>, Condvar)>,
}

impl WriteTarget<u8> for Hold {
    fn write(&self, _value: &u8) -> Result<(), PersistError> {
        {
            let mut entered = self.entered.0.lock().unwrap();
            *entered = true;
            self.entered.1.notify_all();
        }
        let mut release = self.release.0.lock().unwrap();
        while !*release {
            release = self.release.1.wait(release).unwrap();
        }
        Ok(())
    }
}

#[test]
fn shutdown_stops_producers_before_the_drain_and_runs_once() {
    let (scratch, fixtures, app) = open(true, Duration::from_secs(3600));
    wait_for(|| !fixtures.cloud.state.lock().unwrap().starts.is_empty());
    app.settings
        .update(|settings| {
            settings.agent_access = true;
            settings.theme = Theme::Light;
        })
        .unwrap();
    wait_for(|| app.agents.status().get().running);
    let socket = scratch.0.join("agent.sock");
    assert!(socket.exists());
    let before = std::fs::read_to_string(scratch.0.join("settings.json")).unwrap_or_default();
    assert!(!before.contains("light"));

    let entered = Arc::new((Mutex::new(false), Condvar::new()));
    let release = Arc::new((Mutex::new(false), Condvar::new()));
    let hold = Hold {
        entered: entered.clone(),
        release: release.clone(),
    };
    app.writer()
        .register(hold, Revision(0))
        .unwrap()
        .submit(Revision(1), Arc::new(1))
        .unwrap();

    let shutting = app.clone();
    let worker = std::thread::spawn(move || shutting.shutdown());
    {
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        let mut flag = entered.0.lock().unwrap();
        while !*flag {
            let left = deadline.saturating_duration_since(std::time::Instant::now());
            assert!(!left.is_zero(), "the drain did not reach the writer");
            flag = entered.1.wait_timeout(flag, left).unwrap().0;
        }
    }
    // (2) the agent socket, (3) the engine and clipboard capture, then the sync worker, all
    // stopped before (4) the writer drains.
    assert!(!fixtures.engine.state.lock().unwrap().running);
    assert!(!socket.exists());
    assert!(fixtures.cloud.state.lock().unwrap().stops > 0);
    let entries = app.clipboard.watch().get().entries.len();
    fixtures.pasteboard.external_copy(text("after stop"));
    fixtures.clock.advance(Duration::from_millis(900));
    std::thread::sleep(Duration::from_millis(30));
    assert_eq!(app.clipboard.watch().get().entries.len(), entries);
    {
        let mut flag = release.0.lock().unwrap();
        *flag = true;
        release.1.notify_all();
    }
    let first = worker.join().unwrap();
    assert!(first.ran);
    assert!(first.durable.is_ok());
    let saved = std::fs::read_to_string(scratch.0.join("settings.json")).unwrap();
    assert!(saved.contains("light"));
    let second = app.shutdown();
    assert!(!second.ran);
    assert_eq!(second.durable, first.durable);
}

fn text(value: &str) -> Vec<kudzu_core::clipboard::PasteItem> {
    vec![vec![(
        kudzu_core::clipboard::uti::PLAIN_TEXT.into(),
        value.as_bytes().to_vec(),
    )]]
}

#[test]
fn the_remaining_reaction_rows_run_end_to_end() {
    let (_scratch, fixtures, app) = open(true, Duration::ZERO);

    // library → sync: after the first full fetch, a new snippet is queued for upload.
    wait_for(|| !fixtures.cloud.state.lock().unwrap().starts.is_empty());
    let generation = fixtures.cloud.state.lock().unwrap().starts[0].0;
    fixtures.cloud.deliver(
        generation,
        CloudBatch {
            full_fetch: true,
            ..CloudBatch::default()
        },
    );
    let (record, _) = app
        .library
        .create_snippet(SnippetDraft {
            name: "Synced".into(),
            content: "body".into(),
            ..SnippetDraft::default()
        })
        .unwrap();
    let name = record.id.to_string();
    wait_for(|| {
        let state = fixtures.cloud.state.lock().unwrap();
        state.pending.keys().any(|key| key.contains(&name))
            || state
                .sends
                .iter()
                .any(|(_, batch)| batch.records.iter().any(|r| r.name.contains(&name)))
    });

    // settings.icloud_sync → sync stops.
    app.settings
        .update(|settings| settings.icloud_sync = false)
        .unwrap();
    wait_for(|| fixtures.cloud.state.lock().unwrap().stops > 0);

    // settings (matching data) → expansion rebuilds the engine index.
    let reloads = fixtures.engine.state.lock().unwrap().reloads;
    app.settings
        .update(|settings| settings.denied_bundle_ids.push("com.example.denied".into()))
        .unwrap();
    wait_for(|| fixtures.engine.state.lock().unwrap().reloads > reloads);

    // settings.clipboard_* → capture follows the switch on the 300 ms poll.
    fixtures.pasteboard.external_copy(text("first"));
    wait_for(|| {
        fixtures.clock.advance(Duration::from_millis(300));
        app.clipboard.watch().get().entries.len() == 1
    });
    app.settings
        .update(|settings| settings.clipboard_enabled = false)
        .unwrap();
    fixtures.pasteboard.external_copy(text("second"));
    for _ in 0..5 {
        fixtures.clock.advance(Duration::from_millis(300));
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(app.clipboard.watch().get().entries.len(), 1);

    // engine activity → both the outcome Watch and the Activity ring.
    let before = app.activity.watch().get().len();
    fixtures
        .engine
        .emit_activity(ActivityEvent::health(ActivityOutcome::EngineStopped));
    wait_for(|| app.activity.watch().get().len() > before);
    wait_for(|| app.expansion.outcomes().get().is_some());
}
