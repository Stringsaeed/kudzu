//! The trigger chips (EDT-4, `trigger-input.tsx`): Space, Return, Tab and blur commit what was
//! typed; a paste with whitespace splits into triggers in one step (all or none); Backspace on an
//! empty field edits the last trigger; Escape clears what was typed. A rejected trigger shows the
//! core validator's sentence and shakes the field; a duplicate also points at the chip it repeats.
//! Chips that collide with another snippet are amber. The first focus suggests the prefix.
//!
//! One addition: a paste of only spaces says "A trigger can't be only spaces." (Tauri drops it
//! silently), so the validator's whitespace sentence is reachable.

use std::collections::{HashMap, HashSet};

use kudzu_core::library::triggers::{trigger_tokens, validate_trigger, TriggerIssue};
use kudzu_core::CaseMode;
use rune::{
    col, icon, row, text, text_input, ClipboardContent, Cx, El, Key, KeyEvent, TextEdit, Tx,
};

use crate::motion::{Land, Shake};
use crate::shell::MainState;
use crate::theme::{icons, motion, radius, Theme};

/// Error text under the field.
pub const ERROR_KEY: &str = "editor-trigger-error";
const BOX_KEY: &str = "editor-trigger-box";

/// The text being typed and its feedback.
#[derive(Clone, Debug, Default)]
pub struct Field {
    pub edit: TextEdit,
    pub error: Option<String>,
    pub shake: Shake,
    /// The chip a rejected duplicate points at, landing from 112%.
    pub pulse: Option<(String, Land)>,
    /// Chips added since the editor opened, popping in.
    pub arrive: Option<(Vec<String>, Land)>,
    /// Chips the editor opened with stay still. A removed chip leaves, so adding it back pops.
    opened: HashSet<String>,
    was_focused: bool,
}

impl Field {
    /// A snippet opened with `triggers`.
    pub fn opened(&mut self, triggers: &[String]) {
        *self = Field {
            opened: chip_keys(triggers).into_iter().collect(),
            ..Field::default()
        };
    }

    /// Ends the shake, pulse and pop once their time is up. True when the view must rebuild.
    pub fn poll(&mut self, clock: f64, tx: &mut Tx) -> bool {
        let mut changed = self.shake.poll(clock, tx);
        if let Some((_, land)) = &mut self.pulse {
            changed |= land.poll(clock, tx);
            if !land.running() {
                self.pulse = None;
                changed = true;
            }
        }
        if let Some((_, land)) = &mut self.arrive {
            changed |= land.poll(clock, tx);
            if !land.running() {
                self.arrive = None;
                changed = true;
            }
        }
        changed
    }

    pub fn motion_running(&self) -> bool {
        self.shake.running() || self.pulse.is_some() || self.arrive.is_some()
    }
}

/// What committing typed text does.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Commit {
    /// Nothing but whitespace was typed.
    Nothing,
    /// The triggers after adding every part.
    Added(Vec<String>),
    /// `part` was refused, so nothing was added.
    Rejected { issue: TriggerIssue, part: String },
}

/// `commit` in `trigger-input.tsx`: each whitespace-separated part is checked against the
/// triggers so far, so a paste that repeats itself is refused too.
pub fn commit(raw: &str, triggers: &[String], case_sensitive: bool) -> Commit {
    if raw.trim().is_empty() {
        return Commit::Nothing;
    }
    let mut next = triggers.to_vec();
    for part in trigger_tokens(raw) {
        match validate_trigger(&part, &next, case_sensitive) {
            Ok(clean) => next.push(clean),
            Err(issue) => return Commit::Rejected { issue, part },
        }
    }
    Commit::Added(next)
}

/// Stable chip keys: the trigger, plus a counter only when a library repeats one.
pub fn chip_keys(triggers: &[String]) -> Vec<String> {
    let mut seen: HashMap<&str, usize> = HashMap::new();
    triggers
        .iter()
        .map(|t| {
            let n = seen.entry(t.as_str()).or_insert(0);
            *n += 1;
            if *n == 1 {
                t.clone()
            } else {
                format!("{t} {}", *n - 1)
            }
        })
        .collect()
}

/// The chip a duplicate `part` repeats, by the same case rule the validator used.
fn duplicate_of(part: &str, triggers: &[String], case_sensitive: bool) -> Option<usize> {
    let t = part.trim();
    triggers.iter().position(|e| {
        if case_sensitive {
            e == t
        } else {
            e.to_lowercase() == t.to_lowercase()
        }
    })
}

fn prefix(s: &MainState) -> String {
    s.svc.settings.watch().get().default_trigger_prefix.clone()
}

fn triggers_of(s: &MainState) -> (Vec<String>, bool) {
    s.editor
        .draft()
        .map(|d| (d.triggers.clone(), d.case_mode == CaseMode::Exact))
        .unwrap_or_default()
}

/// Commits `raw`. With `tx` a refusal shakes (unless motion is reduced); without, it is quiet.
fn apply(s: &mut MainState, raw: &str, motion: Option<(&mut Tx, bool)>) -> bool {
    let (triggers, case_sensitive) = triggers_of(s);
    match commit(raw, &triggers, case_sensitive) {
        Commit::Nothing => false,
        Commit::Rejected { issue, part } => {
            s.editor.trigger.error = Some(issue.message());
            if let Some((tx, false)) = motion {
                s.editor.trigger.shake.start(tx);
                if let Some(i) = duplicate_of(&part, &triggers, case_sensitive) {
                    let mut land = Land::new(1.12, false);
                    land.start(tx);
                    s.editor.trigger.pulse = Some((chip_keys(&triggers)[i].clone(), land));
                }
            }
            false
        }
        Commit::Added(next) => {
            let keys = chip_keys(&next);
            let fresh: Vec<String> = keys
                .iter()
                .filter(|k| !s.editor.trigger.opened.contains(*k))
                .cloned()
                .collect();
            if let (Some((tx, false)), false) = (motion, fresh.is_empty()) {
                let mut land = Land::new(0.94, true);
                land.start(tx);
                s.editor.trigger.arrive = Some((fresh, land));
            }
            s.editor.trigger.edit.replace_text("");
            s.editor.trigger.error = None;
            super::edit_draft(s, |d| d.triggers = next);
            true
        }
    }
}

/// Flush: what was typed commits without motion. The prefix alone is dropped.
pub(crate) fn commit_quietly(s: &mut MainState) -> bool {
    let raw = s.editor.trigger.edit.text.clone();
    if raw.is_empty() {
        return false;
    }
    if raw == prefix(s) {
        s.editor.trigger.edit.replace_text("");
        return true;
    }
    apply(s, &raw, None)
}

/// The remove button on chip `index`: the chips after it slide into its place.
pub fn remove(s: &mut MainState, index: usize) {
    let (triggers, _) = triggers_of(s);
    let Some(key) = chip_keys(&triggers).get(index).cloned() else {
        return;
    };
    s.editor.trigger.opened.remove(&key);
    s.editor.trigger.error = None;
    super::edit_draft(s, |d| {
        d.triggers.remove(index);
    });
}

/// Focus in suggests the prefix; focus out drops a lone prefix or commits what was typed.
pub(crate) fn sync_focus(s: &mut MainState, tx: &mut Tx, reduced: bool) -> bool {
    let focused = s.editor.trigger.edit.focused;
    if focused == s.editor.trigger.was_focused {
        return false;
    }
    s.editor.trigger.was_focused = focused;
    let prefix = prefix(s);
    let typed = s.editor.trigger.edit.text.clone();
    if focused {
        let (triggers, _) = triggers_of(s);
        if typed.is_empty() && triggers.is_empty() && !prefix.is_empty() {
            s.editor.trigger.edit.replace_text(&prefix);
            s.editor.trigger.edit.focused = true;
            return true;
        }
        return false;
    }
    if typed == prefix {
        s.editor.trigger.edit.replace_text("");
        return true;
    }
    if typed.trim().is_empty() {
        return false;
    }
    apply(s, &typed, Some((tx, reduced)));
    true
}

/// `onChange`: a trailing space commits; leading spaces never stay.
fn changed(s: &mut MainState, tx: &mut Tx, reduced: bool) {
    if s.editor.trigger.edit.is_composing() {
        return;
    }
    s.editor.trigger.error = None;
    let typed = s.editor.trigger.edit.text.clone();
    if typed.ends_with(char::is_whitespace) && !typed.trim().is_empty() {
        if !apply(s, &typed, Some((tx, reduced))) {
            // The field is controlled in Tauri: a refused space never shows.
            s.editor.trigger.edit.replace_text(typed.trim_end());
        }
    } else if typed.starts_with(char::is_whitespace) {
        s.editor.trigger.edit.replace_text(typed.trim_start());
    }
}

fn bare(ev: &KeyEvent) -> bool {
    !ev.mods.cmd && !ev.mods.ctrl && !ev.mods.alt
}

fn key(s: &mut MainState, ev: &KeyEvent, tx: &mut Tx, reduced: bool) -> bool {
    if s.editor.trigger.edit.is_composing() {
        // Rune keeps Return and Escape for the input method; Tab would move focus and the
        // blur would commit the half-composed trigger, so it stays here and does nothing.
        return ev.key == Key::Tab;
    }
    if !bare(ev) {
        return super::field_key(s, ev, tx);
    }
    let typed = s.editor.trigger.edit.text.clone();
    let worth = !typed.trim().is_empty() && typed != prefix(s);
    match ev.key {
        Key::Enter => {
            if worth {
                apply(s, &typed, Some((tx, reduced)));
            }
            true
        }
        Key::Tab if worth => {
            apply(s, &typed, Some((tx, reduced)));
            true
        }
        Key::Backspace if typed.is_empty() => {
            let (triggers, _) = triggers_of(s);
            let Some(last) = triggers.last().cloned() else {
                return false;
            };
            let keys = chip_keys(&triggers);
            if let Some(k) = keys.last() {
                s.editor.trigger.opened.remove(k);
            }
            super::edit_draft(s, |d| {
                d.triggers.pop();
            });
            s.editor.trigger.edit.replace_text(&last);
            true
        }
        Key::Escape if !ev.mods.shift && !typed.is_empty() => {
            s.editor.trigger.edit.replace_text("");
            s.editor.trigger.error = None;
            true
        }
        _ => super::field_key(s, ev, tx),
    }
}

fn paste(s: &mut MainState, content: &ClipboardContent, tx: &mut Tx, reduced: bool) -> bool {
    if s.editor.trigger.edit.is_composing() {
        return false;
    }
    let pasted = content.text();
    if !pasted.is_empty() && pasted.trim().is_empty() {
        s.editor.trigger.error = Some(TriggerIssue::WhitespaceOnly.message());
        if !reduced {
            s.editor.trigger.shake.start(tx);
        }
        return true;
    }
    if !pasted.trim().contains(char::is_whitespace) {
        return false;
    }
    let combined = format!("{}{pasted}", s.editor.trigger.edit.text);
    apply(s, &combined, Some((tx, reduced)));
    true
}

/// The label, the chip well and the error (`snippet-editor.tsx:350-366`).
pub fn view(s: &MainState, cx: &Cx<MainState>, theme: &Theme) -> El {
    let c = &theme.c;
    let reduced = cx.is_reduced_motion();
    let Some(draft) = s.editor.draft() else {
        return col();
    };
    let field = &s.editor.trigger;
    let focused = field.edit.focused;
    let label = row()
        .gap(4.0)
        .child(
            text("Triggers")
                .font(theme.ty.control_xs)
                .color(c.muted_foreground),
        )
        .child(
            text("— type any of these to expand")
                .font(rune::TextStyle::new(12.0, 400.0).lh(16.0))
                .color(c.faint),
        );

    let keys = chip_keys(&draft.triggers);
    let conflicted = super::view::conflicted(s);
    let mut well = row()
        .key(BOX_KEY)
        .w_full()
        .min_h(36.0)
        .wrap()
        .px(6.0)
        .py(4.0)
        .gap(6.0)
        .items_center()
        .radius(radius::LG)
        .on_press(|st: &mut MainState| st.editor.trigger.edit.focused = true);
    if focused {
        well = well.outline(
            2.0,
            0.0,
            if field.error.is_some() {
                c.destructive.alpha(0.6)
            } else {
                c.ring
            },
        );
    } else {
        well = well.fill(if theme.dark {
            rune::Color::BLACK.alpha(0.15)
        } else {
            c.foreground.alpha(0.025)
        });
        if field.error.is_some() {
            well = well.outline(1.0, 0.0, c.destructive.alpha(0.6));
        }
        for shadow in &theme.depth.well {
            well = well.box_shadow(*shadow);
        }
    }
    for (i, trigger) in draft.triggers.iter().enumerate() {
        well = well.child(chip(
            s,
            theme,
            trigger,
            &keys[i],
            i,
            conflicted.contains(&trigger.to_lowercase()),
            reduced,
        ));
    }
    let placeholder = if draft.triggers.is_empty() {
        format!("{}trigger", prefix(s))
    } else {
        "Add another…".to_owned()
    };
    let input = text_input(
        &field.edit,
        rune::TextStyle {
            size: 12.5,
            ..theme.ty.mono
        }
        .lh(24.0),
    )
    .single_line()
    .key(super::TRIGGER_KEY)
    .label("Add trigger")
    .placeholder(placeholder, c.faint)
    .h(24.0)
    .min_w(96.0)
    .grow(1.0)
    .px(4.0)
    .color(c.foreground)
    .caret(c.iris, theme.kit().selection())
    .invalid(field.error.is_some())
    .on_edit(|st: &mut MainState| &mut st.editor.trigger.edit)
    .on_input_change(move |st: &mut MainState, tx: &mut Tx| changed(st, tx, reduced))
    .on_input_key(move |st: &mut MainState, ev: &KeyEvent, tx: &mut Tx| key(st, ev, tx, reduced))
    .on_input_paste(
        move |st: &mut MainState, content: &ClipboardContent, tx: &mut Tx| {
            paste(st, content, tx, reduced)
        },
    );
    well = well.child(input);
    let well = if reduced {
        well
    } else {
        field.shake.apply(well)
    };

    let mut out = col().w_full().gap(6.0).child(label).child(well);
    if let Some(error) = &field.error {
        out = out.child(
            text(error.clone())
                .key(ERROR_KEY)
                .role("alert")
                .font(rune::TextStyle::new(12.0, 400.0).lh(16.0))
                .color(c.destructive)
                .transition(crate::motion::rise()),
        );
    }
    out
}

fn chip(
    s: &MainState,
    theme: &Theme,
    trigger: &str,
    key: &str,
    index: usize,
    conflicted: bool,
    reduced: bool,
) -> El {
    let c = &theme.c;
    let (bg, fg) = if conflicted {
        (c.warning.alpha(0.15), c.warning)
    } else {
        (c.iris_soft, c.iris_foreground)
    };
    let remove_button = row()
        .key(format!("editor-trigger-remove-{key}"))
        .role("button")
        .label(format!("Remove trigger {trigger}"))
        .focusable(true)
        .w(16.0)
        .h(16.0)
        .radius(4.0)
        .center()
        .hover(c.foreground.alpha(0.1))
        .child(icon(&icons::X, 12.0).color(fg.alpha(0.5)).a11y_hidden())
        .on_press(move |st: &mut MainState| remove(st, index));
    let mut chip = row()
        .key(format!("editor-trigger-chip-{key}"))
        .h(24.0)
        .pl(8.0)
        .pr(4.0)
        .gap(4.0)
        .radius(radius::MD)
        .fill(bg)
        .items_center()
        .motion(motion::soft_spring(motion::SLOW))
        .child(
            text(trigger.to_owned())
                .font(
                    rune::TextStyle {
                        size: 12.0,
                        ..theme.ty.mono
                    }
                    .lh(16.0),
                )
                .color(fg),
        )
        .child(remove_button);
    if conflicted {
        chip = chip.help("Conflicts with another snippet");
    }
    if reduced {
        return chip;
    }
    let field = &s.editor.trigger;
    if let Some((keys, land)) = &field.arrive {
        if keys.iter().any(|k| k == key) {
            chip = land.apply(chip);
        }
    }
    if let Some((k, land)) = &field.pulse {
        if k == key {
            chip = land.apply(chip);
        }
    }
    chip
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn a_paste_adds_every_part_or_none() {
        assert_eq!(
            commit(";a ;b\n;c", &v(&[";x"]), false),
            Commit::Added(v(&[";x", ";a", ";b", ";c"]))
        );
        assert_eq!(
            commit(";a ;X", &v(&[";x"]), false),
            Commit::Rejected {
                issue: TriggerIssue::Duplicate,
                part: ";X".into()
            }
        );
        assert_eq!(
            commit(";a ;a", &[], true),
            Commit::Rejected {
                issue: TriggerIssue::Duplicate,
                part: ";a".into()
            },
            "a paste that repeats itself"
        );
        assert_eq!(commit("   ", &[], false), Commit::Nothing);
    }

    #[test]
    fn exact_case_lets_a_differently_cased_trigger_in() {
        assert_eq!(
            commit(";X", &v(&[";x"]), true),
            Commit::Added(v(&[";x", ";X"]))
        );
    }

    #[test]
    fn the_validators_sentences_are_tauris() {
        let long = "x".repeat(65);
        let Commit::Rejected { issue, .. } = commit(&long, &[], false) else {
            panic!("65 graphemes are refused");
        };
        assert_eq!(
            issue.message(),
            "Triggers can be at most 64 characters; this one has 65."
        );
        assert_eq!(
            TriggerIssue::Duplicate.message(),
            "This snippet already has that trigger."
        );
        assert_eq!(
            TriggerIssue::WhitespaceOnly.message(),
            "A trigger can't be only spaces."
        );
        let flags = "🇺🇸".repeat(64);
        assert!(
            matches!(commit(&flags, &[], false), Commit::Added(_)),
            "graphemes, not bytes"
        );
    }

    #[test]
    fn repeated_triggers_get_distinct_chip_keys() {
        assert_eq!(chip_keys(&v(&[";a", ";b", ";a"])), v(&[";a", ";b", ";a 1"]));
    }

    #[test]
    fn a_duplicate_points_at_the_chip_it_repeats() {
        assert_eq!(duplicate_of(" ;SIG", &v(&[";x", ";sig"]), false), Some(1));
        assert_eq!(duplicate_of(";SIG", &v(&[";sig"]), true), None);
    }
}
