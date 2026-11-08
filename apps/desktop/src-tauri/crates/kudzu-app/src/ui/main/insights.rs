//! Insights (`components/app/main/insights.tsx`): headline totals, the most-used ranking with its
//! growing bars, recently used and never-used snippets, and the typing speed that prices time
//! saved.
//!
//! ```text
//! ┌ h-12 header ───────────────────────────────────────────────┐
//! │ max-w 880, px/py 32, gap 32, .stagger                       │
//! │ [tile] [tile] [tile] [tile]                                 │
//! │ Most used (1.4fr)            │ Recently used / Never used   │
//! │ [typing speed card]                                          │
//! │ 🔒 privacy line                                              │
//! └─────────────────────────────────────────────────────────────┘
//! ```

use rune::{col, icon, row, text, Align, Cx, Drag, Edge, El, ScrollSpec, Spring, SpringPair, Transition, Tx};
use uuid::Uuid;

use kudzu_core::client::stats::{self, SnippetUsage, CHARS_PER_WORD};
use kudzu_core::SnippetRecord;

use crate::ui::icons;
use crate::ui::kit::button::{hover_fast, ink, is_pressed, paint, track_press, Look, Skin, State};
use crate::ui::kit::item::Item;
use crate::ui::kit::trigger_chip::trigger_chip;
use crate::ui::main::{MainUi, View};
use crate::ui::theme::{motion, radius, text as ty, weight, Theme, Utilities, WeightOf};
use crate::ui::theme_for;

/// `.stagger > *`: each direct child rises (4 pt, fading in) on `--motion-slow` `--spring-soft`,
/// 30 ms after the one before, capped at 150 ms. Nothing animates on the way out.
pub fn rise() -> Transition {
    Transition::offset(0.0, 4.0).combined(Transition::opacity()).springs(SpringPair::new(motion::soft(), Spring::NONE))
}

pub fn stagger_delay(i: usize) -> f32 {
    (i.min(5) as f32) * 0.03
}

/// No entrance of its own. Rune fades every element new to a column in, and nested fades
/// multiply; CSS only animates what declares an animation, so the rest of a page shows whole.
pub fn still(e: El) -> El {
    e.transition(Transition::none())
}

/// [`still`], and everything inside enters whole too (`instant_changes` is inherited).
pub fn whole(e: El) -> El {
    still(e).instant_changes()
}

/// Applies `.stagger` to a column's children.
pub fn stagger(children: Vec<El>) -> Vec<El> {
    children.into_iter().enumerate().map(|(i, c)| c.transition(rise()).enter_delay(stagger_delay(i))).collect()
}

/// `animate-bar` (`bar-grow`, `scale: 0 1 → 1 1` from the left, 480 ms `--spring-soft`): a wipe
/// from the left edge, which is what a left-anchored horizontal scale shows on a bar this thin.
fn bar_grow() -> Transition {
    let mut t = Transition::reveal(Edge::Trailing).springs(SpringPair::new(Spring::new(0.48, 0.05), Spring::NONE));
    if let Some(r) = t.insertion.reveal.as_mut() {
        r.feather = 1.0;
    }
    if let Some(r) = t.removal.reveal.as_mut() {
        r.feather = 1.0;
    }
    t
}

const TABULAR: &str = "'tnum' 1";
/// `text-[11px]` under the body's `leading-5`: the size changes, the 20 pt line stays.
const XS2_BODY: rune::TextStyle = ty::XS2.lh(20.0);
/// Inside an Item (`text-sm`, a unitless 1.25/0.875 line height), `text-[13px]` and
/// `text-[11px]` keep that ratio.
const ITEM_RATIO: f32 = 1.25 / 0.875;
const ITEM_13: rune::TextStyle = ty::BASE.lh(13.0 * ITEM_RATIO);
const ITEM_11: rune::TextStyle = ty::XS2.lh(11.0 * ITEM_RATIO);

/// `setView({ kind: "all" }); select(id)`.
fn open(s: &mut MainUi, id: Uuid) {
    s.set_view(View::All);
    s.selected = Some(id);
}

fn tile(th: &Theme, label: &str, value: String, sub: String) -> El {
    let t = &th.t;
    col()
        .instant_changes()
        .grow(1.0)
        .min_w(0.0)
        .gap(4.0)
        .radius(radius::XL)
        .fill(t.surface)
        .padding(16.0)
        .elevate(&t.elev_card)
        .child(text(label.to_string()).font(ty::XS).color(t.muted_foreground))
        .child(text(value).font(rune::TextStyle::new(26.0, weight::SEMIBOLD).lh(26.0).tracking(-0.52).features(TABULAR)).color(t.foreground))
        .child(text(sub).font(XS2_BODY).color(t.faint))
}

fn heading(th: &Theme, s: &str) -> El {
    text(s.to_string()).font(ty::BASE.weight_of(weight::SEMIBOLD)).color(th.t.foreground)
}

fn name_of(s: &SnippetRecord) -> String {
    if s.name.is_empty() { "Untitled".into() } else { s.name.clone() }
}

fn most_used(th: &Theme, top: &[(&SnippetRecord, SnippetUsage)]) -> El {
    let t = &th.t;
    let mut sec = col().key("ins-top").grow(1.4).min_w(0.0).gap(12.0).role("region").label("Most used").child(still(heading(th, "Most used")));
    if top.is_empty() {
        return sec.child(whole(
            el_center(text("Expand a snippet and it\u{2019}ll show up here.").font(ty::SM).color(t.muted_foreground).align(Align::Center).wrap())
                .radius(radius::XL)
                .fill(Theme::tint(t.foreground, 0.03))
                .padding(24.0),
        ));
    }
    let max = top[0].1.count.max(1) as f32;
    let mut list = col().gap(2.0).role("list");
    for (i, (s, u)) in top.iter().enumerate() {
        let id = s.id;
        let pct = (u.count as f32 / max * 100.0).max(3.0) / 100.0;
        let mut title = row().instant_changes().transition(Transition::none()).min_w(0.0).center_y().gap(8.0).child(text(name_of(s)).font(ITEM_13).color(t.foreground).truncate().min_w(0.0).shrink());
        if let Some(trig) = s.triggers.first() {
            title = title.child(trigger_chip(th, trig, true));
        }
        let fill = rune::el()
            .key(format!("ins-bar-{id}"))
            .pin(Some(0.0), Some(0.0), None, Some(0.0))
            .w_frac(pct)
            .radius(radius::FULL)
            .fill(Theme::tint(t.iris, 0.8))
            .transition(bar_grow())
            .enter_delay(0.06 + i as f32 * 0.04);
        let bar = rune::el().transition(Transition::none()).w_full().h(4.0).radius(radius::FULL).clip_rounded(radius::FULL).fill(Theme::tint(t.foreground, 0.05)).child(fill);
        let item = Item::new()
            .hover_fill(Theme::tint(t.foreground, 0.04))
            .label(format!("{}, {} expansions", name_of(s), u.count))
            .child(still(text((i + 1).to_string()).font(ITEM_11.features(TABULAR)).color(t.faint).w(18.0)))
            .child(still(col().grow(1.0).min_w(0.0).gap(4.0).child(title).child(bar)))
            .child(still(text(stats::format_count(u.count)).font(ty::XS.features(TABULAR)).color(t.muted_foreground)))
            .on_press(move |st: &mut MainUi| open(st, id))
            .build(th)
            .key(format!("ins-top-{id}"))
            .px(9.0)
            .py(7.0)
            .gap(12.0);
        list = list.child(still(item));
    }
    sec = sec.child(still(list));
    sec
}

fn el_center(e: El) -> El {
    rune::el().w_full().center().child(e)
}

fn recently_used(th: &Theme, recent: &[(&SnippetRecord, SnippetUsage)]) -> El {
    let t = &th.t;
    let now = chrono::Utc::now().timestamp_millis();
    let mut list = col().gap(2.0).role("list");
    for (s, u) in recent {
        let id = s.id;
        list = list.child(
            Item::new()
                .hover_fill(Theme::tint(t.foreground, 0.04))
                .child(text(name_of(s)).font(ITEM_13).color(t.foreground).truncate().grow(1.0).min_w(0.0).shrink())
                .child(text(stats::relative_time(u.last, now, &chrono::Local)).font(ITEM_11).color(t.faint))
                .on_press(move |st: &mut MainUi| open(st, id))
                .build(th)
                .key(format!("ins-recent-{id}"))
                .px(9.0)
                .py(7.0)
                .gap(8.0),
        );
    }
    if recent.is_empty() {
        list = list.child(text("Nothing yet.").font(ty::XS).color(t.muted_foreground).px(8.0));
    }
    whole(col()).key("ins-recent").gap(12.0).role("region").label("Recently used").child(heading(th, "Recently used")).child(list)
}

/// The never-used chips: `rounded-md bg-surface-raised px-2 py-1 text-xs text-muted-foreground
/// shadow-raised press hover:text-foreground hover:shadow-raised-hover active:shadow-pressed`.
fn chip(th: &Theme, s: &SnippetRecord) -> El {
    let t = &th.t;
    let id = s.id;
    let key = format!("ins-unused-{id}");
    let skin = Skin {
        shadows: t.elev_raised.clone(),
        hover_shadows: Some(t.elev_raised_hover.clone()),
        pressed_shadows: Some(t.elev_pressed.clone()),
        focus_ring: Some(Theme::tint(t.ring, 0.5)),
        ..Skin::flat(radius::MD, t.surface_raised)
    };
    let look = Look::of(State::Rest, is_pressed(&key), true);
    let label = if s.name.is_empty() { s.triggers.first().cloned().unwrap_or_default() } else { s.name.clone() };
    let mut e = hover_fast(row()).key(key.clone()).role("button").label(label.clone()).px(8.0).py(4.0).center().radius(radius::MD);
    e = paint(e, &skin, look);
    e = e.child(ink(text(label).font(ty::XS), t.muted_foreground, Some(t.foreground), look).a11y_hidden());
    e = e.focusable(true).press_sink(0.97).on_press(move |st: &mut MainUi| open(st, id));
    track_press(e, key)
}

fn never_used(th: &Theme, unused: &[&SnippetRecord]) -> El {
    let t = &th.t;
    let head = row()
        .gap(8.0)
        .items_end()
        .child(heading(th, "Never used"))
        .child(text(unused.len().to_string()).font(ty::XS).color(t.faint).mb(2.0));
    let chips = row().flex_wrap().gap(6.0).children(unused.iter().take(18).map(|s| chip(th, s)));
    whole(col())
        .key("ins-unused")
        .gap(12.0)
        .role("region")
        .label("Never used")
        .child(head)
        .child(chips)
        .child(text("Forgotten triggers? Try the quick-search tray, or tidy these up.").font(XS2_BODY).color(t.faint).wrap())
}

/// `<input type="range" min=10 max=150 step=5 className="w-48 accent-[var(--iris)]">`, drawn as
/// Chrome draws an accent-coloured range on macOS: a 16 pt thumb in the accent over a rounded
/// track filled with the accent up to it.
fn wpm_range(th: &Theme, value: f32) -> El {
    const MIN: f32 = 10.0;
    const MAX: f32 = 150.0;
    const STEP: f32 = 5.0;
    const W: f32 = 192.0;
    const THUMB: f32 = 16.0;
    let t = &th.t;
    let p = ((value - MIN) / (MAX - MIN)).clamp(0.0, 1.0);
    let left = p * (W - THUMB);
    let snap = |v: f32| (MIN + ((v - MIN) / STEP).round() * STEP).clamp(MIN, MAX);
    let set = move |s: &mut MainUi, v: f32| {
        let v = snap(v);
        if (s.settings.typing_speed_wpm as f32 - v).abs() > f32::EPSILON {
            s.update_settings(serde_json::json!({ "typingSpeedWPM": v as i64 }));
        }
    };
    let track_bg = rune::oklch(0.951, 0.0, 0.0);
    let track = rune::el()
        .pin(Some(0.0), Some(4.5), Some(0.0), None)
        .h(7.0)
        .radius(radius::FULL)
        .fill(track_bg)
        .border(1.0, rune::oklch(0.763, 0.0, 0.0))
        .clip_rounded(radius::FULL)
        .child(rune::el().pin(Some(0.0), Some(0.0), None, Some(0.0)).w(left + THUMB / 2.0).fill(t.iris))
        .a11y_hidden();
    let thumb = rune::el().key("ins-wpm-thumb").absolute(left, 0.0).size(THUMB, THUMB).radius(radius::FULL).fill(t.iris).a11y_hidden();
    rune::el()
        .instant_changes()
        .key("ins-wpm")
        .role("slider")
        .label("Typing speed in words per minute")
        .size(W, THUMB)
        .range_value(value as f64, MIN as f64, MAX as f64)
        .range_step(STEP as f64)
        .child(track)
        .child(thumb)
        .focusable(true)
        .focus_ring(2.0, 2.0, t.ring)
        .on_drag(move |s: &mut MainUi, d: &mut Drag, _tx: &mut Tx| {
            if d.w > THUMB {
                set(s, MIN + ((d.x - THUMB / 2.0) / (d.w - THUMB)).clamp(0.0, 1.0) * (MAX - MIN));
            }
        })
        .on_step(move |s: &mut MainUi, n: i32, _tx: &mut Tx| {
            let cur = s.settings.typing_speed_wpm as f32;
            set(s, cur + n as f32 * STEP);
        })
        .on_step_limit(move |s: &mut MainUi, end: bool, _tx: &mut Tx| set(s, if end { MAX } else { MIN }))
}

fn typing_speed(th: &Theme, wpm: f32) -> El {
    let t = &th.t;
    row()
        .key("ins-speed")
        .center_y()
        .justify_between()
        .gap(24.0)
        .radius(radius::XL)
        .fill(t.surface)
        .px(16.0)
        .py(12.0)
        .elevate(&t.elev_card)
        .child(
            col()
                .instant_changes()
                .min_w(0.0)
                .shrink()
                .child(text("Your typing speed").font(ty::BASE.weight_of(weight::MEDIUM)).color(t.foreground))
                .child(text("Used to estimate time saved.").font(ty::XS).color(t.muted_foreground)),
        )
        .child(
            row()
                .center_y()
                .gap(12.0)
                .child(wpm_range(th, wpm))
                .child(rune::el().instant_changes().w(64.0).child(text(format!("{} wpm", wpm as i64)).font(ty::SM.features(TABULAR)).color(t.foreground).align(Align::End).w_full())),
        )
}

pub fn view(s: &MainUi, cx: &mut Cx<MainUi>) -> El {
    let th = theme_for(cx);
    let t = &th.t;
    let wpm = s.settings.typing_speed_wpm as f64;
    let totals = stats::totals(&s.library, &s.usage, wpm);

    let mut ranked: Vec<(&SnippetRecord, SnippetUsage)> =
        s.library.snippets.iter().filter_map(|x| s.usage.get(&x.id).filter(|u| u.count > 0).map(|u| (x, *u))).collect();
    ranked.sort_by_key(|x| std::cmp::Reverse(x.1.count));
    let top: Vec<_> = ranked.iter().take(10).cloned().collect();
    let unused: Vec<&SnippetRecord> =
        s.library.snippets.iter().filter(|x| x.is_enabled && s.usage.get(&x.id).is_none_or(|u| u.count == 0) && !x.triggers.is_empty()).collect();
    let mut recent = ranked.clone();
    recent.sort_by_key(|x| std::cmp::Reverse(x.1.last));
    recent.truncate(6);

    let tiles = row()
        .key("ins-tiles")
        .gap(12.0)
        .child(tile(th, "Expansions", stats::format_count(totals.expansions), format!("{} snippets used", totals.used_snippets)))
        .child(tile(
            th,
            "Characters saved",
            stats::format_count(totals.chars),
            format!("≈ {} words", stats::format_count((totals.chars as f64 / CHARS_PER_WORD).round() as i64)),
        ))
        .child(tile(th, "Time saved", stats::format_duration(totals.minutes_saved), format!("at {} words per minute", s.settings.typing_speed_wpm)))
        .child(tile(
            th,
            "Active snippets",
            totals.active_snippets.to_string(),
            format!("{} turned off", s.library.snippets.len() - totals.active_snippets),
        ));

    let mut side = col().grow(1.0).min_w(0.0).gap(24.0).child(recently_used(th, &recent));
    if !unused.is_empty() {
        side = side.child(never_used(th, &unused));
    }
    let lists = row().key("ins-lists").gap(24.0).items_start().child(most_used(th, &top)).child(side);

    let privacy = row()
        .key("ins-privacy")
        .center()
        .gap(6.0)
        .child(icon(&icons::LOCK, 12.0).color(t.faint).a11y_hidden())
        .child(text("Kudzu counts expansions only. What you type is never recorded or sent anywhere.").font(XS2_BODY).color(t.faint));

    let body = col()
        .key("ins-body")
        .w_full()
        .max_w(880.0)
        .gap(32.0)
        .px(32.0)
        .py(32.0)
        .children(stagger(vec![tiles, lists, typing_speed(th, wpm as f32), privacy]));

    let header = row()
        .key("ins-header")
        .h(48.0)
        .px(24.0)
        .center_y()
        .border_sides([0.0, 0.0, 1.0, 0.0], t.hairline)
        .window_drag()
        .child(text("Insights").font(ty::BASE.weight_of(weight::SEMIBOLD)).color(t.foreground).role("heading"));

    // The web swaps pages without a fade; only the `.stagger` children rise.
    col()
        .key("insights")
        .transition(Transition::none())
        .w_full()
        .h_full()
        .min_h(0.0)
        .child(whole(header))
        .child(
            // `overflow-y-auto`: macOS draws its overlay scroll bar only while scrolling.
            still(col().key("insights-scroll").w_full().grow(1.0).min_h(0.0).scroll(ScrollSpec { stick_bottom: false }).child(still(col().w_full().items_center().child(still(body))))),
        )
}
