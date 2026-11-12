//! S51 trigger chips (EDT-4): commits, the atomic paste, Backspace and Escape, the validator's
//! sentences with the shake and the duplicate pulse (both expire), the pop-in, amber conflicts
//! and their banners, and IME.

use std::time::Duration;

use rune::{Ime, Key};

use super::editor::{compose, draft, has_text, key, open, rig_with, snippet, type_str};
use super::{esc, Rig};
use crate::main::editor::{self, triggers};
use crate::main::state::MainPane;
use crate::theme::Theme;

fn chips(rig: &Rig) -> Vec<String> {
    draft(rig).triggers
}

fn focus_triggers(rig: &mut Rig) {
    rig.click_key(editor::TRIGGER_KEY);
    assert_eq!(rig.focused_key().as_deref(), Some(editor::TRIGGER_KEY));
}

fn one() -> Rig {
    rig_with(vec![snippet(1, "Sig", &[";a"], "x")], vec![])
}

fn error(rig: &Rig) -> Option<String> {
    rig.rt.state.editor.trigger.error.clone()
}

#[test]
fn space_return_tab_and_blur_commit_a_trigger() {
    let mut rig = one();
    focus_triggers(&mut rig);
    type_str(&mut rig, ";b ");
    assert_eq!(chips(&rig), [";a", ";b"]);
    assert_eq!(rig.rt.state.editor.trigger.edit.text, "");
    type_str(&mut rig, ";c");
    rig.press(key(Key::Enter));
    assert_eq!(chips(&rig), [";a", ";b", ";c"]);
    type_str(&mut rig, ";d");
    rig.press(key(Key::Tab));
    assert_eq!(chips(&rig), [";a", ";b", ";c", ";d"]);
    assert_eq!(
        rig.focused_key().as_deref(),
        Some(editor::TRIGGER_KEY),
        "Tab with text stays"
    );
    type_str(&mut rig, ";e");
    rig.click_key(editor::NAME_KEY);
    assert_eq!(chips(&rig), [";a", ";b", ";c", ";d", ";e"], "blur commits");
    assert!(rig.rt.state.editor.is_dirty());
}

#[test]
fn a_paste_with_spaces_adds_every_trigger_or_none() {
    let mut rig = one();
    focus_triggers(&mut rig);
    rig.rt.paste(";x ;y\n;z");
    rig.settle();
    assert_eq!(chips(&rig), [";a", ";x", ";y", ";z"]);
    rig.rt.paste(";q ;A");
    rig.settle();
    assert_eq!(
        chips(&rig),
        [";a", ";x", ";y", ";z"],
        "one refused part refuses all"
    );
    assert_eq!(
        error(&rig).as_deref(),
        Some("This snippet already has that trigger.")
    );
    assert!(has_text(&rig, "This snippet already has that trigger."));
    assert!(rig.rt.state.editor.trigger.shake.running());
    let pulse = rig
        .rt
        .state
        .editor
        .trigger
        .pulse
        .as_ref()
        .map(|(k, _)| k.clone());
    assert_eq!(pulse.as_deref(), Some(";a"), "the chip it repeats lands");
}

#[test]
fn the_shake_and_the_pulse_expire() {
    let mut rig = one();
    focus_triggers(&mut rig);
    type_str(&mut rig, ";a");
    rig.press(key(Key::Enter));
    assert!(rig.rt.state.editor.trigger.shake.running());
    assert!(rig.rt.state.editor.trigger.pulse.is_some());
    for _ in 0..4 {
        rig.rt.frame(0.1);
    }
    rig.rt.frame(0.0);
    assert!(
        !rig.rt.state.editor.trigger.shake.running(),
        "the shake ends after 240 ms"
    );
    assert!(
        rig.rt.state.editor.trigger.pulse.is_none(),
        "the pulse ends"
    );
    assert!(!rig.rt.state.editor.trigger.motion_running());
}

#[test]
fn a_new_chip_pops_in_once_and_opened_chips_stay_still() {
    let mut rig = one();
    assert!(
        rig.rt.state.editor.trigger.arrive.is_none(),
        "opening stays still"
    );
    focus_triggers(&mut rig);
    type_str(&mut rig, ";n ");
    let arriving = rig
        .rt
        .state
        .editor
        .trigger
        .arrive
        .as_ref()
        .map(|(k, _)| k.clone());
    assert_eq!(arriving, Some(vec![";n".to_owned()]));
    for _ in 0..4 {
        rig.rt.frame(0.1);
    }
    assert!(rig.rt.state.editor.trigger.arrive.is_none(), "the pop ends");
}

#[test]
fn the_validators_sentences_show_and_clear_on_the_next_change() {
    let mut rig = one();
    focus_triggers(&mut rig);
    rig.rt.paste("   ");
    rig.settle();
    assert_eq!(
        error(&rig).as_deref(),
        Some("A trigger can't be only spaces.")
    );
    assert!(has_text(&rig, "A trigger can't be only spaces."));
    type_str(&mut rig, "x");
    assert_eq!(error(&rig), None, "typing clears it");
    rig.press(key(Key::Backspace));
    let long = "y".repeat(65);
    rig.rt.paste(long.clone());
    rig.settle();
    rig.press(key(Key::Enter));
    assert_eq!(
        error(&rig).as_deref(),
        Some("Triggers can be at most 64 characters; this one has 65.")
    );
    assert!(has_text(
        &rig,
        "Triggers can be at most 64 characters; this one has 65."
    ));
    assert_eq!(chips(&rig), [";a"]);
    assert_eq!(
        rig.rt.state.editor.trigger.edit.text, long,
        "a refused trigger stays to fix"
    );
}

#[test]
fn backspace_on_an_empty_field_edits_the_last_trigger() {
    let mut rig = rig_with(vec![snippet(1, "Sig", &[";a", ";b"], "x")], vec![]);
    focus_triggers(&mut rig);
    rig.press(key(Key::Backspace));
    assert_eq!(chips(&rig), [";a"]);
    assert_eq!(rig.rt.state.editor.trigger.edit.text, ";b");
}

#[test]
fn escape_clears_what_was_typed_then_returns_to_the_list() {
    let mut rig = one();
    focus_triggers(&mut rig);
    type_str(&mut rig, ";zz");
    rig.press(esc());
    assert_eq!(rig.rt.state.editor.trigger.edit.text, "");
    assert_eq!(rig.focused_key().as_deref(), Some(editor::TRIGGER_KEY));
    rig.press(esc());
    assert_eq!(rig.rt.state.frame.focus.active(), MainPane::List);
    assert_eq!(chips(&rig), [";a"]);
}

#[test]
fn a_lone_prefix_is_dropped_on_blur() {
    let mut rig = rig_with(vec![snippet(1, "Bare", &[], "")], vec![]);
    focus_triggers(&mut rig);
    assert_eq!(rig.rt.state.editor.trigger.edit.text, ";");
    rig.click_key(editor::NAME_KEY);
    assert_eq!(rig.rt.state.editor.trigger.edit.text, "");
    assert!(chips(&rig).is_empty());
    assert!(!rig.rt.state.editor.is_dirty());
}

#[test]
fn the_remove_button_drops_its_chip() {
    let mut rig = rig_with(vec![snippet(1, "Sig", &[";a", ";b"], "x")], vec![]);
    rig.click_key("editor-trigger-remove-;a");
    assert_eq!(chips(&rig), [";b"]);
    let labels: Vec<String> = rig
        .rt
        .elements()
        .into_iter()
        .filter_map(|e| e.label)
        .collect();
    assert!(labels.iter().any(|l| l == "Remove trigger ;b"));
    assert!(!labels.iter().any(|l| l == "Remove trigger ;a"));
}

#[test]
fn trigger_keys_do_nothing_while_composing() {
    let mut rig = rig_with(vec![snippet(1, "Sig", &[";a", ";b"], "x")], vec![]);
    focus_triggers(&mut rig);
    type_str(&mut rig, ";c");
    compose(&mut rig);
    rig.press(key(Key::Enter));
    rig.press(key(Key::Tab));
    rig.press(esc());
    assert_eq!(chips(&rig), [";a", ";b"]);
    assert_eq!(rig.focused_key().as_deref(), Some(editor::TRIGGER_KEY));
    rig.rt.ime(Ime::Commit("".into()));
    rig.settle();
    rig.press(key(Key::Enter));
    assert_eq!(
        chips(&rig),
        [";a", ";b", ";c"],
        "after composition the key works"
    );
}

fn wait_conflicts(rig: &mut Rig) {
    for _ in 0..200 {
        let revision = rig.kudzu.library.watch().revision();
        if rig.kudzu.library.conflicts().get().revision == revision {
            rig.settle();
            return;
        }
        rig.fixtures.clock.advance(Duration::from_millis(100));
        std::thread::sleep(Duration::from_millis(10));
        rig.settle();
    }
    panic!("conflicts were never checked");
}

#[test]
fn colliding_triggers_turn_amber_and_name_the_other_snippet() {
    let mut rig = rig_with(
        vec![
            snippet(1, "Mine", &[";dup", ";s"], "x"),
            snippet(2, "Theirs", &[";dup"], "y"),
            snippet(3, "", &[";sig"], "z"),
        ],
        vec![],
    );
    wait_conflicts(&mut rig);
    open(&mut rig, 1);
    let theme = Theme::dark();
    let chip = rig
        .rt
        .elements()
        .into_iter()
        .find(|e| e.key.as_deref() == Some("editor-trigger-chip-;dup"))
        .expect("the ;dup chip");
    assert_eq!(chip.fill, theme.c.warning.alpha(0.15));
    assert_eq!(chip.help.as_deref(), Some("Conflicts with another snippet"));
    assert!(has_text(&rig, "“;dup” is also used by"));
    assert!(has_text(&rig, "Theirs"));
    assert!(has_text(
        &rig,
        "“;s” fires before “;sig” can be finished in"
    ));
    assert!(has_text(&rig, "another snippet"), "an unnamed snippet");
    let fine = rig
        .rt
        .elements()
        .into_iter()
        .find(|e| e.key.as_deref() == Some("editor-trigger-chip-;s"))
        .expect("the ;s chip");
    assert_eq!(
        fine.fill,
        theme.c.warning.alpha(0.15),
        "shadowing is a conflict too"
    );

    open(&mut rig, 3);
    assert!(has_text(
        &rig,
        "“;sig” can't be typed in full — “;s” fires first in"
    ));
    open(&mut rig, 1);
    let theirs = super::editor::id(2);
    let other = rig
        .rt
        .elements()
        .into_iter()
        .find(|e| e.text.as_deref() == Some("Theirs") && e.role == "button")
        .and_then(|e| e.key)
        .expect("the other snippet's link");
    rig.click_key(&other);
    assert_eq!(rig.rt.state.frame.selected_snippet, Some(theirs));
    assert_eq!(rig.rt.state.editor.open_id(), Some(theirs));
}

#[test]
fn opening_a_snippet_resets_the_field() {
    let mut rig = rig_with(
        vec![snippet(1, "A", &[";a"], "x"), snippet(2, "B", &[";b"], "y")],
        vec![],
    );
    focus_triggers(&mut rig);
    type_str(&mut rig, ";a");
    rig.press(key(Key::Enter));
    assert!(error(&rig).is_some());
    open(&mut rig, 2);
    assert_eq!(error(&rig), None);
    assert!(!rig.rt.state.editor.trigger.motion_running());
    assert_eq!(triggers::chip_keys(&chips(&rig)), [";b"]);
}
