//! Headless `Runtime<MainState>` tests for the main window frame (`PLAN.md` S41, `LIB-14`,
//! `LIB-15`, `LIB-7`). This file holds the rig; each file next to it covers one area, so a
//! screen step adds its own file rather than editing a shared one.

mod clipboard;
mod clipboard_landmarks;
mod clipboard_preview;
mod clipboard_preview_landmarks;
mod clipboard_review;
mod clipboard_review59;
mod clipboard_scroll;
mod clipboard_secrets;
mod command_menu;
mod command_menu_review;
mod frame;
mod group_settings;
mod insights;
mod insights_motion;
mod landmarks;
mod library;
mod library_bulk;
mod library_landmarks;
mod library_list;
mod library_reveal;
mod library_review;
mod library_scroll;
mod onboarding;
mod onboarding_review;
mod routes;
mod shortcuts;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use kudzu_app::assembly::{Config, Host, Kudzu};
use kudzu_app::fixtures::{hotkey_registrar::FakeHotkeys, Fixtures};
use kudzu_app::paths::Paths;
use kudzu_app::ports::HotkeyRegistrar;
use kudzu_app::settings::{Theme, Variant};
use rune::{Env, Key, KeyEvent, Mods, Runtime};
use uuid::Uuid;

use crate::shell::windows::{parts, Launch};
use crate::shell::{Identity, MainState};

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("kudzu-main-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn build_kudzu(dir: &Path, fixtures: &Fixtures) -> Arc<Kudzu> {
    Kudzu::new(Config {
        paths: Paths {
            data: dir.to_path_buf(),
            logs: dir.join("logs"),
        },
        host: Host::fixtures(fixtures, dir.to_path_buf()),
        variant: Variant::Dev,
        version: "0.7.0".into(),
        app_name: "Kudzu Next".into(),
        bundle_id: "sh.saeed.kudzu.next".into(),
        os_major: 15,
        mcp_name: "kudzu-next".into(),
        executable: dir.join("Kudzu"),
        agent_args: vec!["--mcp".into()],
        bundled_skill_dir: None,
        production: false,
        cloud_and_updates: false,
        debounce: Duration::from_millis(50),
        retry: Duration::from_millis(50),
    })
    .unwrap()
}

pub(super) struct Rig {
    _scratch: Scratch,
    pub(super) fixtures: Fixtures,
    pub(super) kudzu: Arc<Kudzu>,
    pub(super) rt: Runtime<MainState>,
}

impl Rig {
    pub(super) fn new(onboarded: bool) -> Self {
        Self::themed(onboarded, Theme::Dark)
    }

    /// A rig whose stored Appearance is `theme` before the first poll.
    pub(super) fn themed(onboarded: bool, theme: Theme) -> Self {
        let scratch = Scratch::new();
        let fixtures = Fixtures::new().unwrap();
        let kudzu = build_kudzu(&scratch.0, &fixtures);
        kudzu
            .settings
            .update(|s| {
                s.onboarding_completed = onboarded;
                s.theme = theme;
            })
            .unwrap();
        let hotkeys: Arc<dyn HotkeyRegistrar> = Arc::new(FakeHotkeys::default());
        let p = parts(
            &kudzu,
            Launch {
                identity: Identity {
                    app_name: "Kudzu Next",
                    dev: true,
                },
                native: false,
                hidden: false,
            },
            Some(hotkeys),
        );
        let mut rt = Runtime::new(
            p.main,
            crate::main::view,
            (1180.0, 760.0),
            Env::new(true, 1.0),
        );
        rt.poll = Some(crate::shell::lifecycle::poll);
        rt.on_key = Some(crate::shell::lifecycle::on_key);
        rt.settle_all();
        rt.frame(0.0);
        rt.frame(0.0);
        Self {
            _scratch: scratch,
            fixtures,
            kudzu,
            rt,
        }
    }

    /// The key of the element holding Rune's keyboard focus.
    pub(super) fn focused_key(&self) -> Option<String> {
        self.rt
            .elements()
            .into_iter()
            .find(|e| e.focused)
            .and_then(|e| e.key)
    }

    /// Asks for `route` the way `Shell::open_main` does, then lets the poll apply it.
    pub(super) fn open_route(&mut self, route: crate::shell::Route) {
        self.rt.update(move |st, _| st.open_route(route));
        self.settle();
    }

    pub(super) fn settle(&mut self) {
        self.rt.settle_all();
        self.rt.frame(0.016);
        self.rt.frame(0.0);
    }

    /// The library as the service holds it now.
    pub(super) fn library(&self) -> Arc<kudzu_core::KudzuLibrary> {
        self.kudzu.library.watch().get()
    }

    /// The fixture data directory (`library.json` is written here).
    pub(super) fn data_dir(&self) -> &Path {
        &self._scratch.0
    }

    pub(super) fn rect(&self, key: &str) -> Option<[f32; 4]> {
        self.rt
            .elements()
            .into_iter()
            .find(|e| e.key.as_deref() == Some(key))
            .map(|e| e.rect)
    }

    pub(super) fn click_key(&mut self, key: &str) {
        let r = self
            .rect(key)
            .unwrap_or_else(|| panic!("missing layout rect for key `{key}`"));
        let (cx, cy) = (r[0] + r[2] * 0.5, r[1] + r[3] * 0.5);
        self.rt.pointer_move(cx, cy);
        self.rt.pointer_down(cx, cy);
        self.rt.pointer_up(cx, cy);
        self.rt.settle_all();
        self.rt.frame(0.016);
        self.rt.frame(0.0);
    }

    pub(super) fn click_text(&mut self, needle: &str) {
        let r = self
            .rt
            .elements()
            .into_iter()
            .find(|e| e.text.as_deref() == Some(needle))
            .map(|e| e.rect)
            .unwrap_or_else(|| panic!("missing element with text `{needle}`"));
        let (cx, cy) = (r[0] + r[2] * 0.5, r[1] + r[3] * 0.5);
        self.rt.pointer_move(cx, cy);
        self.rt.pointer_down(cx, cy);
        self.rt.pointer_up(cx, cy);
        self.rt.settle_all();
        self.rt.frame(0.016);
        self.rt.frame(0.0);
    }

    pub(super) fn press(&mut self, ev: KeyEvent) {
        self.rt.key(ev);
        self.rt.settle_all();
        self.rt.frame(0.016);
        self.rt.frame(0.0);
    }
}

impl Drop for Rig {
    fn drop(&mut self) {
        let _ = self.kudzu.shutdown();
    }
}

pub(super) fn cmd(ch: char) -> KeyEvent {
    KeyEvent {
        key: Key::Command(ch),
        mods: Mods {
            cmd: true,
            ..Mods::default()
        },
    }
}

pub(super) fn cmd_shift(ch: char) -> KeyEvent {
    KeyEvent {
        key: Key::Command(ch),
        mods: Mods {
            cmd: true,
            shift: true,
            ..Mods::default()
        },
    }
}

pub(super) fn plain(ch: char, shift: bool) -> KeyEvent {
    KeyEvent {
        key: Key::Text(ch.to_string()),
        mods: Mods {
            shift,
            ..Mods::default()
        },
    }
}

pub(super) fn esc() -> KeyEvent {
    KeyEvent {
        key: Key::Escape,
        mods: Mods::default(),
    }
}

pub(super) fn cmd_backspace() -> KeyEvent {
    KeyEvent {
        key: Key::Backspace,
        mods: Mods {
            cmd: true,
            ..Mods::default()
        },
    }
}

/// M1 (S41 review): the window file keeps services, state and spec only; screen logic lives in
/// the screen modules, so parallel screen steps don't all edit one file.
#[test]
fn the_main_window_file_holds_only_services_state_and_spec() {
    let source = include_str!("../../shell/windows/main.rs");
    let lines = source.lines().count();
    assert!(lines < 400, "shell/windows/main.rs has {lines} lines");
    for moved in [
        "fn dispatch_action",
        "fn delete_snippets_with_undo",
        "fn reconcile_route",
    ] {
        assert!(
            !source.contains(moved),
            "{moved} belongs in a screen module"
        );
    }
}
