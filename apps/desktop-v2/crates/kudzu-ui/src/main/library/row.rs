//! One row of the list (`snippet-list.tsx` `Row`): 64 points tall with a 60 point item inset
//! 8 at the sides. Name (with the query's matches), favorite and off marks, the first trigger
//! with a count of the rest; the summary and the use count below.
//!
//! The row is a `memo` whose revision is its [`RowData`] plus how it is selected, so a row
//! rebuilds only when something it draws changed. Its entrance plays when it arrived with a
//! small library change; a row the virtual list remounts never replays it.

use std::hash::{DefaultHasher, Hash, Hasher};

use rune::{col, icon, memo, row, text, El, Span, TextStyle, Tx};
use uuid::Uuid;

use super::model::{ClickMods, RowData};
use super::{drag, row_key, sync};
use crate::main::state::MainPane;
use crate::shell::MainState;
use crate::theme::{icons, radius, Theme};

/// The virtual row's height.
pub const ROW_H: f32 = 64.0;
/// The item inside it (`h-[60px]`).
const ITEM_H: f32 = 60.0;

/// How the row is selected.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub(super) struct Look {
    /// The open snippet.
    pub cursor: bool,
    /// In the selection (`aria-selected`).
    pub selected: bool,
    /// The list has focus, or nothing but the cursor is selected: the cursor shows iris.
    pub active: bool,
    /// Arrived with the last small library change: rises in.
    pub fresh: bool,
    pub dark: bool,
    /// The trigger's widest, in points (`max-w-[45%]` of the item's content).
    pub trigger_max: u16,
}

pub(super) fn build(data: RowData, look: Look, theme: &Theme) -> El {
    let key = row_key(data.id);
    let mut version = DefaultHasher::new();
    (&data, look).hash(&mut version);
    row()
        .key(key.clone())
        .h(ROW_H)
        .w_full()
        .center_y()
        .child(memo(key, version.finish(), move || {
            item(&data, look, theme)
        }))
}

fn item(data: &RowData, look: Look, theme: &Theme) -> El {
    let c = &theme.c;
    let ty = &theme.ty;
    let id = data.id;
    let name_ink = if data.off || data.untitled {
        c.muted_foreground
    } else {
        c.foreground
    };
    let spans: Vec<Span> = data
        .marks
        .iter()
        .map(|r| {
            Span::new(r.start, r.end)
                .color(c.iris_foreground)
                .underline(c.iris.alpha(0.5))
        })
        .collect();
    let name_style = if data.untitled {
        ty.body.italic()
    } else {
        ty.body
    };
    let mut name = text(data.name.clone())
        .font(name_style)
        .color(name_ink)
        .truncate()
        .grow(1.0)
        .min_w(0.0);
    if !spans.is_empty() {
        name = name.spans(spans);
    }
    let mut top = row().w_full().gap(8.0).center_y().child(name);
    if data.favorite {
        top = top.child(
            icon(&icons::STAR, 12.0)
                .color(c.favorite)
                .label("Favorite")
                .transition(crate::motion::check_in()),
        );
    }
    if data.off {
        top = top.child(
            icon(&icons::CIRCLE_OFF, 12.0)
                .color(c.faint)
                .label("Turned off")
                .transition(crate::motion::check_in()),
        );
    }
    if let Some(trigger) = &data.trigger {
        let ink = if data.off {
            c.faint
        } else {
            c.iris_foreground.alpha(0.9)
        };
        let mut label = trigger.clone();
        let mut spans = Vec::new();
        if data.more_triggers > 0 {
            let at = label.len();
            label.push_str(&format!(" +{}", data.more_triggers));
            spans.push(Span::new(at, label.len()).color(c.faint));
        }
        top = top.child(
            text(label)
                .font(TextStyle {
                    size: 11.5,
                    line_height: 16.0,
                    ..ty.mono
                })
                .color(ink)
                .spans(spans)
                .truncate()
                .max_w(f32::from(look.trigger_max)),
        );
    }
    let small = TextStyle {
        weight: 400.0,
        ..ty.control_xs
    };
    let summary = if data.summary.is_empty() {
        text("Empty").font(small.italic())
    } else {
        text(data.summary.to_string()).font(small)
    };
    let mut bottom = row().w_full().gap(8.0).center_y().child(
        summary
            .color(c.muted_foreground.alpha(0.8))
            .truncate()
            .grow(1.0)
            .min_w(0.0),
    );
    if data.usage > 0 {
        bottom = bottom.child(
            text(format!("{}×", kudzu_core::fmt::format_count(data.usage)))
                .font(ty.meta_10_5.features("'tnum' 1"))
                .color(c.faint),
        );
    }

    let rest = c.foreground.alpha(0.0);
    let (fill, hover) = if look.cursor && look.active {
        (c.iris_soft, c.iris_soft)
    } else if look.selected {
        (c.foreground.alpha(0.06), c.foreground.alpha(0.07))
    } else {
        (rest, c.foreground.alpha(0.035))
    };
    let mut el = col()
        .role("option")
        .label(data.name.clone())
        .focus_selected(look.selected)
        .mx(8.0)
        .grow(1.0)
        .min_w(0.0)
        .h(ITEM_H)
        .px(13.0)
        .gap(4.0)
        .justify_center()
        .radius(radius::LG)
        .fill(fill)
        .hover(hover)
        .child(top)
        .child(bottom)
        .on_press_tx(move |s: &mut MainState, tx| press(s, id, tx))
        .on_drag(move |s: &mut MainState, d, tx| drag::gesture(s, id, d, tx));
    if look.cursor && look.active {
        el = el.border(1.0, c.iris.alpha(0.35));
    } else if look.selected {
        el = theme
            .depth
            .selected
            .iter()
            .fold(el, |e, shadow| e.box_shadow(*shadow));
    }
    if look.fresh {
        el = el.transition(crate::motion::rise());
    }
    el
}

/// `onRowClick`: a plain click selects, ⌘ toggles the row in the selection, ⇧ takes the range
/// from the anchor. The list keeps keyboard focus.
fn press(s: &mut MainState, id: Uuid, tx: &mut Tx) {
    if !drag::take_press(s) {
        return;
    }
    let Some(listing) = s.library.listing(s) else {
        return;
    };
    let m = tx.mods();
    let mods = ClickMods {
        toggle: m.cmd || m.ctrl,
        range: m.shift,
    };
    let cursor = s.frame.selected_snippet;
    let next = s.library.selection.click(&listing.ids, cursor, id, mods);
    s.select_snippet(Some(next));
    s.blur_all_inputs();
    s.frame.focus.move_to(MainPane::List);
    tx.focus(super::LIST_KEY, false);
    sync(s);
}
