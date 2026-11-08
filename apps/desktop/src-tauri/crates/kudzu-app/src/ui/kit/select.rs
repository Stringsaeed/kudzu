//! `components/ui/select.tsx` (Radix Select, `position="item-aligned"` as shadcn defaults it):
//! a raised trigger (`bg-surface-raised shadow-raised sheen`, `shadow-pressed` while open) and a
//! listbox placed so the chosen row sits over the trigger, its text over the trigger's text, kept
//! 10 pt inside the window. Item-aligned content doesn't animate (`data-[align-trigger=true]:animate-none`),
//! so it appears and goes at once; `.popper()` drops it below the trigger with the menu motion.
//!
//! Keys follow Radix: on the trigger Enter, Space, Up and Down open it on the chosen row, and
//! typing picks the next matching option without opening; in the list Up and Down move without
//! wrapping, Home and End jump, typing jumps, Enter and Space choose, Escape closes. Focus
//! returns to the trigger.
//!
//! ```ignore
//! // state: keep: SelectState
//! Select::new("keep-history", &s.settings_page.keep, |s: &mut MainUi| &mut s.settings_page.keep)
//!     .value(Some("7"))
//!     .option("1", "1 day").option("7", "1 week").option("custom", "Custom…")
//!     .width(128.0).label("Keep history for")
//!     .on_value_change(|s: &mut MainUi, v, _| ..)
//!     .build(cx, &mut layer)
//! ```

use std::rc::Rc;
use std::time::{Duration, Instant};

use rune::{Align, Color, Cx, El, Key, ScrollSpec, Side, Spring, Tx, col, el, icon, row, text};

use super::button::{hover_fast, invalid_colors, paint, Look, Skin, State};
use super::popover::{self, window_size};
use crate::ui::icons;
use crate::ui::theme::{radius, text as ty, Theme, Utilities};
use crate::ui::theme_for;

type Access<S> = Rc<dyn for<'a> Fn(&'a mut S) -> &'a mut SelectState>;
type Change<S> = Rc<dyn Fn(&mut S, &str, &mut Tx)>;

/// A select's open state and highlighted row, owned by the screen.
#[derive(Clone, Debug, Default)]
pub struct SelectState {
    pub open: bool,
    hl: Option<usize>,
    typed: String,
    typed_at: Option<Instant>,
}

impl SelectState {
    pub fn close(&mut self) {
        self.open = false;
        self.hl = None;
    }
    /// Opens on row `hl` (the chosen one, or the first).
    pub fn open_at(&mut self, hl: Option<usize>) {
        self.open = true;
        self.hl = hl;
    }
    fn typeahead(&mut self, t: &str) -> (String, bool) {
        let now = Instant::now();
        if self.typed_at.is_none_or(|at| now.duration_since(at) > Duration::from_secs(1)) {
            self.typed.clear();
        }
        self.typed_at = Some(now);
        self.typed.push_str(&t.to_lowercase());
        let first = self.typed.chars().next().unwrap_or(' ');
        let cycle = self.typed.chars().all(|c| c == first);
        (if cycle { first.to_string() } else { self.typed.clone() }, cycle)
    }
}

#[derive(Clone, Debug)]
enum Row {
    Item { value: String, label: String, disabled: bool },
    Label(String),
    Separator,
}

/// Heights of the rows: `py-1` items (28), `px-1.5 py-1 text-xs` labels (24), `my-1` separators (9).
fn row_h(r: &Row) -> f32 {
    match r {
        Row::Item { .. } => 28.0,
        Row::Label(_) => 24.0,
        Row::Separator => 9.0,
    }
}

/// `size`: `data-[size=default]:h-8`, `data-[size=sm]:h-7`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum SelectSize {
    #[default]
    Default,
    Sm,
}

pub struct Select<S> {
    key: String,
    state: SelectState,
    access: Access<S>,
    rows: Vec<Row>,
    value: Option<String>,
    placeholder: String,
    width: Option<f32>,
    size: SelectSize,
    disabled: bool,
    invalid: bool,
    popper: bool,
    align: Align,
    label: Option<String>,
    change: Option<Change<S>>,
}

impl<S: 'static> Select<S> {
    pub fn new(key: impl Into<String>, state: &SelectState, access: impl for<'a> Fn(&'a mut S) -> &'a mut SelectState + 'static) -> Self {
        Select {
            key: key.into(),
            state: state.clone(),
            access: Rc::new(access),
            rows: Vec::new(),
            value: None,
            placeholder: String::new(),
            width: None,
            size: SelectSize::Default,
            disabled: false,
            invalid: false,
            popper: false,
            align: Align::Center,
            label: None,
            change: None,
        }
    }
    /// The chosen value (`value`).
    pub fn value(mut self, v: Option<&str>) -> Self {
        self.value = v.map(str::to_string);
        self
    }
    /// `SelectValue placeholder`, shown `text-muted-foreground` when nothing is chosen.
    pub fn placeholder(mut self, p: impl Into<String>) -> Self {
        self.placeholder = p.into();
        self
    }
    /// `SelectItem value label`.
    pub fn option(mut self, value: impl Into<String>, label: impl Into<String>) -> Self {
        self.rows.push(Row::Item { value: value.into(), label: label.into(), disabled: false });
        self
    }
    pub fn option_disabled(mut self, value: impl Into<String>, label: impl Into<String>) -> Self {
        self.rows.push(Row::Item { value: value.into(), label: label.into(), disabled: true });
        self
    }
    /// `SelectLabel`.
    pub fn group_label(mut self, l: impl Into<String>) -> Self {
        self.rows.push(Row::Label(l.into()));
        self
    }
    /// `SelectSeparator`.
    pub fn separator(mut self) -> Self {
        self.rows.push(Row::Separator);
        self
    }
    /// The trigger's width (`className="w-32"`); otherwise it fits its value (`w-fit`).
    pub fn width(mut self, w: f32) -> Self {
        self.width = Some(w);
        self
    }
    pub fn size(mut self, s: SelectSize) -> Self {
        self.size = s;
        self
    }
    pub fn disabled(mut self, d: bool) -> Self {
        self.disabled = d;
        self
    }
    /// `aria-invalid`: `border-destructive ring-3 ring-destructive/20`.
    pub fn invalid(mut self, i: bool) -> Self {
        self.invalid = i;
        self
    }
    /// `position="popper"`: below the trigger, at least its width, with the menu motion.
    pub fn popper(mut self) -> Self {
        self.popper = true;
        self
    }
    /// `SelectContent align` (popper only).
    pub fn align(mut self, a: Align) -> Self {
        self.align = a;
        self
    }
    /// `aria-label`.
    pub fn label(mut self, l: impl Into<String>) -> Self {
        self.label = Some(l.into());
        self
    }
    /// `onValueChange`.
    pub fn on_value_change(mut self, f: impl Fn(&mut S, &str, &mut Tx) + 'static) -> Self {
        self.change = Some(Rc::new(f));
        self
    }

    fn chosen(&self) -> Option<usize> {
        let v = self.value.as_deref()?;
        self.rows.iter().position(|r| matches!(r, Row::Item { value, .. } if value == v))
    }

    pub fn build(self, cx: &mut Cx<S>, layer: &mut Vec<El>) -> El {
        let th = theme_for(cx);
        let t = &th.t;
        let anchor = format!("{}-trigger", self.key);
        let open = self.state.open && !self.disabled;
        let chosen = self.chosen();
        let rows = Rc::new(self.rows.clone());
        let nav: Rc<Vec<usize>> = Rc::new((0..rows.len()).filter(|&i| matches!(rows[i], Row::Item { disabled: false, .. })).collect());
        let label_of = |i: usize| match &rows[i] {
            Row::Item { label, .. } => label.clone(),
            _ => String::new(),
        };
        let shown = chosen.map(label_of);
        let (h, r) = match self.size {
            SelectSize::Default => (32.0, radius::LG),
            SelectSize::Sm => (28.0, radius::MD.min(10.0)),
        };
        let skin = Skin {
            radius: r,
            rim: 1.0,
            fill: t.surface_raised,
            hover_fill: None,
            shadows: if open { t.elev_pressed.clone() } else { t.elev_raised.clone() },
            hover_shadows: (!open).then(|| t.elev_raised_hover.clone()),
            pressed_shadows: None,
            sheen: Some(false),
            focus_border: Some(t.ring),
            focus_ring: Some(Theme::tint(t.ring, 0.5)),
            invalid: self.invalid.then(|| invalid_colors(th)),
            ..Skin::flat(r, t.surface_raised)
        };
        let mut trigger = hover_fast(row())
            .key(anchor.clone())
            .role("combobox")
            .label(self.label.clone().unwrap_or_else(|| shown.clone().unwrap_or(self.placeholder.clone())))
            .expanded(open)
            .h(h)
            .pl(11.0)
            .pr(9.0)
            .gap(6.0)
            .center_y()
            .justify_between()
            .radius(r);
        trigger = paint(trigger, &skin, Look::of(State::Rest, false, !self.disabled)).focusable(!self.disabled);
        if let Some(w) = self.width {
            trigger = trigger.w(w);
        }
        if let Some(v) = &shown {
            trigger = trigger.a11y_value(v.clone());
        }
        let value_el = match &shown {
            Some(v) => text(v.clone()).font(ty::SM).color(t.foreground),
            None => text(self.placeholder.clone()).font(ty::SM).color(t.muted_foreground),
        };
        trigger = trigger
            .child(value_el.key("value").truncate().shrink().a11y_hidden())
            .child(icon(&icons::CHEVRON_DOWN, 16.0).key("chevron").color(t.muted_foreground).a11y_hidden());
        if self.disabled {
            return trigger.layer_opacity(0.5).disabled(true);
        }
        // Press toggles; Enter, Space, Up and Down open on the chosen row; typing picks.
        let (a, b) = (self.access.clone(), self.access.clone());
        let first = nav.first().copied();
        let start = chosen.or(first);
        trigger = trigger.on_press(move |s: &mut S| {
            let st = a(s);
            if st.open { st.close() } else { st.open_at(start) }
        });
        {
            let (rows, nav, change) = (rows.clone(), nav.clone(), self.change.clone());
            trigger = trigger.on_key(move |s: &mut S, ev, tx| {
                if ev.mods.cmd || ev.mods.ctrl || ev.mods.alt {
                    return false;
                }
                let opens = matches!(ev.key, Key::Enter | Key::Up | Key::Down) || ev.key == Key::Text(" ".into());
                let st = b(s);
                if opens {
                    st.open_at(start);
                    return true;
                }
                if let Key::Text(c) = &ev.key
                    && !c.trim().is_empty()
                {
                    let (search, cycle) = st.typeahead(c);
                    let pos = chosen.and_then(|c| nav.iter().position(|&i| i == c));
                    if let Some(i) = find(&rows, &nav, pos, &search, cycle)
                        && let (Row::Item { value, .. }, Some(f)) = (&rows[i], &change)
                    {
                        f(s, value, tx);
                    }
                    return true;
                }
                false
            });
        }
        if !open {
            return trigger;
        }

        // The listbox.
        let hl = self.state.hl;
        let mut list = col().key(format!("{}-listbox", self.key)).role("listbox").items_stretch();
        for (i, row_) in rows.iter().enumerate() {
            list = list.child(match row_ {
                Row::Separator => el().h(1.0).mt(4.0).mb(4.0).ml(-4.0).mr(-4.0).fill(t.border).a11y_hidden(),
                Row::Label(l) => row().px(6.0).py(4.0).child(text(l.clone()).font(ty::XS).color(t.muted_foreground)),
                Row::Item { value, label, disabled } => {
                    let on = hl == Some(i);
                    let ink = if on { t.accent_foreground } else { t.popover_foreground };
                    let mut it = row()
                        .key(format!("{}-option-{i}", self.key))
                        .role("option")
                        .label(label.clone())
                        .h(28.0)
                        .pl(6.0)
                        .pr(32.0)
                        .gap(6.0)
                        .center_y()
                        .radius(radius::MD)
                        .fill(if on { t.accent } else { Color::TRANSPARENT })
                        .motion(Spring::NONE)
                        .checked(chosen == Some(i))
                        .child(text(label.clone()).font(ty::SM).color(ink).grow(1.0).shrink().truncate().motion(Spring::NONE).a11y_hidden())
                        .child(
                            el().pin(None, Some(6.0), Some(8.0), None).size(16.0, 16.0).center().a11y_hidden().child(
                                icon(&icons::CHECK, 16.0).color(ink).opacity(if chosen == Some(i) { 1.0 } else { 0.0 }).motion(Spring::NONE),
                            ),
                        );
                    if *disabled {
                        it = it.layer_opacity(0.5).disabled(true);
                    } else {
                        let (hov, pick, change, value) = (self.access.clone(), self.access.clone(), self.change.clone(), value.clone());
                        it = it
                            .on_hover_change(move |s: &mut S, inside, _| {
                                let st = hov(s);
                                if inside {
                                    st.hl = Some(i);
                                } else if st.hl == Some(i) {
                                    st.hl = None;
                                }
                            })
                            .on_press_tx(move |s: &mut S, tx| {
                                pick(s).close();
                                if let Some(f) = &change {
                                    f(s, &value, tx);
                                }
                            });
                    }
                    it
                }
            });
        }
        let total: f32 = rows.iter().map(row_h).sum();
        let (ww, wh) = window_size(cx);
        let trig = cx.layout(&anchor);
        let close = self.access.clone();
        let keys = {
            let (access, rows, nav, change, key) = (self.access.clone(), rows.clone(), nav.clone(), self.change.clone(), self.key.clone());
            move |s: &mut S, ev: &rune::KeyEvent, tx: &mut Tx| -> bool {
                if ev.mods.cmd || ev.mods.ctrl || ev.mods.alt {
                    return false;
                }
                let st = access(s);
                let pos = st.hl.and_then(|h| nav.iter().position(|&i| i == h));
                let go = |st: &mut SelectState, p: usize| st.hl = nav.get(p).copied();
                match &ev.key {
                    Key::Down => go(st, pos.map_or(0, |p| (p + 1).min(nav.len().saturating_sub(1)))),
                    Key::Up => go(st, pos.map_or(nav.len().saturating_sub(1), |p| p.saturating_sub(1))),
                    Key::Home => go(st, 0),
                    Key::End => go(st, nav.len().saturating_sub(1)),
                    Key::Escape => st.close(),
                    Key::Tab => {}
                    k if *k == Key::Enter || (*k == Key::Text(" ".into()) && st.typed.is_empty()) => {
                        let picked = st.hl;
                        st.close();
                        if let (Some(i), Some(f)) = (picked, &change)
                            && let Row::Item { value, .. } = &rows[i]
                        {
                            f(s, value, tx);
                        }
                    }
                    Key::Text(c) if !c.trim().is_empty() => {
                        let (search, cycle) = st.typeahead(c);
                        if let Some(i) = find(&rows, &nav, pos, &search, cycle) {
                            st.hl = Some(i);
                        }
                    }
                    _ => return false,
                }
                if let Some(h) = access(s).hl {
                    tx.scroll_into_view(format!("{key}-option-{h}"));
                }
                true
            }
        };
        let content = |extra: El| {
            col()
                .key(format!("{}-content", self.key))
                .radius(radius::LG)
                .fill(t.popover)
                .box_shadow(popover::ring(th))
                .elevate(&t.elev_float)
                .min_w(144.0)
                .focusable(true)
                .no_focus_ring()
                .focus_return(anchor.clone())
                .on_key(keys)
                .modal(true, move |s: &mut S| close(s).close())
                .child(col().radius(radius::LG).clip_rounded(radius::LG).min_h(0.0).shrink().child(
                    col().w_full().shrink().min_h(0.0).scroll(ScrollSpec { stick_bottom: false }).child(extra),
                ))
        };
        if self.popper {
            let w = trig.map_or(144.0, |r| r[2]);
            let c = content(list).min_w(w.max(144.0));
            layer.push(popover::mount(c, &anchor, Side::Bottom, self.align, 4.0));
            return trigger;
        }
        // Item-aligned: the chosen (or first) row's text over the trigger's, centred on it.
        let [tx0, ty0, tw, th_] = trig.unwrap_or([0.0, 0.0, 128.0, h]);
        let at = chosen.or(first).unwrap_or(0);
        let above: f32 = rows[..at].iter().map(row_h).sum();
        let width = 144.0f32.max(tw);
        let margin = 10.0;
        let height = total.min(wh - 2.0 * margin);
        let x = (tx0 + 11.0 - 6.0).clamp(margin, (ww - margin - width).max(margin));
        let y = (ty0 + th_ / 2.0 - above - 14.0).clamp(margin, (wh - margin - height).max(margin));
        let c = content(list).absolute(x.round(), y.round()).w(width).max_h(height).transition(rune::Transition::none());
        layer.push(c);
        trigger
    }
}

/// Radix's typeahead over the enabled rows: the next row whose label starts with `search`,
/// cycling from after `pos` for a repeated letter.
fn find(rows: &[Row], nav: &[usize], pos: Option<usize>, search: &str, cycle: bool) -> Option<usize> {
    if nav.is_empty() {
        return None;
    }
    let start = pos.map_or(0, |p| if cycle { p + 1 } else { p });
    (0..nav.len()).map(|k| nav[(start + k) % nav.len()]).find(|&i| match &rows[i] {
        Row::Item { label, .. } => label.to_lowercase().starts_with(search),
        _ => false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typeahead_finds_and_cycles() {
        let rows = vec![
            Row::Item { value: "1".into(), label: "1 day".into(), disabled: false },
            Row::Item { value: "7".into(), label: "1 week".into(), disabled: false },
            Row::Item { value: "30".into(), label: "1 month".into(), disabled: false },
            Row::Item { value: "f".into(), label: "Forever".into(), disabled: false },
        ];
        let nav = vec![0, 1, 2, 3];
        assert_eq!(find(&rows, &nav, None, "f", true), Some(3));
        assert_eq!(find(&rows, &nav, Some(0), "1", true), Some(1));
        assert_eq!(find(&rows, &nav, Some(0), "1 m", false), Some(2));
    }

    #[test]
    fn row_heights_match_the_css() {
        assert_eq!(row_h(&Row::Separator), 9.0);
        assert_eq!(row_h(&Row::Label(String::new())), 24.0);
    }

}
