//! `breadcrumb`, `selection-pill`, `slider`, `stepper`.

use rune::{col, Cx, El};

use crate::gallery::components::line;
use crate::gallery::GalleryState;
use crate::theme::Theme;
use crate::widgets::breadcrumbs::{Breadcrumbs, Crumb};
use crate::widgets::selection_pill::{Segment, SegmentSize, Segmented};
use crate::widgets::slider::Slider;
use crate::widgets::stepper::Stepper;

fn scratch(s: &mut GalleryState) -> &mut rune::TextEdit {
    &mut s.scratch
}

fn noop_select(_: &mut GalleryState, _: usize) {}
fn noop_menu(_: &mut GalleryState, _: bool) {}

fn trail(labels: &[&str]) -> Vec<Crumb> {
    labels.iter().map(|l| Crumb::new(*l)).collect()
}

/// The trails S02 captured (the main toolbar's, at its 891 point width, and the clipboard
/// item's, hugging its content), then depth past four, truncation and a hovered ancestor.
///
/// The crumbs carry the reference keys; the trails are sized by `spec:` keys, because the
/// reference records the `nav` element's inherited colour (foreground) while every crumb drawn
/// inside it is muted, in both apps (see the S07 report).
pub fn breadcrumb(theme: &Theme, layer: &mut Vec<El>) -> El {
    let crumb = |label: &str, key: Option<&str>| match key {
        Some(k) => Crumb::new(label).sample(k),
        None => Crumb::new(label),
    };
    // The trail fills its row, as in the toolbar: the row sets the width.
    let sized = |e: El, w: f32, key: &str| rune::row().w(w).child(e.key(key.to_owned()));
    let nav = |items: Vec<Crumb>, layer: &mut Vec<El>| {
        Breadcrumbs::new(items).build(theme, layer, noop_select, noop_menu)
    };
    let library = nav(
        vec![
            crumb("Snippets", Some("parity:main:library:button:1:Snippets")),
            crumb("Work", Some("parity:main:library:button:1:Work")),
            crumb("Email", Some("parity:main:library:button:1:Email")),
            crumb("Welcome email", None),
        ],
        layer,
    );
    let library = sized(library, 891.0, "spec:breadcrumb/toolbar:891x20");
    let settings = nav(
        vec![
            crumb(
                "Settings",
                Some("parity:main:settings-general:button:1:Settings"),
            ),
            crumb("General", None),
        ],
        layer,
    );
    let settings = sized(settings, 891.0, "spec:breadcrumb/settings:891x20");
    let item = Breadcrumbs::new(vec![
        crumb(
            "History",
            Some("parity:main:clipboard-history:button:2:History"),
        ),
        crumb("Hi Maya,", None),
    ])
    .label("Clipboard item breadcrumb")
    .hug()
    .build(theme, layer, noop_select, noop_menu)
    .key("spec:breadcrumb/clipboard-item:126.688x20");
    let deep = nav(
        trail(&[
            "Snippets",
            "Work",
            "Clients",
            "Acme",
            "Email",
            "Welcome email",
        ]),
        layer,
    );
    let deep = sized(deep, 480.0, "spec:breadcrumb/collapsed:480x24");
    let narrow = nav(
        trail(&[
            "Snippets",
            "A group with a very long name",
            "A snippet with a long name too",
        ]),
        layer,
    );
    let narrow = sized(narrow, 240.0, "spec:breadcrumb/truncated:240x20");
    let hovered = Breadcrumbs::new(trail(&["Snippets", "Work", "Email"]))
        .hovered(1)
        .build(theme, layer, noop_select, noop_menu);
    let hovered = sized(hovered, 320.0, "spec:breadcrumb/hover:320x20");
    col()
        .gap(24.0)
        .child(line(theme, "Toolbar trail (reference)", [library]))
        .child(line(theme, "Settings trail (reference)", [settings]))
        .child(line(
            theme,
            "Clipboard item trail, hugging (reference)",
            [item],
        ))
        .child(line(theme, "Past four items; truncated", [deep, narrow]))
        .child(line(theme, "Hovered ancestor", [hovered]))
}

fn segmented(
    key: &str,
    label: &str,
    value: &str,
    options: &[(&str, &str, Option<&str>)],
) -> Segmented {
    options
        .iter()
        .fold(Segmented::new(key, label, value), |s, (v, l, sample)| {
            let mut o = Segment::new(*v, *l);
            if let Some(k) = sample {
                o = o.sample(*k);
            }
            s.option(o)
        })
}

/// The segmented controls S02 captured (Appearance, Expansion method), and the `sm` size.
pub fn selection_pill(theme: &Theme, cx: &Cx<GalleryState>) -> El {
    let change = |_: &mut GalleryState, _: &str| {};
    let appearance = segmented(
        "appearance",
        "Appearance",
        // Each capture shows its own theme chosen.
        if theme.dark { "dark" } else { "light" },
        &[
            (
                "dark",
                "Dark",
                Some("parity:main:settings-general:radio:1:Dark"),
            ),
            (
                "light",
                "Light",
                Some("parity:main:settings-general:radio:1:Light"),
            ),
            (
                "system",
                "System",
                Some("parity:main:settings-general:radio:1:System"),
            ),
        ],
    )
    .build(theme, cx, change);
    let method = segmented(
        "method",
        "Expansion method",
        "automatic",
        &[
            (
                "automatic",
                "Automatic",
                Some("parity:main:settings-expansion:radio:1:Automatic"),
            ),
            (
                "type",
                "Type",
                Some("parity:main:settings-expansion:radio:1:Type"),
            ),
            (
                "paste",
                "Paste",
                Some("parity:main:settings-expansion:radio:1:Paste"),
            ),
        ],
    )
    .build(theme, cx, change);
    let small = segmented(
        "small",
        "Small",
        "b",
        &[
            ("a", "One", None),
            ("b", "Two", Some("spec:segmented/sm/selected:_x24")),
            ("c", "Three", None),
        ],
    )
    .size(SegmentSize::Sm)
    .build(theme, cx, change);
    let none = segmented(
        "none",
        "Nothing selected",
        "",
        &[("a", "One", None), ("b", "Two", None)],
    )
    .build(theme, cx, change)
    .key("spec:segmented/none:_x32");
    col()
        .gap(24.0)
        .child(line(
            theme,
            "Appearance; expansion method (reference)",
            [appearance, method],
        ))
        .child(line(
            theme,
            "Sm; nothing selected (the pill fades out)",
            [small, none],
        ))
}

/// The Insights typing-speed slider (`w-48`), at rest and active (hover, focus or drag).
pub fn slider(theme: &Theme) -> El {
    let change = |_: &mut GalleryState, _: f64| {};
    let s = |v: f64, active: bool, key: &str| {
        Slider::new(v, 10.0, 150.0)
            .step(5.0)
            .label("Typing speed in words per minute")
            .value_text(format!("{v} wpm"))
            .active(active)
            .build(theme, change)
            .key(key.to_owned())
    };
    col().child(line(
        theme,
        "At rest, active, at the ends",
        [
            s(40.0, false, "spec:slider/rest:192x12"),
            s(90.0, true, "spec:slider/active:192x12"),
            s(10.0, false, "spec:slider/min:192x12"),
            s(150.0, false, "spec:slider/max:192x12"),
        ],
    ))
}

/// The chip forms' number fields, as wide as the popover's fields (296).
pub fn stepper(theme: &Theme, s: &GalleryState) -> El {
    let field = |label: &str, value: i64, min, max, step, key: &str| {
        let mut edit = s.scratch.clone();
        edit.text = value.to_string();
        Stepper::new(&edit, scratch, value, min, max)
            .label(label)
            .step(step)
            .width(296.0)
            .build(theme, |_: &mut GalleryState, _| {})
            .key(key.to_owned())
    };
    col().gap(16.0).child(line(
        theme,
        "Times, milliseconds, copies back (reference)",
        [
            field("Times", 2, 1, 50, 1, "parity:main:chip-key:textbox:1:Times"),
            field(
                "Milliseconds",
                300,
                1,
                5000,
                50,
                "parity:main:chip-delay:textbox:1:Milliseconds",
            ),
            field(
                "Copies back",
                1,
                0,
                99,
                1,
                "parity:main:chip-clipboard:textbox:1:Copies back",
            ),
        ],
    ))
}
