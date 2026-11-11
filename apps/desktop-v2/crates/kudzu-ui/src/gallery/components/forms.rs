//! `switch`, `checkbox`, `input` (with `input-group`), `textarea`, `label`, `select`.

use rune::{col, row, text, El, Family, TextEdit, TextStyle};
use rune_kit::{Checkbox, Field, Select, SelectAppearance, State};

use super::line;
use crate::theme::{icons, Theme};
use crate::widgets::kit_ext::{
    button::{Button, Size as ButtonSize, Variant},
    checkbox,
    input::Input,
    select,
    switch::{Size, Switch},
    textarea::Textarea,
};
use crate::widgets::shortcut::{Shortcut, Tone};

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

/// rune-kit's 16 point box (`size-4`, 4 point corners), checked in shadcn's primary through
/// `kit_ext::checkbox` (D6, after S06).
pub fn checkbox(theme: &Theme) -> El {
    let b = |c: Checkbox| checkbox::build(c, theme);
    col().child(line(
        theme,
        "Off, on, focus, disabled",
        [
            b(Checkbox::new(false).label("Also clear pinned items")).key("info:checkbox/off"),
            b(Checkbox::new(true).label("Also clear pinned items")).key("info:checkbox/on"),
            b(Checkbox::new(true).state(State::Focus)).key("info:checkbox/focus"),
            b(Checkbox::new(false).state(State::Disabled)).key("info:checkbox/disabled"),
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

/// The select trigger at Kudzu's 32 points with the raised depth, and its list at 10 point
/// corners (`kit_ext::select`, D6 after S06).
pub fn select(theme: &Theme, layer: &mut Vec<El>) -> El {
    const KEEP: [&str; 4] = ["1 day", "1 week", "1 month", "Forever"];
    let closed = select::build(
        Select::new("keep-closed", &KEEP)
            .label("Keep history for")
            .appearance(SelectAppearance::Field)
            .width(128.0)
            .current(Some("1 week")),
        theme,
        layer,
        false,
        State::Rest,
    )
    .key("parity:main:settings-clipboard:combobox:1:Keep history for");
    let hover = select::build(
        Select::new("keep-hover", &KEEP)
            .label("Keep history for (hover)")
            .appearance(SelectAppearance::Field)
            .width(128.0)
            .current(Some("1 week"))
            .state(State::Hover),
        theme,
        layer,
        false,
        State::Hover,
    )
    .key("spec:select/hover:128x32");
    let open = select::build(
        Select::new("keep-open", &KEEP)
            .label("Keep history for (open)")
            .appearance(SelectAppearance::Field)
            .width(128.0)
            .current(Some("1 week"))
            .open(true),
        theme,
        layer,
        true,
        State::Rest,
    );
    let open = col().key("spec:select/open:128x32").child(open);
    col().gap(24.0).child(line(
        theme,
        "Trigger, hover; open list",
        [closed, hover, open],
    ))
}

/// shadcn's `InputGroup` (the filter fields): a leading search icon and a trailing keycap
/// hint, rebuilt as the library's and clipboard's filters are (`snippet-list.tsx`: a 14 point
/// faint icon, `bg-foreground/[0.03]` in both themes, `pl-8 pr-14`, 13 point text), plus the
/// icon-only and button-addon variants.
pub fn input_group(theme: &Theme) -> El {
    let hint = |combo: &str| {
        row()
            .pr(8.0)
            .child(Shortcut::combo(combo).tone(Tone::Ghost).build(theme))
    };
    let filter = |name: &str, width: f32, key: &str| {
        Input::new(&edit("", false))
            .leading_icon(&icons::SEARCH)
            .icon_at(10.0)
            .with(|t| {
                t.placeholder(name.to_owned())
                    .label(name.to_owned())
                    .width(Some(width))
                    .leading_icon_size(14.0)
                    .trailing(hint("mod+f"))
            })
            .text_style(theme.ty.body)
            .build(theme)
            .fill(theme.c.foreground.alpha(0.03))
            .key(key.to_owned())
    };
    let addon = Input::new(&edit("", false))
        .with(|t| {
            t.placeholder("https://")
                .label("Link address")
                .width(Some(240.0))
                .trailing(
                    Button::new("Apply")
                        .variant(Variant::Ghost)
                        .size(ButtonSize::Xs)
                        .build(theme)
                        .mr(4.0),
                )
        })
        .build(theme)
        .key("spec:input-group/button-addon:240x32");
    col()
        .gap(24.0)
        .child(line(
            theme,
            "Icon start and keycap end (reference)",
            [
                filter(
                    "Filter snippets",
                    293.594,
                    "parity:main:library:textbox:1:Filter snippets",
                ),
                filter(
                    "Search clipboard history",
                    329.0,
                    "parity:main:clipboard-history:textbox:1:Search clipboard history",
                ),
            ],
        ))
        .child(line(
            theme,
            "Icon start only; button addon",
            [
                Input::new(&edit("", false))
                    .leading_icon(&icons::SEARCH)
                    .with(|t| t.placeholder("Search").width(Some(180.0)))
                    .build(theme)
                    .key("spec:input-group/leading:180x32"),
                addon,
            ],
        ))
}
