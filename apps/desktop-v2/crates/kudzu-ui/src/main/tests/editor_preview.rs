//! S51 preview (EDT-16): the 120 ms debounce, stale answers dropped, the sample selection, the
//! four badges' copy, "Nothing to show yet.", and the settle cue after 600 ms of stillness.

use super::editor::{has_text, open, rig_with, snippet, type_str, wait_preview};
use super::Rig;
use crate::main::editor::{self, preview};

fn shown(rig: &Rig) -> Option<String> {
    rig.rt
        .state
        .editor
        .preview
        .shown
        .as_ref()
        .map(|r| r.text.clone())
}

fn focus_content(rig: &mut Rig) {
    rig.press(super::cmd('e'));
    assert_eq!(rig.focused_key().as_deref(), Some(editor::CONTENT_KEY));
}

#[test]
fn the_preview_renders_120_ms_after_the_last_edit() {
    let mut rig = rig_with(vec![snippet(1, "A", &[";a"], "Hello")], vec![]);
    wait_preview(&mut rig);
    assert_eq!(
        shown(&rig).as_deref(),
        Some("Hello"),
        "the first render goes at once"
    );
    focus_content(&mut rig);
    type_str(&mut rig, "!");
    let typed_at = rig.rt.state.frame.clock_secs;
    let due = rig.rt.state.editor.preview.due().expect("a render is due");
    assert!((due - typed_at - preview::DEBOUNCE).abs() < 1e-6);
    rig.rt.frame(0.1);
    assert_eq!(
        rig.rt.state.editor.preview.due(),
        Some(due),
        "not sent before 120 ms"
    );
    assert_eq!(shown(&rig).as_deref(), Some("Hello"));
    rig.rt.frame(0.03);
    assert_eq!(rig.rt.state.editor.preview.due(), None, "sent");
    wait_preview(&mut rig);
    assert_eq!(shown(&rig).as_deref(), Some("Hello!"));
}

#[test]
fn an_answer_for_another_snippet_is_dropped() {
    let mut rig = rig_with(
        vec![
            snippet(1, "A", &[";a"], "first"),
            snippet(2, "B", &[";b"], "second"),
        ],
        vec![],
    );
    wait_preview(&mut rig);
    focus_content(&mut rig);
    type_str(&mut rig, "!");
    rig.rt.frame(0.13);
    open(&mut rig, 2);
    wait_preview(&mut rig);
    assert_eq!(shown(&rig).as_deref(), Some("second"));
}

#[test]
fn the_sample_selection_and_the_empty_copy_show() {
    let mut rig = rig_with(
        vec![
            snippet(1, "Sel", &[";s"], "[{{selection}}]"),
            snippet(2, "Empty", &[";e"], ""),
        ],
        vec![],
    );
    wait_preview(&mut rig);
    assert_eq!(shown(&rig).as_deref(), Some("[selected text]"));
    open(&mut rig, 2);
    wait_preview(&mut rig);
    assert!(has_text(&rig, preview::EMPTY));
    assert!(has_text(&rig, "Nothing to show yet."));
}

#[test]
fn the_badges_show_tauris_copy() {
    let content =
        r#"{{field name="Name"}} {{clipboard}} {{selection}} {{cursor}}{{key name="tab"}}"#;
    let mut rig = rig_with(vec![snippet(1, "All", &[";all"], content)], vec![]);
    wait_preview(&mut rig);
    for copy in [
        "Asks for Name",
        "Uses clipboard",
        "Selection: Quick Search only",
        "Presses keys",
        "Cursor before a key press is ignored",
    ] {
        assert!(has_text(&rig, copy), "badge {copy}");
    }
    let helps: Vec<String> = rig
        .rt
        .elements()
        .into_iter()
        .filter_map(|e| e.help)
        .collect();
    assert!(helps
        .iter()
        .any(|h| h
            == "Typing a trigger replaces the selection, so the selected text is empty there."));
    assert!(helps.iter().any(|h| h == "Key presses are sent in order with the text. This snippet can't be undone with Backspace."));
    assert!(helps
        .iter()
        .any(|h| h == "The caret can only be placed in the text after the last key press."));
}

#[test]
fn a_change_after_stillness_settles_and_the_cue_expires() {
    let mut rig = rig_with(vec![snippet(1, "A", &[";a"], "a")], vec![]);
    wait_preview(&mut rig);
    focus_content(&mut rig);
    rig.rt.frame(0.7);
    type_str(&mut rig, "b");
    rig.rt.frame(0.13);
    wait_preview(&mut rig);
    assert_eq!(shown(&rig).as_deref(), Some("ab"));
    assert_eq!(
        rig.rt.state.editor.preview.cues, 1,
        "a change after stillness"
    );
    assert!(rig.rt.state.editor.preview.cue.running());
    type_str(&mut rig, "c");
    rig.rt.frame(0.13);
    wait_preview(&mut rig);
    assert_eq!(shown(&rig).as_deref(), Some("abc"));
    assert_eq!(rig.rt.state.editor.preview.cues, 1, "typing stays still");
    rig.rt.frame(0.2);
    rig.rt.frame(0.0);
    assert!(
        !rig.rt.state.editor.preview.cue.running(),
        "the settle cue expires"
    );
}

/// Headless tooling renders on the UI thread: the first settled frame already shows the text.
#[test]
fn headless_runs_render_the_preview_without_a_worker() {
    let mut rig = rig_with(
        vec![
            snippet(1, "A", &[";a"], "first"),
            snippet(2, "B", &[";b"], "second"),
        ],
        vec![],
    );
    rig.rt.state.editor.preview.inline = true;
    open(&mut rig, 2);
    assert_eq!(shown(&rig).as_deref(), Some("second"));
    assert!(!rig.rt.state.editor.preview.is_pending());
    assert!(has_text(&rig, "second"));
}
