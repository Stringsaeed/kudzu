//! Routes `Shell::open_main` sends.

use super::Rig;
use crate::main::editor;
use crate::main::state::{LibraryFilter, MainPane, MainView};
use crate::shell::{NewSnippet, Route, SettingsPane};

#[test]
fn shell_open_main_reconciles_routes_into_frame() {
    let mut rig = Rig::new(true);
    let shell = rig.rt.state.shell.clone();

    shell.open_main(Route::Settings(SettingsPane::Data));
    // The headless Runtime doesn't run Rune's window loop, so apply the route as the handle's
    // callback does.
    rig.open_route(Route::Settings(SettingsPane::Data));
    assert_eq!(
        rig.rt.state.frame.view,
        MainView::Settings(SettingsPane::Data)
    );

    let _ = &rig.fixtures;
}

/// B1 (S41 review): Quick Search ⌘N sends `Route::NewSnippet` with a trigger or a name. The
/// window creates that snippet (the trigger gets the default prefix, as `store.ts` does),
/// selects it, leaves screens outside the library for All, and focuses its name.
#[test]
fn a_quick_search_new_snippet_route_creates_selects_and_focuses_its_name() {
    let mut rig = Rig::new(true);
    rig.press(super::cmd('4'));
    assert_eq!(rig.rt.state.frame.view, MainView::Insights);
    let before = rig.library().snippets.len();

    rig.open_route(Route::NewSnippet(NewSnippet {
        trigger: "brb".into(),
        ..NewSnippet::default()
    }));
    let lib = rig.library();
    assert_eq!(lib.snippets.len(), before + 1);
    let created = &lib.snippets[0];
    assert_eq!(created.triggers, vec![";brb".to_owned()]);
    assert_eq!(rig.rt.state.frame.selected_snippet, Some(created.id));
    assert_eq!(
        rig.rt.state.frame.view,
        MainView::Library(LibraryFilter::All)
    );
    assert_eq!(rig.rt.state.frame.focus.active(), MainPane::Editor);
    assert_eq!(rig.focused_key().as_deref(), Some(editor::NAME_KEY));

    // A trigger that already has the prefix keeps it; a name seed sets the name.
    rig.open_route(Route::NewSnippet(NewSnippet {
        trigger: ";omw".into(),
        ..NewSnippet::default()
    }));
    assert_eq!(rig.library().snippets[0].triggers, vec![";omw".to_owned()]);
    rig.open_route(Route::NewSnippet(NewSnippet {
        name: "Follow-up".into(),
        ..NewSnippet::default()
    }));
    let named = rig.library().snippets[0].clone();
    assert_eq!(
        (named.name.as_str(), named.triggers.len()),
        ("Follow-up", 0)
    );
    assert_eq!(rig.rt.state.frame.selected_snippet, Some(named.id));
    assert_eq!(rig.rt.state.editor.focus_name_of, Some(named.id));
}

/// B1: "New Snippet from Selection" lands as a snippet holding the captured text, selected.
#[test]
fn new_snippet_from_selection_creates_the_snippet_with_the_captured_content() {
    let mut rig = Rig::new(true);
    *rig.fixtures.apps.selection.lock().unwrap() = Some("picked text".into());
    rig.rt.update(|st, _| st.begin_capture());
    for _ in 0..500 {
        rig.settle();
        if rig.rt.state.model.capture.is_none() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    rig.settle();
    let created = rig.library().snippets[0].clone();
    assert_eq!(created.content, "picked text");
    assert_eq!(rig.rt.state.frame.selected_snippet, Some(created.id));
    assert_eq!(rig.focused_key().as_deref(), Some(editor::NAME_KEY));
}

/// B1: a route is a request, so the same one sent twice applies twice: two empty captures make
/// two snippets, and "What's new" reopens About after the user moved on.
#[test]
fn the_same_route_sent_twice_applies_twice() {
    let mut rig = Rig::new(true);
    let before = rig.library().snippets.len();
    rig.open_route(Route::NewSnippet(NewSnippet::default()));
    rig.open_route(Route::NewSnippet(NewSnippet::default()));
    assert_eq!(rig.library().snippets.len(), before + 2);

    rig.open_route(Route::Settings(SettingsPane::About));
    rig.press(super::cmd('1'));
    rig.open_route(Route::Settings(SettingsPane::About));
    assert_eq!(
        rig.rt.state.frame.view,
        MainView::Settings(SettingsPane::About)
    );
}
