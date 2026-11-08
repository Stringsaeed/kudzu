//! `Expansion` in settings.tsx: pause, the trigger prefix, the default insertion method, and the
//! "Good to know" notes.

use rune::{col, rich, row, Align, Cx, El, Span, TextEdit, TextStyle};
use serde_json::json;

use super::{desc, group, page, row_el, sentence, Inline};
use crate::ui::kit::input::{field, shell, Input};
use crate::ui::kit::segmented::Segmented;
use crate::ui::kit::switch::Switch;
use crate::ui::main::insights::whole;
use crate::ui::main::MainUi;
use crate::ui::theme::{radius, text as ty, weight, Theme};
use crate::ui::theme_for;

/// `<Input className="h-8 w-20 text-center …">`: a field whose text is centred ([`centered`] boxes it).
pub(super) fn centered_input(edit: &TextEdit, label: &str, font: TextStyle, access: fn(&mut MainUi) -> &mut TextEdit, change: impl Fn(&mut MainUi, &mut rune::Tx) + 'static) -> Input {
    Input::new(edit).label(label.to_string()).font(font).on_edit(access).on_change(change)
}

/// Builds a [`centered_input`] into its box.
pub(super) fn centered(th: &Theme, input: Input, focused: bool, width: f32) -> El {
    let f = field(th, focused, false, false, true);
    let editor = input.editor(th).align(Align::Center);
    shell(row().role("textbox").h(32.0).w(width).px(11.0).center_y().child(editor), &f, radius::LG)
}

fn prefix_edit(s: &mut MainUi) -> &mut TextEdit {
    page(s).prefix.get_mut()
}

pub fn view(s: &MainUi, cx: &mut Cx<MainUi>) -> Vec<El> {
    let th = theme_for(cx);
    let t = &th.t;
    let st = &s.settings;
    {
        // The field mirrors the saved prefix unless it's being edited.
        let mut edit = s.settings_page.prefix.borrow_mut();
        if !edit.focused && edit.text != st.default_trigger_prefix {
            edit.replace_text(st.default_trigger_prefix.clone());
        }
    }
    let edit = s.settings_page.prefix.borrow().clone();
    let focused = edit.focused;
    let mono = TextStyle { family: rune::Family::Mono, ..ty::SM };
    let prefix = centered_input(&edit, "Trigger prefix", mono, prefix_edit, |s: &mut MainUi, _| {
        // `maxLength={4}`.
        let mut v = s.settings_page.prefix.get_mut().text.clone();
        if v.chars().count() > 4 {
            v = v.chars().take(4).collect();
            s.settings_page.prefix.get_mut().replace_text(v.clone());
        }
        s.update_settings(json!({ "defaultTriggerPrefix": v }));
    });
    let insertion = match st.default_insertion {
        kudzu_core::InsertionMethod::Automatic => "automatic",
        kudzu_core::InsertionMethod::Type => "type",
        kudzu_core::InsertionMethod::Paste => "paste",
    };
    let expanding = group(
        th,
        "Expanding",
        None,
        vec![
            row_el(
                th,
                "Pause all snippets",
                Some(sentence(th, vec![Inline::Text("Handy while gaming or presenting. ".into()), Inline::Keys("mod+shift+p"), Inline::Text(" also works.".into())])),
                vec![Switch::new(s.status.paused)
                    .label("Pause all snippets")
                    .id("pause")
                    .on_toggle(|s: &mut MainUi| {
                        let paused = !s.status.paused;
                        s.set_paused(paused)
                    })
                    .build(th)],
            ),
            row_el(th, "Trigger prefix", Some(desc(th, "Suggested at the start of new triggers so they don't fire mid-sentence.")), vec![centered(th, prefix, focused, 80.0)]),
            row_el(
                th,
                "Insert by",
                Some(desc(th, "Default for every snippet. Typing is most compatible; pasting is fastest for long text.")),
                vec![Segmented::new("settings-insertion", "Default insertion method")
                    .option("automatic", "Automatic")
                    .option("type", "Type")
                    .option("paste", "Paste")
                    .selected(insertion)
                    .on_change(|s: &mut MainUi, v: &str| s.update_settings(json!({ "defaultInsertion": v })))
                    .build(th, cx)],
            ),
        ],
    );
    // `flex flex-col gap-2 py-3 text-xs leading-relaxed text-muted-foreground`, each note's lead
    // in `font-medium text-foreground`.
    let relaxed = ty::XS.lh(12.0 * 1.625);
    let note = |lead: &str, rest: &str| {
        let body = format!("{lead}{rest}");
        let n = lead.len();
        rich(body, vec![Span::new(0, n).weight(weight::MEDIUM).color(t.foreground)]).font(relaxed).color(t.muted_foreground).wrap()
    };
    let notes = whole(col()).gap(8.0).py(12.0).children([
        note("Undo an expansion", " — press Backspace right after it to get your trigger back."),
        note("Clipboard is safe", " — when Kudzu pastes, it restores whatever you had copied, and marks its own text so clipboard managers ignore it."),
        note("Secure Input", " — while a password field is focused, macOS blocks all text expanders. Kudzu shows this in the menu bar."),
    ]);
    vec![expanding, group(th, "Good to know", None, vec![notes])]
}
