//! Insights element tree (`components/app/main/insights.tsx`).

use kudzu_core::fmt::format_count;
use rune::{col, el, icon, row, text, Color, El, MotionIntent, ScrollSpec, Spring, TextStyle, Tx};
use uuid::Uuid;

use super::data::{self, Data, Ranked};
use crate::main::state::{LibraryFilter, MainView};
use crate::shell::MainState;
use crate::theme::{icons, motion, radius, Theme};
use crate::widgets::slider::Slider;
use crate::widgets::trigger_chip::trigger_chip;

const SKETCH: [f32; 3] = [1.0, 0.64, 0.36];
const BAR_MIN: f32 = 0.03;

pub fn tree(s: &MainState, theme: &Theme, reduced: bool) -> El {
    let data = data::from_state(s);
    s.insights.observe(s.frame.clock_secs, reduced, &data);
    let instant = reduced || s.insights.landed();
    col()
        .key("insights-view-slot")
        .grow(1.0)
        .w_full()
        .h_full()
        .min_h(0.0)
        .on_drawn_layout_tx(|st: &mut MainState, _, tx: &mut Tx| {
            if st.insights.arriving() {
                tx.wake_at(tx.clock() + 1.0 / 120.0);
            }
            tx.live_only();
        })
        .child(header(theme))
        .child(body(s, theme, &data, instant))
}

fn header(theme: &Theme) -> El {
    let c = &theme.c;
    col()
        .key("insights-header")
        .w_full()
        .child(
            row()
                .h(48.0)
                .w_full()
                .px(24.0)
                .center_y()
                .window_drag()
                .child(
                    text("Insights")
                        .key("insights-heading")
                        .role("heading")
                        .font(theme.ty.body_strong)
                        .color(c.foreground),
                ),
        )
        .child(
            el().key("insights-header-rule")
                .h(1.0)
                .w_full()
                .fill(c.hairline)
                .a11y_hidden(),
        )
}

fn body(s: &MainState, theme: &Theme, data: &Data, instant: bool) -> El {
    col()
        .key("insights-scroll")
        .grow(1.0)
        .w_full()
        .min_h(0.0)
        .scroll(ScrollSpec {
            stick_bottom: false,
        })
        .child(page(s, theme, data, instant))
}

fn page(s: &MainState, theme: &Theme, data: &Data, instant: bool) -> El {
    col()
        .key("insights-page")
        .max_w(880.0)
        .w_full()
        .self_center()
        .px(32.0)
        .py(32.0)
        .gap(32.0)
        .child(tiles(s, theme, data))
        .child(columns(s, theme, data, instant))
        .child(wpm_card(theme, data.wpm))
        .child(footnote(theme))
}

fn tiles(s: &MainState, theme: &Theme, data: &Data) -> El {
    let mut row = row().key("insights-tiles").w_full().gap(12.0);
    for (i, tile) in data.tiles.iter().enumerate() {
        row = row.child(tile_card(s, theme, i, tile.label, &tile.value, &tile.sub));
    }
    row
}

fn tile_card(
    s: &MainState,
    theme: &Theme,
    index: usize,
    label: &str,
    value: &str,
    sub: &str,
) -> El {
    let c = &theme.c;
    let shown = &s.insights.tiles[index];
    let card = col()
        .key(format!("insights-tile-{index}"))
        .grow(1.0)
        .min_w(0.0)
        .gap(4.0)
        .padding(16.0)
        .radius(radius::XL)
        .fill(c.surface)
        .child(text(label.to_owned()).font(xs()).color(c.muted_foreground))
        .child(
            text(value.to_owned())
                .live_text(shown)
                .font(theme.ty.stat)
                .color(c.foreground),
        )
        .child(text(sub.to_owned()).font(theme.ty.meta_11).color(c.faint));
    theme
        .depth
        .card
        .iter()
        .fold(card, |el, shadow| el.box_shadow(*shadow))
}

fn columns(s: &MainState, theme: &Theme, data: &Data, instant: bool) -> El {
    row()
        .key("insights-columns")
        .w_full()
        .gap(24.0)
        .child(most_used(s, theme, data, instant).grow(1.4).min_w(0.0))
        .child(
            col()
                .grow(1.0)
                .min_w(0.0)
                .gap(24.0)
                .child(recent(theme, data))
                .child(unused(theme, data).unwrap_or_else(el)),
        )
}

fn most_used(s: &MainState, theme: &Theme, data: &Data, instant: bool) -> El {
    let c = &theme.c;
    let mut section = col()
        .key("insights-most-used")
        .role("region")
        .label("Most used")
        .gap(12.0)
        .child(
            text("Most used")
                .role("heading")
                .font(theme.ty.body_strong)
                .color(c.foreground),
        );
    if data.top.is_empty() {
        section = section.child(empty_most_used(s, theme, instant));
    } else {
        let mut list = col().key("insights-top-list").gap(2.0);
        for (i, row) in data.top.iter().enumerate() {
            list = list.child(top_row(s, theme, i, row, data.max, instant));
        }
        section = section.child(list);
    }
    section
}

fn empty_most_used(s: &MainState, theme: &Theme, instant: bool) -> El {
    let c = &theme.c;
    let mut bars = col().key("insights-sketch").w(160.0).gap(8.0).a11y_hidden();
    for (i, pct) in SKETCH.iter().enumerate() {
        bars = bars.child(bar_fill(
            s,
            format!("insights-sketch-bar-{i}"),
            i,
            *pct,
            instant,
            c.foreground.alpha(0.08),
        ));
    }
    col()
        .key("insights-most-used-empty")
        .gap(16.0)
        .padding(24.0)
        .center()
        .radius(radius::XL)
        .fill(c.foreground.alpha(0.03))
        .child(bars)
        .child(
            text("Expand a snippet and it'll show up here.")
                .font(theme.ty.body)
                .color(c.muted_foreground),
        )
}

fn top_row(
    s: &MainState,
    theme: &Theme,
    index: usize,
    item: &Ranked,
    max: i64,
    instant: bool,
) -> El {
    let c = &theme.c;
    let id = item.id;
    let rank = (index + 1).to_string();
    let pct = (item.count as f32 / max.max(1) as f32).clamp(BAR_MIN, 1.0);
    let mut name_line = row().gap(8.0).min_w(0.0).center_y().child(
        text(item.name.clone())
            .font(theme.ty.body)
            .color(c.foreground)
            .truncate()
            .a11y_hidden(),
    );
    if let Some(trigger) = item.trigger.as_deref() {
        name_line = name_line.child(trigger_chip(theme, trigger, true));
    }
    row()
        .key(format!("insights-top-{index}"))
        .role("button")
        .label(rank.clone())
        .w_full()
        .gap(12.0)
        .px(8.0)
        .py(6.0)
        .border(1.0, Color::TRANSPARENT)
        .border_layout()
        .center_y()
        .radius(radius::LG)
        .hover(c.foreground.alpha(0.04))
        .press_scale(motion::press::SCALE_RANKED_ROW)
        .on_press(move |st: &mut MainState| open(st, id))
        .child(
            text(rank)
                .w(18.0)
                .font(theme.ty.meta_11)
                .color(c.faint)
                .a11y_hidden(),
        )
        .child(
            col()
                .grow(1.0)
                .min_w(0.0)
                .gap(4.0)
                .child(name_line)
                .child(bar_track(s, theme, index, pct, instant)),
        )
        .child(
            text(format_count(item.count))
                .font(xs())
                .color(c.muted_foreground)
                .a11y_hidden(),
        )
}

fn bar_track(s: &MainState, theme: &Theme, index: usize, pct: f32, instant: bool) -> El {
    let c = &theme.c;
    el().key(format!("insights-bar-track-{index}"))
        .h(4.0)
        .w_full()
        .radius(2.0)
        .clip_rounded(2.0)
        .fill(c.foreground.alpha(0.05))
        .child(bar_fill(
            s,
            format!("insights-bar-{index}"),
            index,
            pct,
            instant,
            c.iris.alpha(0.8),
        ))
}

fn bar_fill(
    s: &MainState,
    key: String,
    index: usize,
    pct: f32,
    instant: bool,
    fill: rune::Color,
) -> El {
    let started = instant || s.insights.bar_started(index);
    let spring = if instant {
        Spring::NONE
    } else {
        motion::soft_spring(motion::BAR.duration)
    };
    let mut bar = el().key(key).h(4.0).radius(2.0).fill(fill);
    if started {
        bar = bar
            .w_frac(pct)
            .token_motion(spring)
            .motion_intent(MotionIntent::OVERSHOOT);
    } else {
        bar = bar.w(0.0);
    }
    bar
}

fn recent(theme: &Theme, data: &Data) -> El {
    let c = &theme.c;
    let mut list = col().key("insights-recent-list").gap(2.0);
    for (i, item) in data.recent.iter().enumerate() {
        let id = item.id;
        list = list.child(
            row()
                .key(format!("insights-recent-{i}"))
                .role("button")
                .label(item.name.clone())
                .w_full()
                .gap(8.0)
                .px(8.0)
                .py(6.0)
                .border(1.0, Color::TRANSPARENT)
                .border_layout()
                .center_y()
                .radius(radius::LG)
                .hover(c.foreground.alpha(0.04))
                .press_scale(motion::press::SCALE_RANKED_ROW)
                .on_press(move |st: &mut MainState| open(st, id))
                .child(
                    text(item.name.clone())
                        .grow(1.0)
                        .min_w(0.0)
                        .font(theme.ty.body)
                        .color(c.foreground)
                        .truncate()
                        .a11y_hidden(),
                )
                .child(
                    text(item.relative.clone())
                        .font(theme.ty.meta_11)
                        .color(c.faint)
                        .a11y_hidden(),
                ),
        );
    }
    if data.recent.is_empty() {
        list = list.child(
            text("Nothing yet.")
                .px(8.0)
                .font(xs())
                .color(c.muted_foreground),
        );
    }
    col()
        .key("insights-recent")
        .role("region")
        .label("Recently used")
        .gap(12.0)
        .child(
            text("Recently used")
                .role("heading")
                .font(theme.ty.body_strong)
                .color(c.foreground),
        )
        .child(list)
}

fn unused(theme: &Theme, data: &Data) -> Option<El> {
    if data.unused.is_empty() {
        return None;
    }
    let c = &theme.c;
    let count = data.unused_total;
    let mut chips = row().key("insights-unused-chips").flex_wrap().gap(6.0);
    for (i, chip) in data.unused.iter().enumerate() {
        let id = chip.id;
        let mut face = row()
            .key(format!("insights-unused-{i}"))
            .role("button")
            .label(chip.label.clone())
            .px(8.0)
            .py(4.0)
            .radius(radius::MD)
            .fill(c.surface_raised)
            .hover(c.foreground.alpha(0.06))
            .press_scale(motion::press::SCALE_RANKED_ROW)
            .on_press(move |st: &mut MainState| open(st, id))
            .child(
                text(chip.label.clone())
                    .font(xs())
                    .color(c.muted_foreground)
                    .a11y_hidden(),
            );
        for shadow in &theme.depth.raised {
            face = face.box_shadow(*shadow);
        }
        chips = chips.child(face);
    }
    Some(
        col()
            .key("insights-unused")
            .role("region")
            .label(format!("Never used {count}"))
            .gap(12.0)
            .child(
                row()
                    .gap(8.0)
                    .center_y()
                    .child(
                        text("Never used")
                            .role("heading")
                            .font(theme.ty.body_strong)
                            .color(c.foreground),
                    )
                    .child(text(count.to_string()).font(xs()).color(c.faint)),
            )
            .child(chips)
            .child(
                text("Forgotten triggers? Try the quick-search tray, or tidy these up.")
                    .font(theme.ty.meta_11)
                    .color(c.faint),
            ),
    )
}

fn wpm_card(theme: &Theme, wpm: u32) -> El {
    let c = &theme.c;
    let slider = Slider::new(wpm as f64, 10.0, 150.0)
        .step(5.0)
        .label("Typing speed in words per minute")
        .value_text(format!("{wpm} wpm"))
        .build(theme, |st: &mut MainState, v| {
            let _ = st.svc.settings.update(|settings| {
                settings.typing_speed_wpm = v as u32;
            });
        });
    let card = row()
        .key("insights-wpm")
        .w_full()
        .px(16.0)
        .py(12.0)
        .gap(24.0)
        .center_y()
        .justify_between()
        .radius(radius::XL)
        .fill(c.surface)
        .child(
            col()
                .gap(2.0)
                .child(
                    text("Your typing speed")
                        .font(theme.ty.body_medium)
                        .color(c.foreground),
                )
                .child(
                    text("Used to estimate time saved.")
                        .font(xs())
                        .color(c.muted_foreground),
                ),
        )
        .child(
            row().gap(12.0).center_y().child(slider).child(
                text(format!("{wpm} wpm"))
                    .w(64.0)
                    .font(theme.ty.body)
                    .color(c.foreground),
            ),
        );
    theme
        .depth
        .card
        .iter()
        .fold(card, |el, shadow| el.box_shadow(*shadow))
}

fn footnote(theme: &Theme) -> El {
    let c = &theme.c;
    row()
        .key("insights-footnote")
        .w_full()
        .gap(6.0)
        .center()
        .child(icon(&icons::LOCK, 12.0).color(c.faint))
        .child(
            text("Kudzu counts expansions only. What you type is never recorded or sent anywhere.")
                .font(theme.ty.meta_11)
                .color(c.faint),
        )
}

fn open(s: &mut MainState, id: Uuid) {
    s.navigate_to(MainView::Library(LibraryFilter::All));
    s.select_snippet(Some(id));
}

fn xs() -> TextStyle {
    TextStyle::new(12.0, 400.0).lh(16.0)
}
