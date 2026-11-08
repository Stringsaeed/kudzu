//! Pieces the library chrome (toolbar, sidebar, list, ⌘K, sheets) shares: the in-app drag that
//! moves snippets and groups onto the sidebar, the `animate-rise` / `animate-tick` keyframes,
//! a filled star, a mount hook standing in for React effects, and the file that remembers the
//! sidebar's folded sections.

pub mod drag;

use std::collections::BTreeSet;

use rune::{el, text, Beat, Color, El, Icon, Sequence, SpringPair, TextStyle, Transition, Tx};

use crate::ui::kit::button::{button_skin, hover_fast, ink, is_pressed, paint, track_press, Look, State, Variant, SPRING_SOFT};
use crate::ui::theme::{motion, radius, Theme};

/// `animate-rise`: fade in from 4 pt below on `--motion-slow` `--spring-soft`. React unmounts at
/// once, so nothing animates out.
pub fn rise() -> Transition {
    let enter = Transition::offset(0.0, 4.0).combined(Transition::opacity()).springs(SpringPair::new(motion::soft(), motion::soft()));
    Transition::asymmetric(enter, Transition::none())
}

/// `tick` (scale 1.3 in `--iris-foreground` → 1 in its own colour, `--motion-slow` `--spring-soft`).
const TICK_SCALE: &[Beat] = &[Beat::new(0.0).scale(1.3), Beat::new(crate::ui::theme::MOTION_SLOW as f64).easing(SPRING_SOFT)];
const TICK_TINT: &[Beat] = &[Beat::new(0.0), Beat::new(crate::ui::theme::MOTION_SLOW as f64).opacity(0.0).easing(SPRING_SOFT)];

/// `<span key={count} className="… animate-tick">{count}</span>`: a count that pops each time it
/// changes. `key` scopes it (the sequence replays when the count, and so its key, changes).
///
/// It pops when the count changes, not when it first appears, so a window's first frame (and a
/// headless snapshot of it) shows counts at rest.
pub fn tick(th: &Theme, key: &str, value: impl std::fmt::Display, font: TextStyle, color: Color) -> El {
    let label = value.to_string();
    let changed = SEEN.with(|seen| {
        let mut seen = seen.borrow_mut();
        let prev = seen.insert(key.to_string(), label.clone());
        prev.is_some_and(|p| p != label)
    });
    let e = el().key(format!("{key}-{label}")).child(text(label.clone()).font(font).color(color));
    if !changed && !REPLAY.with(|r| r.borrow().contains(&format!("{key}-{label}"))) {
        return e;
    }
    REPLAY.with(|r| {
        r.borrow_mut().insert(format!("{key}-{label}"));
    });
    e.sequence(Sequence::new(TICK_SCALE))
        .child(el().pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0)).sequence(Sequence::new(TICK_TINT)).a11y_hidden().child(text(label).font(font).color(th.t.iris_foreground)))
}

thread_local! {
    /// The last count each tick showed.
    static SEEN: std::cell::RefCell<std::collections::HashMap<String, String>> = Default::default();
    /// Counts that changed: they keep their sequence while shown (rebuilding without it would
    /// cut the pop short).
    static REPLAY: std::cell::RefCell<std::collections::HashSet<String>> = Default::default();
}

/// `font-variant-numeric: tabular-nums` (`tabular` in globals.css).
pub fn tabular(f: TextStyle) -> TextStyle {
    TextStyle { features: "'tnum' 1", ..f }
}

/// Runs `f` once after the element mounts: the stand-in for a React effect that fixes state up
/// after a render (keep a valid selection, reveal the selected row). Give each condition its
/// own `key`; while the key stays, it doesn't run again.
pub fn on_mount<S: 'static>(key: impl Into<String>, f: impl Fn(&mut S, &mut Tx) + 'static) -> El {
    el().key(key.into()).absolute(0.0, 0.0).size(0.0, 0.0).a11y_hidden().on_layout_tx(move |s: &mut S, _, tx| f(s, tx))
}

/// lucide `star` with `fill-favorite text-favorite`: the outline and its fill.
pub static STAR_FILLED: Icon = Icon {
    name: "kudzu-star-filled",
    view: 24.0,
    body: r#"<g fill="currentColor" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M11.525 2.295a.53.53 0 0 1 .95 0l2.31 4.679a2.123 2.123 0 0 0 1.595 1.16l5.166.756a.53.53 0 0 1 .294.904l-3.736 3.638a2.123 2.123 0 0 0-.611 1.878l.882 5.14a.53.53 0 0 1-.771.56l-4.618-2.428a2.122 2.122 0 0 0-1.973 0L6.396 21.01a.53.53 0 0 1-.77-.56l.881-5.139a2.122 2.122 0 0 0-.611-1.879L2.16 9.795a.53.53 0 0 1 .294-.906l5.165-.755a2.122 2.122 0 0 0 1.597-1.16z"/></g>"#,
};

/// A ghost `icon-sm` (28 pt) or `icon` (32 pt) button whose glyph can turn (the "+" buttons'
/// `hover:rotate-90` on `--motion-slow` `ease-pop`). `turned` comes from the screen's hover state.
pub fn ghost_icon(th: &Theme, id: &str, glyph: &'static Icon, label: &str, side: f32, color: Color, turned: bool) -> El {
    let r = if side < 32.0 { radius::MD } else { radius::LG };
    let look = Look::of(State::Rest, is_pressed(id), true);
    let (skin, _, hover) = button_skin(th, Variant::Ghost, r, false);
    let glyph = rune::icon(glyph, 16.0)
        .icon_rotation(if turned { std::f32::consts::FRAC_PI_2 } else { 0.0 })
        .motion(motion::pop())
        .a11y_hidden();
    let e = paint(hover_fast(rune::row()), &skin, look)
        .key(id.to_string())
        .role("button")
        .label(label.to_string())
        .size(side, side)
        .center()
        .radius(r)
        .child(ink(glyph, color, hover, look))
        .focusable(true)
        .press_scale(0.97);
    track_press(e, id.to_string())
}

// MARK: Remembered sections

/// localStorage's `kudzu-sidebar-sections`, as a file beside the library on this Mac.
fn sections_file() -> std::path::PathBuf {
    crate::backend::identity::data_dir().join("sidebar-sections.json")
}

/// The folded feature sections (`"snippets"`, `"clipboard"`).
pub fn load_sections() -> BTreeSet<String> {
    std::fs::read(sections_file()).ok().and_then(|b| serde_json::from_slice::<Vec<String>>(&b).ok()).map(|v| v.into_iter().collect()).unwrap_or_default()
}

pub fn save_sections(closed: &BTreeSet<String>) {
    let list: Vec<&String> = closed.iter().collect();
    if let Ok(json) = serde_json::to_vec(&list) {
        let path = sections_file();
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(path, json);
    }
}
