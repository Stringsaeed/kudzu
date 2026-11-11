//! `LIB-14` shortcuts and their IME, text-field and modal guards.

use rune::Ime;

use super::{cmd, cmd_backspace, cmd_shift, esc, plain, Rig};
use crate::main::state::{ClipFilter, LibraryFilter, MainPane, MainView, MAX_NAV_HISTORY};
use crate::shell::SettingsPane;

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
    rig.press(cmd_backspace());
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
