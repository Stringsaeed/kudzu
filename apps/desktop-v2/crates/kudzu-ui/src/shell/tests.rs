//! The shell on fixtures and a real temporary folder, without a running Rune loop: window
//! states are driven through the same methods their hooks and updates call.
use super::windows::{opens_at_launch, parts, Launch, Parts};
use super::{Identity, NewSnippet, Route};
use crate::theme::Appearance;
use kudzu_app::assembly::{Config, Host, Kudzu};
use kudzu_app::fixtures::{hotkey_registrar::FakeHotkeys, Fixtures};
use kudzu_app::library::SnippetDraft;
use kudzu_app::paths::Paths;
use kudzu_app::ports::{FormRequest, HotkeyRegistrar, ShortcutSlot};
use kudzu_app::settings::{Theme, Variant};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        let path = PathBuf::from("/tmp").join(format!("kudzu-shell-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn kudzu(dir: &std::path::Path, fixtures: &Fixtures) -> Arc<Kudzu> {
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

const LAUNCH: Launch = Launch {
    identity: Identity {
        app_name: "Kudzu Next",
        dev: true,
    },
    native: false,
    hidden: false,
};

struct Rig {
    _scratch: Scratch,
    dir: PathBuf,
    fixtures: Fixtures,
    hotkeys: Arc<FakeHotkeys>,
    kudzu: Arc<Kudzu>,
    p: Parts,
}

fn rig() -> Rig {
    let scratch = Scratch::new();
    let dir = scratch.0.clone();
    let fixtures = Fixtures::new().unwrap();
    let kudzu = kudzu(&dir, &fixtures);
    let hotkeys = Arc::new(FakeHotkeys::default());
    let registrar: Arc<dyn HotkeyRegistrar> = hotkeys.clone();
    let p = parts(&kudzu, LAUNCH, Some(registrar));
    Rig {
        _scratch: scratch,
        dir,
        fixtures,
        hotkeys,
        kudzu,
        p,
    }
}

fn wait_for(mut ready: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while !ready() {
        assert!(Instant::now() < deadline, "condition was not met");
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn draft(name: &str) -> SnippetDraft {
    SnippetDraft {
        name: name.into(),
        content: "draft body".into(),
        ..SnippetDraft::default()
    }
}

/// Reopen the same folder, as an immediate relaunch would, and look for the draft.
fn on_disk_after_relaunch(dir: &std::path::Path, name: &str) -> bool {
    let fixtures = Fixtures::new().unwrap();
    let again = kudzu(dir, &fixtures);
    let found = again
        .library
        .watch()
        .get()
        .snippets
        .iter()
        .any(|s| s.name == name);
    let _ = again.shutdown();
    found
}

#[test]
fn every_quit_and_hide_path_leaves_the_draft_on_disk() {
    // The tray's Quit: drafts, then shutdown, in one update on `main`.
    let mut r = rig();
    r.p.main.model.draft = Some(draft("tray quit"));
    r.p.main.quit();
    assert!(!r.kudzu.shutdown().ran, "the tray's quit already shut down");
    drop(r.p);
    drop(r.kudzu);
    assert!(on_disk_after_relaunch(&r.dir, "tray quit"));

    // A system quit (Dock, logout): Rune runs on_quit, the same sequence.
    let mut r = rig();
    r.p.main.model.draft = Some(draft("system quit"));
    r.p.main.quit();
    r.p.main.quit();
    assert!(!r.kudzu.shutdown().ran, "shutdown ran once");
    drop(r.p);
    drop(r.kudzu);
    assert!(on_disk_after_relaunch(&r.dir, "system quit"));

    // ⌘Q and Close Windows hide `main`, committing first; a later quit drains the writer.
    let mut r = rig();
    r.p.main.model.draft = Some(draft("command q"));
    r.p.shell.close_windows();
    r.p.main.hide();
    assert!(r.p.main.model.draft.is_none());
    assert!(r.kudzu.shutdown().ran);
    drop(r.p);
    drop(r.kudzu);
    assert!(on_disk_after_relaunch(&r.dir, "command q"));

    // The close button: Rune hides the window and runs on_close.
    let mut r = rig();
    r.p.main.model.draft = Some(draft("close button"));
    r.p.main.closed();
    assert!(r.kudzu.shutdown().ran);
    drop(r.p);
    drop(r.kudzu);
    assert!(on_disk_after_relaunch(&r.dir, "close button"));
}

fn unsuppressed(r: &Rig) -> bool {
    r.fixtures.engine.wait_unsuppressed(Duration::from_secs(5))
}

#[test]
fn every_palette_dismissal_lifts_suppression() {
    let mut r = rig();
    // Escape or the shortcut again: restore the target.
    r.p.palette.toggle();
    assert!(r.fixtures.engine.state.lock().unwrap().suppressed);
    wait_for(|| {
        r.p.palette.sync();
        r.p.palette.model.session.is_some()
    });
    r.p.palette.toggle();
    assert!(!r.p.palette.model.open);
    assert!(unsuppressed(&r));
    // A blur, before the capture even landed.
    r.p.palette.toggle();
    r.p.palette.dismiss(false);
    assert!(unsuppressed(&r));
    // Close Windows.
    r.p.palette.toggle();
    wait_for(|| {
        r.p.palette.sync();
        r.p.palette.model.session.is_some()
    });
    r.p.palette.dismiss(false);
    assert!(r.p.palette.model.session.is_none());
    assert!(unsuppressed(&r));
}

fn emit_form(r: &Rig, native: u64) -> kudzu_app::ports::FormToken {
    let snippet = r.kudzu.library.watch().get().snippets[0].id;
    let token = r.fixtures.engine.form_token(native);
    r.fixtures.engine.emit_form(FormRequest {
        token,
        snippet_id: snippet,
        snippet_name: "Form".into(),
        fields: vec![],
        target: None,
        caret: None,
        selection: None,
    });
    wait_for(|| r.kudzu.forms.pending().get().is_some());
    token
}

#[test]
fn form_dismissals_and_close_windows_cancel_once_and_lift_suppression() {
    let mut r = rig();
    // Escape in the form.
    let token = emit_form(&r, 1);
    assert!(r.p.form.sync(), "the form window shows itself");
    assert!(r.p.form.model.request.is_some());
    r.p.form.cancel();
    assert!(r.p.form.model.request.is_none());
    assert!(unsuppressed(&r));
    assert_eq!(
        r.fixtures.engine.state.lock().unwrap().cancelled,
        vec![token]
    );
    // ⌘Q / Close Windows: the pending form is cancelled even with its panel hidden.
    let second = emit_form(&r, 2);
    r.p.form.sync();
    r.p.shell.close_windows();
    wait_for(|| r.kudzu.forms.pending().get().is_none());
    assert!(unsuppressed(&r));
    assert!(r
        .fixtures
        .engine
        .state
        .lock()
        .unwrap()
        .cancelled
        .contains(&second));
    r.p.form.sync();
    assert!(
        r.p.form.model.request.is_none(),
        "it hides when pending clears"
    );
}

#[test]
fn picker_needs_recording_and_hides_when_it_turns_off() {
    let mut r = rig();
    r.p.main.sync();
    assert!(r
        .hotkeys
        .registered
        .lock()
        .unwrap()
        .contains_key(&ShortcutSlot::Clipboard));
    r.p.picker.toggle();
    assert!(r.p.picker.model.open);
    wait_for(|| {
        r.p.picker.sync();
        r.p.picker.model.session.is_some()
    });
    r.kudzu
        .settings
        .update(|s| s.clipboard_enabled = false)
        .unwrap();
    assert!(r.p.picker.sync());
    assert!(!r.p.picker.model.open);
    assert!(r.p.picker.model.session.is_none());
    r.p.picker.toggle();
    assert!(!r.p.picker.model.open, "unavailable while recording is off");
    // Turning recording off applies the slots again: the clipboard chord is released.
    r.p.main.sync();
    let registered = r.hotkeys.registered.lock().unwrap().clone();
    assert!(registered.contains_key(&ShortcutSlot::QuickSearch));
    assert!(!registered.contains_key(&ShortcutSlot::Clipboard));
}

#[test]
fn a_theme_change_reaches_every_window_in_one_poll() {
    let mut r = rig();
    r.p.main.sync();
    r.p.palette.sync();
    r.p.picker.sync();
    r.p.form.sync();
    r.p.permissions.sync();
    r.kudzu.settings.update(|s| s.theme = Theme::Light).unwrap();
    assert!(r.p.main.sync());
    assert!(r.p.palette.sync());
    assert!(r.p.picker.sync());
    assert!(r.p.form.sync());
    assert!(r.p.permissions.sync());
    for appearance in [
        r.p.main.model.theme.appearance(),
        r.p.palette.model.theme.appearance(),
        r.p.picker.model.theme.appearance(),
        r.p.form.model.theme.appearance(),
        r.p.permissions.model.theme.appearance(),
    ] {
        assert_eq!(appearance, Appearance::Light);
    }
}

#[test]
fn new_snippet_from_selection_routes_even_when_the_selection_is_empty() {
    let mut r = rig();
    r.p.main.begin_capture();
    wait_for(|| {
        r.p.main.sync();
        r.p.main.model.capture.is_none()
    });
    assert_eq!(
        r.p.main.model.route,
        Route::NewSnippet(NewSnippet::default())
    );
    *r.fixtures.apps.selection.lock().unwrap() = Some("picked text".into());
    r.p.main.begin_capture();
    wait_for(|| {
        r.p.main.sync();
        r.p.main.model.capture.is_none()
    });
    assert_eq!(
        r.p.main.model.route,
        Route::NewSnippet(NewSnippet {
            content: "picked text".into()
        })
    );
}

#[test]
fn hidden_is_ignored_on_first_run_and_the_route_follows_onboarding() {
    assert!(opens_at_launch(false, false));
    assert!(opens_at_launch(true, true));
    assert!(!opens_at_launch(true, false));
    let r = rig();
    let onboarded = r.kudzu.settings.watch().get().onboarding_completed;
    let expected = if onboarded {
        Route::Library
    } else {
        Route::Onboarding
    };
    assert_eq!(r.p.main.model.route, expected);
}

#[test]
fn the_first_poll_registers_both_hotkeys_and_pushes_no_window_yet() {
    let mut r = rig();
    assert!(r.p.main.sync());
    let registered = r.hotkeys.registered.lock().unwrap().clone();
    assert_eq!(
        registered
            .get(&ShortcutSlot::QuickSearch)
            .map(String::as_str),
        Some("Control+Alt+Shift+Space")
    );
    assert!(r.p.main.model.hotkey_error.is_none());
    // A rejected chord rolls back and is reported, not fatal.
    r.hotkeys
        .rejected
        .lock()
        .unwrap()
        .insert("Control+Alt+Shift+K".into());
    let set = r.kudzu.hotkeys.set(
        ShortcutSlot::QuickSearch,
        "Control+Alt+Shift+K",
        &*r.hotkeys,
    );
    assert!(set.is_err());
    assert_eq!(
        r.hotkeys
            .registered
            .lock()
            .unwrap()
            .get(&ShortcutSlot::QuickSearch)
            .map(String::as_str),
        Some("Control+Alt+Shift+Space")
    );
    r.p.shell.push_windows();
    let pushes = r.fixtures.engine.state.lock().unwrap().windows.clone();
    assert!(
        pushes.last().is_some_and(Vec::is_empty),
        "no native window exists"
    );
}
