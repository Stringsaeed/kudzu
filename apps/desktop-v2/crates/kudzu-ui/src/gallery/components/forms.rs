//! `switch`, `checkbox`, `input` (with `input-group`), `textarea`, `label`, `select`.

use rune::{col, text, El, Family, TextEdit, TextStyle};
use rune_kit::{Checkbox, Field, Select, SelectAppearance, State};

use super::line;
use crate::theme::{icons, Theme};
use crate::widgets::kit_ext::{
    input::Input,
    switch::{Size, Switch},
    textarea::Textarea,
};

pub fn switch(theme: &Theme) -> El {
    let sized = |name: &str, size: Size, on: bool, state: State| {
        let (w, h, _) = size.metrics();
        Switch::new(on)
            .size(size)
            .state(state)
            .label(name)
            .build(theme)
            .key(format!("spec:switch/{name}:{w}x{h}"))
    };
    let reference = [
        Switch::new(true)
            .label("Open at login")
            .build(theme)
            .key("parity:main:settings-general:switch:1:Open at login"),
        Switch::new(false)
            .label("Pause all snippets")
            .build(theme)
            .key("parity:main:settings-expansion:switch:1:Pause all snippets"),
        Switch::new(true)
            .size(Size::Sm)
            .label("Enabled")
            .build(theme)
            .key("parity:main:library:switch:1:Enabled"),
    ];
    col()
        .gap(24.0)
        .child(line(
            theme,
            "Default 32 × 18.4: off, on, pressed, focus, disabled",
            [
                sized("default-off", Size::Default, false, State::Rest),
                sized("default-on", Size::Default, true, State::Rest),
                sized("default-pressed", Size::Default, false, State::Pressed),
                sized("default-focus", Size::Default, true, State::Focus),
                sized("default-disabled", Size::Default, true, State::Disabled),
            ],
        ))
        .child(line(
            theme,
            "Sm 24 × 14: off, on, pressed",
            [
                sized("sm-off", Size::Sm, false, State::Rest),
                sized("sm-on", Size::Sm, true, State::Rest),
                sized("sm-pressed", Size::Sm, true, State::Pressed),
            ],
        ))
        .child(line(theme, "Reference landmarks", reference))
}

/// rune-kit's 16 point box (`size-4`, 4 point corners). Its checked fill is the kit's
/// `surface_weak`, not shadcn's primary: see the matrix.
pub fn checkbox(theme: &Theme) -> El {
    let k = theme.kit();
    col().child(line(
        theme,
        "Off, on, focus, disabled",
        [
            Checkbox::new(false)
                .label("Also clear pinned items")
                .build(k)
                .key("info:checkbox/off"),
            Checkbox::new(true)
                .label("Also clear pinned items")
                .build(k)
                .key("info:checkbox/on"),
            Checkbox::new(true)
                .state(State::Focus)
                .build(k)
                .key("info:checkbox/focus"),
            Checkbox::new(false)
                .state(State::Disabled)
                .build(k)
                .key("info:checkbox/disabled"),
        ],
    ))
}

fn edit(value: &str, focused: bool) -> TextEdit {
    let mut e = TextEdit::new(value);
    e.focused = focused;
    e
}

pub fn input(theme: &Theme) -> El {
    let field = |key: &str, value: &str, state: State| {
        Input::new(&edit(value, false))
            .with(|t| t.placeholder("Placeholder").width(Some(180.0)))
            .state(state)
            .build(theme)
            .key(format!("spec:input/{key}:180x32"))
    };
    let reference = [Input::new(&edit(";", false))
        .with(|t| t.label("Trigger prefix").width(Some(80.0)))
        .build(theme)
        .key("parity:main:settings-expansion:textbox:1:Trigger prefix")];
    col()
        .gap(24.0)
        .child(line(
            theme,
            "Input: empty, filled, focus, invalid, disabled",
            [
                field("empty", "", State::Rest),
                field("filled", "Kudzu", State::Rest),
                Input::new(&edit("Kudzu", true))
                    .with(|t| t.width(Some(180.0)))
                    .build(theme)
                    .key("spec:input/focus:180x32"),
                Input::new(&edit("Kudzu", false))
                    .with(|t| t.width(Some(180.0)))
                    .invalid(true)
                    .build(theme)
                    .key("spec:input/invalid:180x32"),
                field("disabled", "Kudzu", State::Disabled),
            ],
        ))
        .child(line(
            theme,
            "Input group: leading icon (the filter fields)",
            [Input::new(&edit("", false))
                .leading_icon(&icons::SEARCH)
                .with(|t| t.placeholder("Search").width(Some(180.0)))
                .build(theme)
                .key("spec:input-group/leading:180x32")],
        ))
        .child(line(theme, "Reference landmarks", reference))
}

pub fn textarea(theme: &Theme) -> El {
    let area = |key: &str, value: &str, focused: bool| {
        Textarea::new(&edit(value, focused))
            .placeholder("One option per line")
            .build(theme)
            .w(296.0)
            .key(format!("spec:textarea/{key}:296x_"))
    };
    // The choice chip's options: `className="min-h-20 font-mono text-xs"`, where `md:text-sm`
    // still sets the size.
    let mono = TextStyle::new(14.0, 400.0).family(Family::Mono).lh(20.0);
    col().gap(24.0).child(line(
        theme,
        "Empty (min-h-16), two lines, focus",
        [
            area("empty", "", false),
            Textarea::new(&edit("Mac\nLinux", false))
                .label("Mac Linux")
                .min_height(80.0)
                .text_style(mono)
                .build(theme)
                .w(296.0)
                .key("parity:main:chip-choice:textbox:1:Mac Linux"),
            area("focus", "Mac\nLinux\nWindows", true),
        ],
    ))
}

/// shadcn's `Label` (`text-sm leading-none font-medium`) over its control: rune-kit's `Field`.
pub fn label(theme: &Theme) -> El {
    let k = theme.kit();
    col().gap(16.0).child(line(
        theme,
        "Field label over an input; a plain label",
        [
            Field::new()
                .label("Group name")
                .control(
                    Input::new(&edit("Work", false))
                        .with(|t| t.width(Some(180.0)))
                        .build(theme),
                )
                .build(k)
                .key("info:label/field"),
            text("Default value")
                .font(theme.ty.control)
                .color(theme.c.foreground)
                .key("info:label/plain"),
        ],
    ))
}

pub fn select(theme: &Theme, layer: &mut Vec<El>) -> El {
    let k = theme.kit();
    const KEEP: [&str; 4] = ["1 day", "1 week", "1 month", "Forever"];
    let closed = Select::new("keep-closed", &KEEP)
        .label("Keep history for")
        .appearance(SelectAppearance::Field)
        .width(128.0)
        .current(Some("1 month"))
        .build(k, layer);
    let closed = col().key("info:select/trigger").child(closed);
    let open = Select::new("keep-open", &KEEP)
        .label("Keep history for (open)")
        .appearance(SelectAppearance::Field)
        .width(128.0)
        .current(Some("1 week"))
        .open(true)
        .build(k, layer);
    let open = col().key("info:select/open").child(open);
    col()
        .gap(24.0)
        .child(line(theme, "Trigger; open list", [closed, open]))
}
