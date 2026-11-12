//! Content only the app that copied it understands: the note that every format is kept.

use rune::{col, icon, row, text, El};

use super::{well, Body};
use crate::theme::{icons, Theme};

/// "Something copied in Pages" or "App-specific data".
pub fn heading(b: &Body) -> String {
    b.entry.source.as_ref().map_or_else(
        || "App-specific data".to_owned(),
        |s| format!("Something copied in {}", s.name),
    )
}

pub fn view(theme: &Theme, b: &Body) -> El {
    let c = &theme.c;
    let into = b
        .entry
        .source
        .as_ref()
        .map_or_else(|| "back".to_owned(), |s| format!("into {}", s.name));
    well(theme)
        .key("clip-other-body")
        .padding(16.0)
        .gap(12.0)
        .items_start()
        .child(
            row()
                .size(40.0, 40.0)
                .center()
                .radius(14.0)
                .fill(c.foreground.alpha(0.06))
                .child(icon(&icons::PACKAGE, 20.0).color(c.muted_foreground)),
        )
        .child(
            col()
                .gap(4.0)
                .child(text(heading(b)).font(theme.ty.body_medium).color(c.foreground))
                .child(
                    text(format!(
                        "Kudzu can't show this kind of content, but it kept every format. Copy it again and it pastes {into} exactly as before."
                    ))
                    .font(theme.ty.control_xs)
                    .color(c.muted_foreground)
                    .wrap()
                    .max_w(384.0),
                ),
        )
}
