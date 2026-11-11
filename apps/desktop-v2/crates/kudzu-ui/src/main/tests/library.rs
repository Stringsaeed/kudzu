//! Snippet actions: new, delete and the Undo toast.

use super::{cmd_backspace, Rig};
use crate::main::state::MainPane;
use kudzu_app::library::SnippetDraft;

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
    rig.press(cmd_backspace());
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
    rig.press(cmd_backspace());
    assert_eq!(rig.rt.state.frame.toasts.len(), 1);
    rig.rt.frame(5.8);
    assert_eq!(rig.rt.state.frame.toasts.len(), 1);
    rig.rt.frame(0.4);
    assert_eq!(rig.rt.state.frame.toasts.len(), 0);
}
