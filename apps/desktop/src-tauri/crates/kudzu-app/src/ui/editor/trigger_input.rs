//! `trigger-input.tsx`: a snippet's triggers as removable chips with a field after them. Space
//! or Return adds what's typed (several at once when pasted), Backspace on an empty field takes
//! the last chip back to edit, Escape clears the field. A focused empty field starts with the
//! default trigger prefix; leaving it commits what's typed.

use std::collections::HashSet;

use rune::{el, icon, row, text, text_input, Color, Cx, El, Family, Key, KeyEvent, TextEdit, TextStyle, Tx};

use super::template::doc::grapheme_stops;
use super::{ensure, update, EditorState};
use crate::ui::icons;
use crate::ui::main::MainUi;
use crate::ui::theme::{radius, text as ty, weight, Theme, Utilities};
use crate::ui::theme_for;

pub const MAX_TRIGGER_LENGTH: usize = 64;

#[derive(Default, Clone)]
pub struct TriggerState {
    pub input: TextEdit,
    pub error: Option<String>,
    /// The focus state the view last saw, to notice focus and blur.
    seen_focus: bool,
}

fn graphemes(s: &str) -> usize {
    grapheme_stops(&s.encode_utf16().collect::<Vec<_>>()).len() - 1
}

/// `validateTrigger`: the cleaned trigger, or why it can't be added.
pub fn validate_trigger(raw: &str, existing: &[String], case_sensitive: bool) -> Result<String, String> {
    if raw.is_empty() {
        return Err("Enter a trigger first.".into());
    }
    let t = kudzu_core::client::js::trim(raw);
    if t.is_empty() {
        return Err("A trigger can't be only spaces.".into());
    }
    let len = graphemes(t);
    if len > MAX_TRIGGER_LENGTH {
        return Err(format!("Triggers can be at most {MAX_TRIGGER_LENGTH} characters; this one has {len}."));
    }
    let dup = existing.iter().any(|e| if case_sensitive { e == t } else { e.to_lowercase() == t.to_lowercase() });
    if dup {
        return Err("This snippet already has that trigger.".into());
    }
    Ok(t.to_string())
}

/// `commit`: adds every whitespace-separated trigger in `raw`, or shows the first problem.
fn commit(s: &mut MainUi, raw: &str) -> bool {
    ensure(s);
    if kudzu_core::client::js::trim(raw).is_empty() {
        return false;
    }
    let Some(d) = s.editor.draft.as_ref() else { return false };
    let case_sensitive = d.case_mode == kudzu_core::CaseMode::Exact;
    let mut next = d.triggers.clone();
    for p in kudzu_core::client::js::split_spaces(raw) {
        match validate_trigger(p, &next, case_sensitive) {
            Ok(t) => next.push(t),
            Err(e) => {
                s.editor.triggers.error = Some(e);
                return false;
            }
        }
    }
    update(s, move |d| d.triggers = next);
    s.editor.triggers.input.replace_text(String::new());
    s.editor.triggers.error = None;
    true
}

fn prefix(s: &MainUi) -> String {
    s.settings.default_trigger_prefix.clone()
}

/// Focus came or went (seen the frame after): prefill the prefix, or commit on the way out.
fn focus_changed(s: &mut MainUi) {
    ensure(s);
    let focused = s.editor.triggers.input.focused;
    if focused == s.editor.triggers.seen_focus {
        return;
    }
    s.editor.triggers.seen_focus = focused;
    let p = prefix(s);
    let draft = s.editor.triggers.input.text.clone();
    let none = s.editor.draft.as_ref().is_none_or(|d| d.triggers.is_empty());
    if focused {
        if draft.is_empty() && none && !p.is_empty() {
            let len = p.len();
            s.editor.triggers.input.replace_text(p);
            s.editor.triggers.input.set_caret(len);
        }
    } else if draft == p {
        s.editor.triggers.input.replace_text(String::new());
    } else if !kudzu_core::client::js::trim(&draft).is_empty() {
        commit(s, &draft);
    }
}

fn on_change(s: &mut MainUi, _tx: &mut Tx) {
    s.editor.triggers.error = None;
    let v = s.editor.triggers.input.text.clone();
    let ends_space = v.chars().last().is_some_and(kudzu_core::client::js::is_space);
    if ends_space && !kudzu_core::client::js::trim(&v).is_empty() {
        commit(s, &v);
    } else {
        let trimmed = v.trim_start_matches(kudzu_core::client::js::is_space);
        if trimmed.len() != v.len() {
            let t = trimmed.to_string();
            s.editor.triggers.input.replace_text(t);
        }
    }
}

fn on_key(s: &mut MainUi, ev: &KeyEvent, tx: &mut Tx) -> bool {
    if super::save_key(s, ev) {
        return true;
    }
    let p = prefix(s);
    let draft = s.editor.triggers.input.text.clone();
    let live = !kudzu_core::client::js::trim(&draft).is_empty() && draft != p;
    match &ev.key {
        Key::Enter | Key::Tab if !ev.mods.cmd && !ev.mods.alt => {
            if matches!(ev.key, Key::Tab) && !live {
                return false;
            }
            if live {
                commit(s, &draft);
                return true;
            }
            false
        }
        Key::Backspace if draft.is_empty() && !ev.mods.cmd && !ev.mods.alt => {
            let Some(last) = s.editor.draft.as_ref().and_then(|d| d.triggers.last().cloned()) else { return false };
            update(s, |d| {
                d.triggers.pop();
            });
            let len = last.len();
            s.editor.triggers.input.replace_text(last);
            s.editor.triggers.input.set_caret(len);
            true
        }
        Key::Escape if !draft.is_empty() => {
            s.editor.triggers.input.replace_text(String::new());
            s.editor.triggers.error = None;
            true
        }
        Key::Escape => super::escape_to_list(s, tx),
        _ => false,
    }
}

/// `font-mono text-[12px]`.
const CHIP_FONT: TextStyle = TextStyle::new(12.0, weight::NORMAL).family(Family::Mono).lh(16.0);
/// `font-mono text-[12.5px]`.
const FIELD_FONT: TextStyle = TextStyle::new(12.5, weight::NORMAL).family(Family::Mono).lh(24.0);

pub fn view(s: &MainUi, st: &EditorState, conflicted: &HashSet<String>, cx: &mut Cx<MainUi>) -> El {
    let th = theme_for(cx);
    let t = &th.t;
    let d = st.draft.as_ref().expect("draft");
    let tr = &st.triggers;
    let focused = tr.input.focused;
    let p = prefix(s);
    let mut field = row()
        .key("trigger-box")
        .flex_wrap()
        .min_h(36.0)
        .center_y()
        .gap(6.0)
        .px(6.0)
        .py(4.0)
        .radius(radius::LG)
        .hover_group()
        .on_press(|s: &mut MainUi| {
            ensure(s);
            s.editor.triggers.input.focused = true;
        });
    // `bg-foreground/[0.025] shadow-well dark:bg-black/15`, `focus-within:` transparent with a
    // 2 pt ring, a 1 pt destructive ring on error.
    if focused {
        field = field.fill(Color::TRANSPARENT).outline(2.0, 0.0, if tr.error.is_some() { Theme::tint(t.destructive, 0.6) } else { t.ring });
    } else {
        field = field.fill(if th.dark { Color::BLACK.alpha(0.15) } else { Theme::tint(t.foreground, 0.025) }).elevate(&t.elev_well);
        if tr.error.is_some() {
            field = field.outline(1.0, 0.0, Theme::tint(t.destructive, 0.6));
        }
    }
    for (i, trig) in d.triggers.iter().enumerate() {
        let bad = conflicted.contains(&trig.to_lowercase());
        let (bg, fg) = if bad { (Theme::tint(t.warning, 0.15), t.warning) } else { (t.iris_soft, t.iris_foreground) };
        let label = trig.clone();
        let remove = el()
            .key(format!("trigger-remove-{i}"))
            .size(16.0, 16.0)
            .center()
            .radius(radius::SM * 0.66)
            .hover_group()
            .hover(Theme::tint(t.foreground, 0.1))
            .role("button")
            .label(format!("Remove trigger {label}"))
            .focusable(true)
            .on_press(move |s: &mut MainUi| {
                ensure(s);
                update(s, |d| {
                    if i < d.triggers.len() {
                        d.triggers.remove(i);
                    }
                });
            })
            .child(icon(&icons::X, 12.0).color(fg).group_opacity(0.5, 1.0));
        let mut chip = row()
            .key(format!("trigger-chip-{i}-{trig}"))
            .h(24.0)
            .center_y()
            .gap(4.0)
            .pl(8.0)
            .pr(4.0)
            .radius(radius::MD)
            .fill(bg)
            .transition(rune::Transition::scale(0.94).springs(rune::SpringPair::new(crate::ui::theme::motion::pop(), crate::ui::theme::motion::moderate())))
            .child(text(trig.clone()).font(CHIP_FONT).color(fg))
            .child(remove);
        if bad {
            chip = chip.help("Conflicts with another snippet");
        }
        field = field.child(chip);
    }
    let placeholder = if d.triggers.is_empty() { format!("{p}trigger") } else { "Add another…".into() };
    let input = text_input(&tr.input, FIELD_FONT)
        .single_line()
        .key("trigger-input")
        .w_full()
        .h(24.0)
        .color(t.foreground)
        .caret(t.foreground, t.iris_soft)
        .spellcheck(false)
        .label("Add trigger")
        .invalid(tr.error.is_some())
        .on_edit(|s: &mut MainUi| {
            ensure(s);
            &mut s.editor.triggers.input
        })
        .on_input_change(on_change)
        .on_input_key(on_key)
        .on_input_paste(|s: &mut MainUi, content, _tx| {
            let text = content.text().to_string();
            let trimmed = kudzu_core::client::js::trim(&text);
            if trimmed.chars().any(kudzu_core::client::js::is_space) {
                let draft = s.editor.triggers.input.text.clone();
                commit(s, &format!("{draft}{text}"));
                return true;
            }
            false
        });
    // The placeholder in the sans face (`placeholder:font-sans placeholder:text-faint`).
    let mut slot = el().key("trigger-slot").grow(1.0).min_w(96.0).h(24.0).px(4.0).child(input);
    if tr.input.text.is_empty() && tr.input.preedit.as_ref().is_none_or(|p| p.0.is_empty()) {
        slot = slot.child(text(placeholder).font(ty::XS3).color(t.faint).absolute(4.0, 3.0).inert().a11y_hidden());
    }
    field = field.child(slot);
    if tr.input.text.is_empty() && !d.triggers.is_empty() {
        field = field.child(icon(&icons::PLUS, 14.0).color(t.faint).mr(4.0).group_opacity(0.0, 1.0).a11y_hidden());
    }
    // Notices focus and blur (the key changes with focus, so its first layout runs).
    let watch = el().key(format!("trigger-focus-{focused}")).absolute(0.0, 0.0).size(0.0, 0.0).on_layout_tx(|s: &mut MainUi, _, _| focus_changed(s));
    let mut out = rune::col().gap(6.0).child(field.child(watch));
    if let Some(e) = &tr.error {
        out = out.child(text(e.clone()).font(ty::XS).color(t.destructive).wrap().role("alert").key("trigger-error"));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_like_the_web() {
        assert_eq!(validate_trigger("", &[], false), Err("Enter a trigger first.".into()));
        assert_eq!(validate_trigger("   ", &[], false), Err("A trigger can't be only spaces.".into()));
        assert_eq!(validate_trigger(" ;sig ", &[], false), Ok(";sig".into()));
        assert!(validate_trigger(";SIG", &[";sig".into()], false).is_err());
        assert!(validate_trigger(";SIG", &[";sig".into()], true).is_ok());
        let long = "x".repeat(65);
        assert_eq!(validate_trigger(&long, &[], false), Err("Triggers can be at most 64 characters; this one has 65.".into()));
        assert!(validate_trigger(&"👍🏽".repeat(64), &[], false).is_ok());
    }
}
