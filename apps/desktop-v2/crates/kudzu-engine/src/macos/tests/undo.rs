//! Undo by Backspace (EXP-12) and reloads (EXP-19). Backspace right after an expansion puts
//! the trigger back exactly as typed, after moving back over any cursor placement; anything
//! else typed first disarms it. A reload that changes nothing the engine matches on (the
//! library saved again after a usage update) keeps an armed undo and a partly typed trigger;
//! a real change drops the undo.

use kudzu_core::activity::ActivityOutcome;

use super::desk::Out;
use super::rig::{backspaces, lefts, snippet, whole_word, Rig};

fn cursor_snippet() -> Vec<kudzu_core::SnippetRecord> {
    vec![snippet(";sig", "Hi {{cursor}}there")]
}

/// Expands `;sig` into "Hi |there" and returns the rig with an empty log.
fn expanded() -> Rig {
    let rig = Rig::new(&cursor_snippet());
    rig.type_(";sig");
    let expected = [backspaces(3), vec![Out::Type("Hi there".into())], lefts(5)].concat();
    assert_eq!(rig.take_posted(), expected);
    assert_eq!(rig.doc(), "Hi |there");
    rig
}

fn undo() -> Vec<Out> {
    [
        vec![Out::Right; 5],
        backspaces(8),
        vec![Out::Type(";sig".into())],
    ]
    .concat()
}

#[test]
fn backspace_undoes_after_moving_back_over_the_cursor() {
    let rig = expanded();
    rig.type_("⌫");
    assert_eq!(rig.posted(), undo());
    assert_eq!(rig.doc(), ";sig|");
    assert_eq!(rig.host.outcomes().last(), Some(&ActivityOutcome::Undone));
}

#[test]
fn a_whole_word_undo_restores_the_separator() {
    let rig = Rig::new(&[whole_word("sig", "Best")]);
    rig.type_("sig ");
    // The swallowed space is typed again after the expansion.
    assert_eq!(
        rig.take_posted(),
        [
            backspaces(3),
            vec![Out::Type("Best".into()), Out::Type(" ".into())]
        ]
        .concat()
    );
    assert_eq!(rig.doc(), "Best |");
    rig.type_("⌫");
    assert_eq!(
        rig.posted(),
        [backspaces(5), vec![Out::Type("sig ".into())]].concat()
    );
    assert_eq!(rig.doc(), "sig |");
}

#[test]
fn typing_anything_first_disarms_the_undo() {
    let rig = expanded();
    rig.type_("x⌫");
    assert_eq!(rig.posted(), []);
    assert_eq!(rig.doc(), "Hi |there");
}

#[test]
fn a_usage_only_reload_keeps_an_armed_undo() {
    let rig = expanded();
    rig.reload(&rig.loaded());
    rig.type_("⌫");
    assert_eq!(rig.posted(), undo());
    assert_eq!(rig.doc(), ";sig|");
}

#[test]
fn a_usage_only_reload_keeps_a_partly_typed_trigger() {
    let rig = Rig::new(&cursor_snippet());
    rig.type_(";si");
    rig.reload(&rig.loaded());
    rig.type_("g");
    assert_eq!(rig.doc(), "Hi |there");
    assert_eq!(rig.host.expansions(), 1);
}

#[test]
fn a_matching_change_drops_the_undo() {
    let rig = expanded();
    let mut changed = rig.loaded();
    changed[0].content = "Hello {{cursor}}there".into();
    rig.reload(&changed);
    rig.type_("⌫");
    assert_eq!(rig.posted(), []);
    assert_eq!(rig.doc(), "Hi|there");
}
