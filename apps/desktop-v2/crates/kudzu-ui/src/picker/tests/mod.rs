//! Picker tests: a real `Kudzu` on fixtures, driven through a headless runtime. This file is
//! the rig; each area adds a file beside it.

mod flow;
mod landmarks;
mod rows;
mod scroll;
mod timers;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use kudzu_app::assembly::{Config, Host, Kudzu};
use kudzu_app::fixtures::Fixtures;
use kudzu_app::paths::Paths;
use kudzu_app::settings::{Theme, Variant};
use kudzu_core::clipboard::uti;
use kudzu_core::system::AppInfo;
use rune::{Env, Key, KeyEvent, Mods, Runtime};
use uuid::Uuid;

use crate::picker::drive::FIELD_KEY;
use crate::shell::windows::{parts, Launch, PickerState};
use crate::shell::Identity;

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("kudzu-picker-{}", Uuid::new_v4()));
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
    pub(super) rt: Runtime<PickerState>,
}

pub(super) const NOTES: &str = "com.test.Notes";

impl Rig {
    pub(super) fn new() -> Self {
        Self::themed(Theme::Dark)
    }

    pub(super) fn themed(theme: Theme) -> Self {
        let scratch = Scratch::new();
        let fixtures = Fixtures::new().unwrap();
        // The app the user came from, and permission to paste into it.
        *fixtures.apps.frontmost.lock().unwrap() = Some(AppInfo {
            bundle_id: NOTES.into(),
            name: "Notes".into(),
            path: None,
            pid: Some(4242),
        });
        *fixtures.pasteboard.paste_allowed.lock().unwrap() = true;
        let kudzu = build_kudzu(&scratch.0, &fixtures);
        kudzu
            .settings
            .update(|s| {
                s.onboarding_completed = true;
                s.theme = theme;
            })
            .unwrap();
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
            None,
        );
        let mut rt = Runtime::new(
            p.picker,
            crate::picker::view,
            (800.0, 520.0),
            Env::new(true, 1.0),
        );
        rt.poll = Some(crate::picker::poll);
        rt.on_key = Some(crate::picker::on_key);
        rt.settle_all();
        rt.frame(0.0);
        Self {
            _scratch: scratch,
            fixtures,
            kudzu,
            rt,
        }
    }

    /// Settles frames and waits for a worker to answer.
    pub(super) fn until(&mut self, what: &str, done: impl Fn(&Rig) -> bool) {
        for _ in 0..600 {
            self.settle();
            if done(self) {
                return;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        panic!("timed out waiting for {what}");
    }

    pub(super) fn settle(&mut self) {
        self.rt.settle_all();
        self.rt.frame(0.016);
        self.rt.frame(0.0);
    }

    /// Advances the UI clock by `secs` in frames of at most 16 ms, so deadlines pass the way
    /// they do in the window.
    pub(super) fn advance(&mut self, secs: f64) {
        let mut left = secs;
        while left > 1e-9 {
            let step = left.min(0.016);
            self.rt.frame(step);
            left -= step;
        }
    }

    pub(super) fn state(&self) -> &PickerState {
        &self.rt.state
    }

    /// Opens the picker as the shortcut does and waits for the session to land.
    pub(super) fn open(&mut self) {
        self.rt.update(|s, _| s.toggle());
        self.until("the session", |r| r.state().model.session.is_some());
        self.focus_field();
    }

    pub(super) fn focus_field(&mut self) {
        self.rt.update(|_, tx| tx.focus(FIELD_KEY, true));
        self.settle();
    }

    /// Copies `texts` through the fake pasteboard, oldest first, and opens the picker.
    pub(super) fn with_clips(texts: &[&str]) -> Self {
        let mut rig = Self::new();
        for t in texts {
            rig.copy(t);
        }
        rig.open();
        rig
    }

    pub(super) fn copy(&self, text: &str) {
        self.fixtures.pasteboard.external_copy(vec![vec![(
            uti::PLAIN_TEXT.into(),
            text.as_bytes().to_vec(),
        )]]);
        self.kudzu.clipboard.poll().unwrap();
    }

    pub(super) fn press(&mut self, ev: KeyEvent) {
        self.rt.key(ev);
        self.settle();
    }

    pub(super) fn type_text(&mut self, text: &str) {
        for ch in text.chars() {
            self.press(plain(ch));
        }
    }

    pub(super) fn query(&self) -> String {
        self.state().model.picker.query().to_owned()
    }

    pub(super) fn open_now(&self) -> bool {
        self.state().model.open
    }

    pub(super) fn order_texts(&self) -> Vec<String> {
        let list = &self.state().model.picker.clips.list;
        list.order()
            .iter()
            .filter_map(|id| list.entry(*id))
            .map(|e| e.text.clone().unwrap_or_default())
            .collect()
    }

    pub(super) fn active_text(&self) -> String {
        self.state()
            .model
            .picker
            .active()
            .and_then(|e| e.text.clone())
            .unwrap_or_default()
    }

    pub(super) fn texts(&self) -> Vec<String> {
        self.rt
            .elements()
            .into_iter()
            .filter_map(|e| e.text)
            .collect()
    }

    pub(super) fn has_text(&self, needle: &str) -> bool {
        self.texts().iter().any(|t| t == needle)
    }

    pub(super) fn rect(&self, key: &str) -> Option<[f32; 4]> {
        self.rt
            .elements()
            .into_iter()
            .find(|e| e.key.as_deref() == Some(key))
            .map(|e| e.rect)
    }

    pub(super) fn pasted(&self) -> usize {
        self.fixtures.pasteboard.pasted_into.lock().unwrap().len()
    }

    pub(super) fn flash(&self) -> Option<String> {
        self.state().model.picker.flash_text().map(str::to_owned)
    }
}

impl Drop for Rig {
    fn drop(&mut self) {
        let _ = self.kudzu.shutdown();
    }
}

pub(super) fn plain(ch: char) -> KeyEvent {
    KeyEvent {
        key: Key::Text(ch.to_string()),
        mods: Mods::default(),
    }
}

pub(super) fn key(k: Key) -> KeyEvent {
    KeyEvent {
        key: k,
        mods: Mods::default(),
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

pub(super) fn with(k: Key, cmd: bool, shift: bool, ctrl: bool) -> KeyEvent {
    KeyEvent {
        key: k,
        mods: Mods {
            cmd,
            shift,
            ctrl,
            ..Mods::default()
        },
    }
}
