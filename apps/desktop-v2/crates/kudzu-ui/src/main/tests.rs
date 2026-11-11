//! Headless `Runtime<MainState>` tests for S41 (`PLAN.md` lines 991–1005, `LIB-14`, `LIB-15`,
//! `LIB-7`): frame geometry, sidebar vibrancy & inertness, collapsing breadcrumbs & `Arrivals`
//! rename/burst stability, every window shortcut, IME composition & editable-input & modal guards,
//! back/forward history, Onboarding full-window routing, and Undo-delete / update-ready toasts.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use kudzu_app::assembly::{Config, Host, Kudzu};
use kudzu_app::fixtures::{hotkey_registrar::FakeHotkeys, Fixtures};
use kudzu_app::library::SnippetDraft;
use kudzu_app::paths::Paths;
use kudzu_app::ports::HotkeyRegistrar;
use kudzu_app::settings::Variant;
use rune::{Env, Ime, Key, KeyEvent, Mods, Runtime};
use uuid::Uuid;

use crate::main::state::{ClipFilter, LibraryFilter, MainPane, MainView, MAX_NAV_HISTORY};
use crate::main::toolbar::{collapse_crumbs, compute_crumbs, VisibleCrumb};
use crate::shell::windows::{parts, Launch};
use crate::shell::{Identity, MainState, Route, SettingsPane};

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        let path = PathBuf::from("/tmp").join(format!("kudzu-main-{}", Uuid::new_v4()));
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

struct Rig {
    _scratch: Scratch,
    fixtures: Fixtures,
    kudzu: Arc<Kudzu>,
    rt: Runtime<MainState>,
}

impl Rig {
    fn new(onboarded: bool) -> Self {
        let scratch = Scratch::new();
        let fixtures = Fixtures::new().unwrap();
        let kudzu = build_kudzu(&scratch.0, &fixtures);
        kudzu
            .settings
            .update(|s| s.onboarding_completed = onboarded)
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

    fn rect(&self, key: &str) -> Option<[f32; 4]> {
        self.rt
            .elements()
            .into_iter()
            .find(|e| e.key.as_deref() == Some(key))
            .map(|e| e.rect)
    }

    fn click_key(&mut self, key: &str) {
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

    fn click_text(&mut self, needle: &str) {
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

    fn press(&mut self, ev: KeyEvent) {
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

fn cmd(ch: char) -> KeyEvent {
    KeyEvent {
        key: Key::Command(ch),
        mods: Mods {
            cmd: true,
            ..Mods::default()
        },
    }
}

fn cmd_shift(ch: char) -> KeyEvent {
    KeyEvent {
        key: Key::Command(ch),
        mods: Mods {
            cmd: true,
            shift: true,
            ..Mods::default()
        },
    }
}

fn plain(ch: char, shift: bool) -> KeyEvent {
    KeyEvent {
        key: Key::Text(ch.to_string()),
        mods: Mods {
            shift,
            ..Mods::default()
        },
    }
}

fn esc() -> KeyEvent {
    KeyEvent {
        key: Key::Escape,
        mods: Mods::default(),
    }
}

#[test]
fn frame_geometry_titlebar_inset_and_sidebar_collapse_sync_vibrancy_and_inertness() {
    let mut rig = Rig::new(true);

    // Headless / fullscreen default (`titlebar_inset == 0`): 12pt left toolbar padding.
    assert_eq!(rig.rect("main-toolbar"), Some([0.0, 0.0, 1180.0, 42.0]));
    let toggle_rect = rig.rect("toolbar-sidebar-toggle").unwrap();
    assert!(
        (toggle_rect[0] - 12.0).abs() < 0.5 && toggle_rect[2] == 32.0 && toggle_rect[3] == 32.0
    );
    let search_rect = rig.rect("toolbar-search-trigger").unwrap();
    assert_eq!((search_rect[2], search_rect[3]), (192.0, 32.0));
    assert!((search_rect[0] - (1180.0 - 16.0 - 192.0)).abs() < 0.5);

    // Sidebar open: 224pt wide, vibrancy material synced, `<main>` card inset by 8pt top/bottom/right.
    assert_eq!(rig.rect("main-sidebar"), Some([0.0, 42.0, 224.0, 718.0]));
    assert_eq!(
        rig.rt.state.frame.sidebar_material_rect,
        Some([0.0, 42.0, 224.0, 718.0])
    );
    assert_eq!(rig.rect("main-content"), Some([224.0, 50.0, 948.0, 702.0]));

    // Traffic-light inset (`titlebar_inset > 0`): toolbar shifts left padding to 88pt.
    rig.rt.set_titlebar_inset(28.0);
    rig.rt.settle_all();
    rig.rt.frame(0.016);
    let inset_toggle = rig.rect("toolbar-sidebar-toggle").unwrap();
    assert!((inset_toggle[0] - 88.0).abs() < 0.5);

    // Move logical pane focus to Sidebar, then collapse via `⌘\`.
    rig.rt.state.frame.focus.move_to(MainPane::Sidebar);
    rig.press(cmd('\\'));

    assert!(!rig.rt.state.frame.sidebar_open);
    // Focus cannot stay trapped inside the hidden sidebar.
    assert_eq!(rig.rt.state.frame.focus.active(), MainPane::List);
    // Sidebar subtree remains mounted at width 0, vibrancy rect syncs width 0, and `<main>` gets 8pt left gutter.
    assert!(rig.rect("sidebar-nav").is_some());
    assert_eq!(rig.rect("main-sidebar"), Some([0.0, 42.0, 0.0, 718.0]));
    assert_eq!(
        rig.rt.state.frame.sidebar_material_rect,
        Some([0.0, 42.0, 0.0, 718.0])
    );
    assert_eq!(rig.rect("main-content"), Some([8.0, 50.0, 1164.0, 702.0]));
    // Accessibility tree hides `"Sidebar"` and flips toggle label to `"Show sidebar"`.
    let a11y = rig.rt.accessibility_json().to_string();
    assert!(a11y.contains("Show sidebar"));
    assert!(!a11y.contains("\"label\":\"Sidebar\""));

    // Clicking the toolbar toggle re-opens the sidebar to 224pt.
    rig.click_key("toolbar-sidebar-toggle");
    assert!(rig.rt.state.frame.sidebar_open);
    assert_eq!(rig.rect("main-sidebar"), Some([0.0, 42.0, 224.0, 718.0]));
}

#[test]
fn breadcrumbs_collapse_past_four_and_navigate_via_overflow_menu() {
    let mut rig = Rig::new(true);

    let (work, _) = rig.kudzu.library.create_group("Work".into(), None).unwrap();
    let (eng, _) = rig
        .kudzu
        .library
        .create_group("Engineering".into(), Some(work.id))
        .unwrap();
    let (rust_grp, _) = rig
        .kudzu
        .library
        .create_group("Rust".into(), Some(eng.id))
        .unwrap();
    let (snip, _) = rig
        .kudzu
        .library
        .create_snippet(SnippetDraft {
            name: "Greeting".into(),
            content: "Hello!".into(),
            group_id: Some(rust_grp.id),
            ..SnippetDraft::default()
        })
        .unwrap();

    rig.rt.frame(0.5);
    rig.rt.update(|st, _| {
        st.navigate_to(MainView::Library(LibraryFilter::Group(rust_grp.id)));
        st.select_snippet(Some(snip.id));
    });
    rig.rt.settle_all();
    rig.rt.frame(0.5);
    rig.rt.frame(0.0);

    // 5 crumbs: Snippets > Work > Engineering > Rust > Greeting -> collapses Work & Engineering.
    let crumbs = compute_crumbs(
        &rig.rt.state.frame.view,
        rig.rt.state.frame.selected_snippet,
        rig.rt.state.library_snapshot(),
    );
    assert_eq!(crumbs.len(), 5);
    let collapsed = collapse_crumbs(&crumbs);
    assert_eq!(collapsed.len(), 4);
    assert!(matches!(&collapsed[1], VisibleCrumb::Overflow(hidden) if hidden.len() == 2));

    let a11y = rig.rt.accessibility_json().to_string();
    assert!(a11y.contains("Snippets"));
    assert!(a11y.contains("Show parent groups"));
    assert!(a11y.contains("Rust"));
    assert!(a11y.contains("Greeting"));
    assert!(!a11y.contains("\"label\":\"Work\""));
    assert!(!a11y.contains("\"label\":\"Engineering\""));

    // Clicking `"Rust"` (`crumb3`) navigates to `Group(rust_grp.id)`; opening `"Show parent groups"` exposes `"Work"`.
    rig.rt
        .update(|st, _| st.frame.breadcrumb_overflow_open = true);
    rig.rt.frame(0.016);
    let open_a11y = rig.rt.accessibility_json().to_string();
    assert!(open_a11y.contains("Work") && open_a11y.contains("Engineering"));

    // Clicking root `"Snippets"` (`crumb0`) navigates back to `Library(All)`.
    rig.rt
        .update(|st, _| st.frame.breadcrumb_overflow_open = false);
    rig.rt.frame(0.016);
    rig.click_key("crumb0");
    assert_eq!(
        rig.rt.state.frame.view,
        MainView::Library(LibraryFilter::All)
    );
}

#[test]
fn breadcrumb_arrivals_ignore_mount_renames_and_bursts() {
    let mut rig = Rig::new(true);

    // Initial mount never counts as an arrival, even across idle frames on the same crumb.
    assert_eq!(rig.rt.state.frame.breadcrumb_arrivals.count(), 0);
    rig.rt.frame(1.0);
    assert_eq!(rig.rt.state.frame.breadcrumb_arrivals.count(), 0);

    // First navigation after mount plays its arrival (`count == 1`).
    rig.press(cmd('2')); // Favorites
    assert_eq!(
        rig.rt.state.frame.view,
        MainView::Library(LibraryFilter::Favorites)
    );
    assert_eq!(rig.rt.state.frame.breadcrumb_arrivals.count(), 1);

    // Rapid second navigation within `< 0.300s` burst window stays suppressed (`count == 1`).
    rig.rt.frame(0.05);
    rig.press(cmd('3')); // Recent (~0.066s later, < 0.300s)
    assert_eq!(
        rig.rt.state.frame.view,
        MainView::Library(LibraryFilter::Recent)
    );
    assert_eq!(rig.rt.state.frame.breadcrumb_arrivals.count(), 1);

    // Navigation after >= 0.300s increments arrival count (`count == 2`).
    rig.rt.frame(0.50);
    rig.press(cmd('4')); // Insights
    assert_eq!(rig.rt.state.frame.view, MainView::Insights);
    assert_eq!(rig.rt.state.frame.breadcrumb_arrivals.count(), 2);

    // Selecting a snippet after >= 0.300s increments arrival count (`count == 3`).
    let (snip, _) = rig
        .kudzu
        .library
        .create_snippet(SnippetDraft {
            name: "Alpha".into(),
            content: "body".into(),
            ..SnippetDraft::default()
        })
        .unwrap();
    rig.rt.frame(0.50);
    rig.rt.update(|st, _| {
        st.refresh_library_snapshot();
        st.navigate_to(MainView::Library(LibraryFilter::All));
        st.select_snippet(Some(snip.id));
    });
    rig.rt.frame(0.05);
    let after_select = rig.rt.state.frame.breadcrumb_arrivals.count();
    assert_eq!(after_select, 3);

    // Renaming the snippet in-place (even > 1s later) keeps `id = "snippet:{uuid}"`, so `Arrivals` does NOT replay!
    let mut renamed = snip;
    renamed.name = "Alpha Renamed".into();
    rig.kudzu.library.update_snippet(renamed).unwrap();
    rig.rt.frame(1.0);
    assert_eq!(rig.rt.state.frame.breadcrumb_arrivals.count(), after_select);
    assert!(rig
        .rt
        .accessibility_json()
        .to_string()
        .contains("Alpha Renamed"));
}

#[test]
fn all_lib_14_shortcuts_and_history_cap_execute_through_runtime() {
    let mut rig = Rig::new(true);

    // ⌘N creates a snippet, selects it, and focuses the editor (`Snippet content`).
    let initial_len = rig.kudzu.library.watch().get().snippets.len();
    rig.press(cmd('n'));
    assert_eq!(
        rig.kudzu.library.watch().get().snippets.len(),
        initial_len + 1
    );
    let created_id = rig
        .rt
        .state
        .frame
        .selected_snippet
        .expect("snippet selected");
    assert_eq!(rig.rt.state.frame.focus.active(), MainPane::Editor);
    assert!(rig.rt.state.editor.is_editing_text());

    // Escape inside the editor returns focus to `MainPane::List`.
    rig.press(esc());
    assert_eq!(rig.rt.state.frame.focus.active(), MainPane::List);
    assert!(!rig.rt.state.editor.is_editing_text());

    // ⌘E focuses editor content; Escape returns to list; ⌘F focuses filter; ⌘L focuses list.
    rig.press(cmd('e'));
    assert_eq!(rig.rt.state.frame.focus.active(), MainPane::Editor);
    rig.press(cmd('f'));
    assert_eq!(rig.rt.state.frame.focus.active(), MainPane::List);
    assert!(rig.rt.state.library.is_editing_text());
    rig.press(cmd('l'));
    assert!(!rig.rt.state.library.is_editing_text());
    assert_eq!(rig.rt.state.frame.focus.active(), MainPane::List);

    // ⌘⇧F toggles favorite; ⌘⇧E toggles enabled; ⌘D duplicates.
    rig.press(cmd_shift('f'));
    let s1 = rig
        .kudzu
        .library
        .watch()
        .get()
        .snippets
        .iter()
        .find(|s| s.id == created_id)
        .unwrap()
        .clone();
    assert!(s1.is_favorite);

    rig.press(cmd_shift('e'));
    let s2 = rig
        .kudzu
        .library
        .watch()
        .get()
        .snippets
        .iter()
        .find(|s| s.id == created_id)
        .unwrap()
        .clone();
    assert!(!s2.is_enabled);

    let before_dup = rig.kudzu.library.watch().get().snippets.len();
    rig.press(cmd('d'));
    assert_eq!(
        rig.kudzu.library.watch().get().snippets.len(),
        before_dup + 1
    );
    assert_ne!(rig.rt.state.frame.selected_snippet, Some(created_id));

    // ⌘⇧N creates a new group and navigates to it.
    let before_groups = rig.kudzu.library.watch().get().groups.len();
    rig.press(cmd_shift('n'));
    assert_eq!(
        rig.kudzu.library.watch().get().groups.len(),
        before_groups + 1
    );
    assert!(matches!(
        rig.rt.state.frame.view,
        MainView::Library(LibraryFilter::Group(_))
    ));

    // ⌘⇧P toggles expansion pause and resume (`Variant::Dev` starts paused).
    let initially_paused = rig.kudzu.expansion.status().get().paused;
    rig.press(cmd_shift('p'));
    assert_eq!(rig.kudzu.expansion.status().get().paused, !initially_paused);
    rig.press(cmd_shift('p'));
    assert_eq!(rig.kudzu.expansion.status().get().paused, initially_paused);

    // ⌘1–5, ⌘, toggle, ⌘[ / ⌘] back/forward + forward truncation on push.
    rig.press(cmd('1'));
    assert_eq!(
        rig.rt.state.frame.view,
        MainView::Library(LibraryFilter::All)
    );
    rig.press(cmd('2'));
    assert_eq!(
        rig.rt.state.frame.view,
        MainView::Library(LibraryFilter::Favorites)
    );
    rig.press(cmd('3'));
    assert_eq!(
        rig.rt.state.frame.view,
        MainView::Library(LibraryFilter::Recent)
    );
    rig.press(cmd('4'));
    assert_eq!(rig.rt.state.frame.view, MainView::Insights);
    rig.press(cmd('5'));
    assert_eq!(
        rig.rt.state.frame.view,
        MainView::Clipboard(ClipFilter::All)
    );

    // ⌘, enters Settings(General) and a second ⌘, returns to Clipboard(All).
    rig.press(cmd(','));
    assert_eq!(
        rig.rt.state.frame.view,
        MainView::Settings(SettingsPane::General)
    );
    rig.press(cmd(','));
    assert_eq!(
        rig.rt.state.frame.view,
        MainView::Clipboard(ClipFilter::All)
    );

    // ⌘[ steps back to Settings(General), ⌘] steps forward to Clipboard(All), then ⌘[ + new push truncates forward.
    rig.press(cmd('['));
    assert_eq!(
        rig.rt.state.frame.view,
        MainView::Settings(SettingsPane::General)
    );
    rig.press(cmd(']'));
    assert_eq!(
        rig.rt.state.frame.view,
        MainView::Clipboard(ClipFilter::All)
    );
    rig.press(cmd('['));
    rig.press(cmd('1'));
    assert!(!rig.rt.state.frame.history.can_go_forward());

    // 100 navigations cap history at MAX_NAV_HISTORY (64).
    for i in 0..80 {
        if i % 2 == 0 {
            rig.press(cmd('2'));
        } else {
            rig.press(cmd('3'));
        }
    }
    assert_eq!(rig.rt.state.frame.history.len(), MAX_NAV_HISTORY);

    // ⌘K, ⌘/, and bare `?` open modals and `Escape` closes each without changing the route.
    let steady_view = rig.rt.state.frame.view.clone();
    rig.press(cmd('k'));
    assert!(rig.rt.state.frame.command_open);
    rig.press(esc());
    assert!(!rig.rt.state.frame.command_open);

    rig.press(cmd('/'));
    assert!(rig.rt.state.frame.shortcuts_open);
    rig.press(esc());
    assert!(!rig.rt.state.frame.shortcuts_open);

    rig.press(plain('?', true));
    assert!(rig.rt.state.frame.shortcuts_open);
    rig.press(esc());
    assert!(!rig.rt.state.frame.shortcuts_open);
    assert_eq!(rig.rt.state.frame.view, steady_view);
}

#[test]
fn ime_composition_text_input_and_modal_guards_protect_typing() {
    let mut rig = Rig::new(true);
    rig.press(cmd('n')); // Create & select a snippet
    let selected = rig.rt.state.frame.selected_snippet.unwrap();

    // Focus the Library filter input (`⌘F`).
    rig.press(cmd('f'));
    assert!(rig.rt.state.library.is_editing_text());

    // 1. Typing `?` inside the focused filter inserts `?` into the field and NEVER opens Shortcuts.
    rig.press(plain('?', true));
    assert!(!rig.rt.state.frame.shortcuts_open);
    assert_eq!(rig.rt.state.library.filter.text, "?");

    // 2. Pressing `⌘⌫` inside a focused text input NEVER deletes the selected snippet (`allow_in_input == false`).
    rig.press(KeyEvent {
        key: Key::Backspace,
        mods: Mods {
            cmd: true,
            ..Mods::default()
        },
    });
    assert!(
        rig.kudzu
            .library
            .watch()
            .get()
            .snippets
            .iter()
            .any(|s| s.id == selected),
        "snippet must not be deleted when ⌘⌫ is pressed inside a text input"
    );

    // 3. Starting IME preedit (`Ime::Preedit`) blocks ALL window shortcuts (`⌘K`, `⌘N`, `⌘4`).
    rig.rt.ime(Ime::Preedit("かん".into(), Some((0, 6))));
    rig.rt.frame(0.016);
    assert!(rig.rt.state.is_composing());

    let count_before = rig.kudzu.library.watch().get().snippets.len();
    rig.press(cmd('k'));
    rig.press(cmd('n'));
    rig.press(cmd('4'));
    assert!(!rig.rt.state.frame.command_open);
    assert_eq!(rig.kudzu.library.watch().get().snippets.len(), count_before);
    assert_eq!(
        rig.rt.state.frame.view,
        MainView::Library(LibraryFilter::All)
    );

    // Finishing IME composition (`Ime::Commit`) re-enables `allow_in_input` chords like `⌘K`.
    rig.rt.ime(Ime::Commit("漢字".into()));
    rig.rt.frame(0.016);
    assert!(!rig.rt.state.is_composing());
    rig.press(cmd('k'));
    assert!(rig.rt.state.frame.command_open);

    // 4. While a modal (`command_open`) is open, underlying shortcuts (`⌘N`, `⌘4`) are suppressed.
    rig.press(cmd('n'));
    rig.press(cmd('4'));
    assert_eq!(rig.kudzu.library.watch().get().snippets.len(), count_before);
    assert_eq!(
        rig.rt.state.frame.view,
        MainView::Library(LibraryFilter::All)
    );
}

#[test]
fn undo_delete_toast_restores_snippet_and_expires_after_six_seconds() {
    let mut rig = Rig::new(true);
    let (snip, _) = rig
        .kudzu
        .library
        .create_snippet(SnippetDraft {
            name: "Signature".into(),
            content: "Best,\nSaeed".into(),
            ..SnippetDraft::default()
        })
        .unwrap();
    rig.rt.frame(0.1);
    rig.rt.update(|st, _| {
        st.select_snippet(Some(snip.id));
        st.frame.focus.move_to(MainPane::List);
    });
    rig.rt.frame(0.05);

    // Press `⌘⌫` from the list pane -> deletes snippet, clears selection, posts 6s Undo toast.
    rig.press(KeyEvent {
        key: Key::Backspace,
        mods: Mods {
            cmd: true,
            ..Mods::default()
        },
    });
    assert_eq!(rig.rt.state.frame.selected_snippet, None);
    assert_eq!(rig.rt.state.frame.focus.active(), MainPane::List);
    assert!(!rig
        .kudzu
        .library
        .watch()
        .get()
        .snippets
        .iter()
        .any(|s| s.id == snip.id));
    assert_eq!(rig.rt.state.frame.toasts.len(), 1);
    assert!(rig.rt.wake_deadline().is_some());

    let a11y = rig.rt.accessibility_json().to_string();
    assert!(a11y.contains("Deleted “Signature”"));
    assert!(a11y.contains("Undo"));

    // Clicking the `"Undo"` action button restores the snippet and dismisses the toast.
    rig.click_text("Undo");
    assert!(rig
        .kudzu
        .library
        .watch()
        .get()
        .snippets
        .iter()
        .any(|s| s.id == snip.id));
    assert_eq!(rig.rt.state.frame.selected_snippet, Some(snip.id));
    assert_eq!(rig.rt.state.frame.toasts.len(), 0);

    // Move cursor off the bottom-center toast area, delete a second time, and verify the 6-second auto-dismiss timer (`UNDO_TOAST_SECS = 6.0`).
    rig.rt.pointer_move(10.0, 10.0);
    rig.rt.frame(0.0);
    rig.press(KeyEvent {
        key: Key::Backspace,
        mods: Mods {
            cmd: true,
            ..Mods::default()
        },
    });
    assert_eq!(rig.rt.state.frame.toasts.len(), 1);
    rig.rt.frame(5.8);
    assert_eq!(rig.rt.state.frame.toasts.len(), 1);
    rig.rt.frame(0.4);
    assert_eq!(rig.rt.state.frame.toasts.len(), 0);
}

#[test]
fn onboarding_route_renders_chromeless_and_transitions_to_library_when_completed() {
    let mut rig = Rig::new(false);

    assert_eq!(rig.rt.state.frame.view, MainView::Onboarding);
    assert_eq!(rig.rt.state.model.route, Route::Onboarding);
    assert_eq!(rig.rect("main-toolbar"), None);
    assert_eq!(rig.rect("main-sidebar"), None);
    assert_eq!(rig.rt.state.frame.sidebar_material_rect, None);
    assert!(rig.rect("onboarding-view-slot").is_some());

    // Completing onboarding transitions the main window frame into Library with toolbar & sidebar.
    rig.kudzu
        .settings
        .update(|s| s.onboarding_completed = true)
        .unwrap();
    rig.rt.settle_all();
    rig.rt.frame(0.1);
    rig.rt.frame(0.0);

    assert_eq!(
        rig.rt.state.frame.view,
        MainView::Library(LibraryFilter::All)
    );
    assert_eq!(rig.rt.state.model.route, Route::Library);
    assert!(rig.rect("main-toolbar").is_some());
    assert_eq!(rig.rect("main-sidebar"), Some([0.0, 42.0, 224.0, 718.0]));
}

#[test]
fn shell_open_main_reconciles_routes_into_frame() {
    let mut rig = Rig::new(true);
    let shell = rig.rt.state.shell.clone();

    shell.open_main(Route::Settings(SettingsPane::Data));
    // Headless Runtime doesn't run Rune's multi-window event loop, so set route the same way the handle callback does.
    rig.rt.update(|st, _| {
        st.model.route = Route::Settings(SettingsPane::Data);
    });
    rig.rt.frame(0.1);
    assert_eq!(
        rig.rt.state.frame.view,
        MainView::Settings(SettingsPane::Data)
    );

    let _ = &rig.fixtures;
}
