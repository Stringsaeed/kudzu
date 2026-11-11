//! Text, multi-line and choice controls in first-definition order.
use kudzu_core::template::{FormFieldKind, FormFieldSpec};
use rune::{col, text, El, Tx};
use rune_kit::{Select, SelectAppearance};

use super::place::FORM_W;
use crate::shell::windows::FormState;
use crate::theme::{radius, Theme};
use crate::widgets::kit_ext::{input::Input, select, textarea::Textarea};

const LABEL_GAP: f32 = 6.0;
const MULTILINE_MIN: f32 = 80.0;
/// Tauri form fields are `h-9` (36). S07 gallery inputs stay 32 (`h-8`).
const FIELD_H: f32 = 36.0;

pub fn field(state: &FormState, theme: &Theme, field: &FormFieldSpec, layer: &mut Vec<El>) -> El {
    let label = text(field.name.clone())
        .font(theme.ty.control_xs)
        .color(theme.c.muted_foreground);
    let control = match field.kind {
        FormFieldKind::Choice => choice(state, theme, field, layer),
        FormFieldKind::Multiline => multiline(state, theme, field),
        FormFieldKind::Text => single(state, theme, field),
    };
    col()
        .key(format!("field-{}", field.name))
        .gap(LABEL_GAP)
        .w_full()
        .child(label)
        .child(control)
}

fn single(state: &FormState, theme: &Theme, field: &FormFieldSpec) -> El {
    let name = field.name.clone();
    let edit = state.edit_snapshot(&name);
    let label = field.name.clone();
    Input::new(&edit)
        .with(move |input| {
            input
                .height(FIELD_H)
                .label(label)
                .on_edit(move |s: &mut FormState| s.edit(&name))
        })
        .build(theme)
        .h(FIELD_H)
        .w_full()
}

fn multiline(state: &FormState, theme: &Theme, field: &FormFieldSpec) -> El {
    let name = field.name.clone();
    let submit_name = field.name.clone();
    let edit = state.edit_snapshot(&name);
    Textarea::new(&edit)
        .min_height(MULTILINE_MIN)
        .label(field.name.clone())
        .on_edit(move |s: &mut FormState| s.edit(&name))
        .on_submit(move |s: &mut FormState, _: &mut Tx| s.break_line(&submit_name))
        .build(theme)
        .w_full()
}

fn choice(state: &FormState, theme: &Theme, field: &FormFieldSpec, layer: &mut Vec<El>) -> El {
    let name = field.name.clone();
    let toggle = name.clone();
    let close = name.clone();
    let choose = name.clone();
    let current = state
        .model
        .panel
        .as_ref()
        .and_then(|panel| panel.values.get(&field.name).cloned());
    let open = state
        .model
        .panel
        .as_ref()
        .is_some_and(|panel| panel.open_choice.as_deref() == Some(field.name.as_str()));
    let width = FORM_W as f32 - 32.0;
    select::build(
        Select::new(format!("choice-{}", field.name), &field.options)
            .appearance(SelectAppearance::Field)
            .label(field.name.clone())
            .width(width)
            .current(current.as_deref())
            .open(open)
            .on_toggle(move |s: &mut FormState| s.toggle_choice(&toggle))
            .on_close(move |s: &mut FormState| s.close_choice(&close))
            .on_choose(move |s: &mut FormState, value, _| s.choose(&choose, value)),
        theme,
        layer,
        open,
        rune_kit::State::Rest,
    )
    .h(FIELD_H)
    .w_full()
    .radius(radius::LG)
}
