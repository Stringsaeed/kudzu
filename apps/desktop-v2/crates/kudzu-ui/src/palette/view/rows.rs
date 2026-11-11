//! The result list: section, hits, the create row, and the empty-scope copy.
use kudzu_core::KudzuLibrary;
use rune::{col, icon, row, text, El, Icon, ScrollSpec, Span, TextStyle};

use crate::shell::windows::PaletteState;
use crate::theme::icons;
use crate::theme::{radius, Theme};
use crate::widgets::shortcut::{Shortcut, Tone};
use crate::widgets::trigger_chip::trigger_chip;

use super::super::clamped;
use super::super::rank::{kind_of, Kind, Row};

pub(super) fn results(state: &PaletteState, theme: &Theme, library: &KudzuLibrary) -> El {
    let presentation = &state.model.search.presentation;
    let active = clamped(state);
    let mut list = col().key("palette-rows").gap(0.0).w_full();
    if let Some(empty) = presentation.empty {
        list = list.child(empty_state(theme, empty, state.model.search.keys.scope));
    } else {
        list = list.child(section(
            theme,
            presentation.section,
            presentation.group_chip.as_deref(),
        ));
        for (index, item) in presentation.rows.iter().enumerate() {
            list = list.child(hit(
                theme,
                library,
                item,
                index,
                index == active,
                state.model.search.keys.show_numbers,
            ));
        }
        if presentation.create {
            let index = presentation.rows.len();
            list = list.child(create_row(
                theme,
                &state.model.search.keys.query,
                index,
                index == active,
            ));
        }
    }
    col()
        .key("palette-list")
        .role("listbox")
        .label(presentation.section)
        .w_frac(0.54)
        .h_full()
        .min_h(0.0)
        .padding(6.0)
        .scroll(ScrollSpec {
            stick_bottom: false,
        })
        .child(list)
}

fn section(theme: &Theme, title: &str, chip: Option<&str>) -> El {
    let c = &theme.c;
    let style = TextStyle::new(11.0, 500.0);
    let mut line = row()
        .key("palette-section")
        .px(10.0)
        .pt(6.0)
        .pb(4.0)
        .gap(4.0)
        .center_y()
        .child(text(title.to_owned()).font(style).color(c.faint));
    if let Some(chip) = chip {
        line = line.child(text(chip.to_owned()).font(style).color(c.iris_foreground));
    }
    line
}

fn empty_state(theme: &Theme, message: &str, scope: super::super::rank::Scope) -> El {
    let c = &theme.c;
    let glyph = match scope {
        super::super::rank::Scope::Favorites => &icons::STAR,
        super::super::rank::Scope::Recent => &icons::HISTORY,
        super::super::rank::Scope::All => &icons::FILE_TEXT,
    };
    col()
        .key("palette-empty")
        .w_full()
        .grow(1.0)
        .center()
        .gap(4.0)
        .px(24.0)
        .child(
            row()
                .size(36.0, 36.0)
                .center()
                .radius(radius::LG)
                .fill(c.foreground.alpha(0.04))
                .child(icon(glyph, 16.0).color(c.faint)),
        )
        .child(
            text(message.to_owned())
                .font(TextStyle::new(14.0, 400.0))
                .color(c.muted_foreground),
        )
        .child(
            text("Type to create one.".to_owned())
                .font(TextStyle::new(12.0, 400.0))
                .color(c.faint),
        )
}

fn hit(
    theme: &Theme,
    library: &KudzuLibrary,
    item: &Row,
    index: usize,
    active: bool,
    numbers: bool,
) -> El {
    let c = &theme.c;
    let content = library
        .snippets
        .iter()
        .find(|snippet| snippet.id == item.id)
        .map(|snippet| snippet.content.as_str())
        .unwrap_or("");
    let mut line = row()
        .key(format!("pal-row-{index}"))
        .role("option")
        .label(item.name.clone())
        .h(44.0)
        .w_full()
        .px(10.0)
        .gap(12.0)
        .center_y()
        .radius(radius::LG)
        .on_hover_change(move |s: &mut PaletteState, on, tx| {
            if on {
                s.model.search.keys.hover(index);
                tx.scroll_into_view(format!("pal-row-{index}"));
            }
        })
        .on_press_tx(move |s: &mut PaletteState, tx| super::super::pick(s, index, tx));
    if active {
        line = line.fill(c.foreground.alpha(0.08));
    }
    if !item.enabled {
        line = line.opacity(0.6);
    }
    line = line
        .child(kind_box(theme, kind_of(content), active))
        .child(titles(theme, item))
        .child(row().grow(1.0));
    if let Some(trigger) = item.triggers.first() {
        line = line.child(trigger_chip(theme, trigger, !active));
    }
    if index < 9 {
        let tone = if numbers { Tone::Accent } else { Tone::Default };
        line = line.child(
            Shortcut::combo(&format!("mod+{}", index + 1))
                .tone(tone)
                .build(theme),
        );
    }
    line
}

fn titles(theme: &Theme, item: &Row) -> El {
    let c = &theme.c;
    let mut name = text(item.name.clone())
        .font(TextStyle::new(13.5, 500.0))
        .color(c.foreground);
    if !item.highlights.is_empty() {
        let spans = item
            .highlights
            .iter()
            .filter(|range| range.end <= item.name.len())
            .map(|range| Span::new(range.start, range.end).color(c.iris_foreground))
            .collect::<Vec<_>>();
        name = name.spans(spans);
    }
    let mut block = col().key("titles").gap(0.0).shrink().child(
        row()
            .gap(6.0)
            .center_y()
            .child(name)
            .child(star(theme, item.favorite)),
    );
    if !item.path.is_empty() {
        block = block.child(
            text(item.path.clone())
                .font(TextStyle::new(11.0, 400.0))
                .color(c.faint),
        );
    }
    block
}

fn star(theme: &Theme, favorite: bool) -> El {
    if favorite {
        icon(&icons::STAR, 12.0).color(theme.c.favorite)
    } else {
        row()
    }
}

fn kind_box(theme: &Theme, kind: Kind, active: bool) -> El {
    let c = &theme.c;
    let (fill, ink) = if active {
        (c.iris, c.iris_contrast)
    } else {
        (c.foreground.alpha(0.06), c.muted_foreground)
    };
    row()
        .size(28.0, 28.0)
        .center()
        .radius(radius::MD)
        .fill(fill)
        .child(icon(kind_icon(kind), 14.0).color(ink))
}

fn kind_icon(kind: Kind) -> &'static Icon {
    match kind {
        Kind::Form => &icons::TEXT_CURSOR_INPUT,
        Kind::Clipboard => &icons::CLIPBOARD,
        Kind::Calendar => &icons::CALENDAR_DAYS,
        Kind::File => &icons::FILE_TEXT,
    }
}

fn create_row(theme: &Theme, query: &str, index: usize, active: bool) -> El {
    let c = &theme.c;
    let label = format!("Create snippet “{}”", query.trim());
    let mut line = row()
        .key(format!("pal-row-{index}"))
        .role("option")
        .label("Create snippet")
        .h(44.0)
        .w_full()
        .px(10.0)
        .gap(12.0)
        .center_y()
        .radius(radius::LG)
        .on_hover_change(move |s: &mut PaletteState, on, tx| {
            if on {
                s.model.search.keys.hover(index);
                tx.scroll_into_view(format!("pal-row-{index}"));
            }
        })
        .on_press_tx(move |s: &mut PaletteState, tx| super::super::pick(s, index, tx))
        .child(
            row()
                .size(28.0, 28.0)
                .center()
                .radius(radius::MD)
                .fill(c.foreground.alpha(0.06))
                .child(icon(&icons::PLUS, 14.0).color(c.foreground)),
        )
        .child(
            text(label)
                .font(TextStyle::new(13.5, 400.0))
                .color(if active {
                    c.foreground
                } else {
                    c.muted_foreground
                }),
        );
    if active {
        line = line.fill(c.foreground.alpha(0.08));
    }
    line
}
