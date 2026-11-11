//! EXP-13, image delivery. How long a pasted image is in the document depends on the app: an
//! attachment character, or the text alternative. The engine compares the caret's movement with
//! both. When neither is confirmed it doesn't guess: no cursor move, no Backspace undo.

use kudzu_core::formatting::{FormatSpan, FormatStyle, Formatting};
use kudzu_core::SnippetRecord;

use super::desk::Out;
use super::rig::{backspaces, lefts, snippet, Rig};

/// A 1×1 PNG.
const PNG: &str = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNkYPhfDwAChwGA60e6kgAAAABJRU5ErkJggg==";

/// "Hi [Logo] there" with the image over "[Logo]" and the cursor after it.
fn logo() -> SnippetRecord {
    let mut snippet = snippet(";img", "Hi [Logo]{{cursor}} there");
    snippet.formatting = Some(Formatting {
        source: snippet.content.clone(),
        spans: vec![FormatSpan {
            start: 3,
            end: 9,
            style: FormatStyle::Image {
                data_url: PNG.into(),
                width: 16,
            },
        }],
    });
    snippet
}

/// Expands `;img` in a target set up by `app`.
fn pasted_into(app: impl FnOnce(&mut super::document::Document)) -> Rig {
    let rig = Rig::with(&[logo()], |d| d.with(|s| app(&mut s.doc)));
    rig.type_(";img");
    rig
}

#[test]
fn an_attachment_paste_is_confirmed_moves_the_cursor_and_can_be_undone() {
    let rig = pasted_into(|doc| doc.paste_inserts = Some("Hi \u{fffc} there".into()));
    let expected = [backspaces(3), vec![Out::Paste], lefts(6)].concat();
    assert_eq!(rig.take_posted(), expected);
    assert_eq!(rig.doc(), "Hi \u{fffc}| there");
    rig.type_("⌫");
    let undo = [
        vec![Out::Right; 6],
        backspaces(10),
        vec![Out::Type(";img".into())],
    ]
    .concat();
    assert_eq!(rig.posted(), undo);
    assert_eq!(rig.doc(), ";img|");
}

#[test]
fn a_text_alternative_paste_is_confirmed_as_text() {
    // The app pastes the plain flavour, "[Logo]" included.
    let rig = pasted_into(|_| {});
    let expected = [backspaces(3), vec![Out::Paste], lefts(6)].concat();
    assert_eq!(rig.take_posted(), expected);
    assert_eq!(rig.doc(), "Hi [Logo]| there");
    rig.type_("⌫");
    assert_eq!(rig.doc(), ";img|");
}

#[test]
fn an_unconfirmed_paste_skips_the_cursor_move_and_the_undo() {
    // The app reports no caret, so nothing is confirmed.
    let rig = pasted_into(|doc| doc.reports_caret = false);
    assert_eq!(
        rig.take_posted(),
        [backspaces(3), vec![Out::Paste]].concat()
    );
    assert_eq!(rig.doc(), "Hi [Logo] there|");
    // Backspace is just a Backspace.
    rig.type_("⌫");
    assert_eq!(rig.posted(), []);
    assert_eq!(rig.doc(), "Hi [Logo] ther|");
    assert_eq!(rig.host.expansions(), 1);
}

#[test]
fn a_paste_of_neither_length_is_unconfirmed() {
    let rig = pasted_into(|doc| doc.paste_inserts = Some("Hi Logo there".into()));
    assert_eq!(
        rig.take_posted(),
        [backspaces(3), vec![Out::Paste]].concat()
    );
    assert_eq!(rig.doc(), "Hi Logo there|");
    rig.type_("⌫");
    assert_eq!(rig.posted(), []);
    assert_eq!(rig.doc(), "Hi Logo ther|");
}
