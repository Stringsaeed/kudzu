//! Palette session regressions. These drive `PaletteState` the way the window hooks do,
//! on fixtures, without a Rune loop.
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use kudzu_app::assembly::{Config, Host, Kudzu};
use kudzu_app::fixtures::hotkey_registrar::FakeHotkeys;
use kudzu_app::fixtures::Fixtures;
use kudzu_app::paths::Paths;
use kudzu_app::ports::HotkeyRegistrar;
use kudzu_app::settings::Variant;
use rune::{Key, KeyEvent, Mods, Tx};

use crate::palette::{apply, on_key, poll, Effect};
use crate::shell::windows::{parts, Launch, Parts};
use crate::shell::Identity;

struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("kudzu-palette-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

struct Rig {
    _scratch: Scratch,
    fixtures: Fixtures,
    kudzu: Arc<Kudzu>,
    p: Parts,
}

impl Drop for Rig {
    fn drop(&mut self) {
        let _ = self.kudzu.shutdown();
    }
}

fn rig() -> Rig {
    let scratch = Scratch::new();
    let fixtures = Fixtures::new().unwrap();
    let kudzu = Kudzu::new(Config {
        paths: Paths {
            data: scratch.0.clone(),
            logs: scratch.0.join("logs"),
        },
        host: Host::fixtures(&fixtures, scratch.0.clone()),
        variant: Variant::Dev,
        version: "0.7.0".into(),
        app_name: "Kudzu Next".into(),
        bundle_id: "sh.saeed.kudzu.next".into(),
        os_major: 15,
        mcp_name: "kudzu-next".into(),
        executable: scratch.0.join("Kudzu"),
        agent_args: vec!["--mcp".into()],
        bundled_skill_dir: None,
        production: false,
        cloud_and_updates: false,
        debounce: Duration::from_millis(50),
        retry: Duration::from_millis(50),
    })
    .unwrap();
    let hotkeys = Arc::new(FakeHotkeys::default());
    let registrar: Arc<dyn HotkeyRegistrar> = hotkeys;
    let launch = Launch {
        identity: Identity {
            app_name: "Kudzu Next",
            dev: true,
        },
        native: false,
        hidden: false,
    };
    let p = parts(&kudzu, launch, Some(registrar));
    Rig {
        _scratch: scratch,
        fixtures,
        kudzu,
        p,
    }
}

fn pump(rig: &mut Rig, mut done: impl FnMut(&crate::shell::windows::PaletteState) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut tx = Tx::default();
    while !done(&rig.p.palette) {
        assert!(Instant::now() < deadline, "the palette did not settle");
        poll(&mut rig.p.palette, 0.0, &mut tx);
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn key(key: Key, mods: Mods) -> KeyEvent {
    KeyEvent { key, mods }
}

fn inserted(rig: &Rig) -> Vec<uuid::Uuid> {
    rig.fixtures
        .engine
        .state
        .lock()
        .unwrap()
        .insertions
        .iter()
        .map(|row| row.snippet)
        .collect()
}

#[test]
fn reopening_the_palette_lists_rows_again() {
    let mut rig = rig();
    rig.p.palette.toggle();
    pump(&mut rig, |palette| {
        !palette.model.search.presentation.rows.is_empty()
    });
    rig.p.palette.dismiss(false);
    assert!(!rig.p.palette.model.open);
    rig.p.palette.toggle();
    pump(&mut rig, |palette| {
        !palette.model.search.presentation.rows.is_empty()
    });
}

#[test]
fn enter_before_the_capture_lands_inserts_when_the_session_arrives() {
    let mut rig = rig();
    rig.p.palette.toggle();
    let id = rig.kudzu.library.watch().get().snippets[0].id;
    apply(&mut rig.p.palette, Effect::Insert(id));
    assert!(
        rig.p.palette.model.capture.is_some(),
        "the capture has to stay until the session lands"
    );
    assert!(rig.p.palette.model.session.is_none());
    assert!(rig.p.palette.model.open);
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut tx = Tx::default();
    while !inserted(&rig).contains(&id) {
        assert!(Instant::now() < deadline, "the queued pick never inserted");
        poll(&mut rig.p.palette, 0.0, &mut tx);
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn a_second_toggle_during_an_insert_stays_hidden_until_the_hold_deadline() {
    let mut rig = rig();
    rig.p.palette.toggle();
    pump(&mut rig, |palette| palette.model.session.is_some());
    let id = rig.kudzu.library.watch().get().snippets[0].id;
    apply(&mut rig.p.palette, Effect::Insert(id));
    assert!(!rig.p.palette.model.open);
    assert_eq!(rig.p.palette.model.search.pending, Some(id));
    rig.p.palette.toggle();
    assert!(
        !rig.p.palette.model.open,
        "a second toggle while the insert is in flight reopens the palette"
    );
    assert_eq!(rig.p.palette.model.search.pending, Some(id));
    rig.fixtures.clock.advance(Duration::from_secs(11));
    rig.p.palette.toggle();
    assert!(rig.p.palette.model.open);
}

#[test]
fn enter_and_digits_wait_for_the_list_that_answers_the_query() {
    let mut rig = rig();
    rig.p.palette.toggle();
    pump(&mut rig, |palette| {
        !palette.model.search.presentation.rows.is_empty()
    });
    let stale = rig.p.palette.model.search.presentation.rows[0].id;
    rig.p
        .palette
        .model
        .search
        .edit
        .replace_text("zzzz-no-match");
    let mut tx = Tx::default();
    on_key(
        &mut rig.p.palette,
        &key(
            Key::Command('1'),
            Mods {
                cmd: true,
                ..Mods::default()
            },
        ),
        &mut tx,
    );
    assert!(rig.p.palette.model.open);
    assert!(
        !inserted(&rig).contains(&stale),
        "⌘1 ran against the previous list"
    );
    on_key(
        &mut rig.p.palette,
        &key(Key::Enter, Mods::default()),
        &mut tx,
    );
    assert!(rig.p.palette.model.open);
    assert!(!inserted(&rig).contains(&stale));
    pump(&mut rig, |palette| {
        palette.model.search.presentation.for_query == "zzzz-no-match" && !palette.model.open
    });
    assert!(!inserted(&rig).contains(&stale));
}
