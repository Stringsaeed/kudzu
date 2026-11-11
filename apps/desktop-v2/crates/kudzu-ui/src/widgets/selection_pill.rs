//! One background for a group's selected option, sliding from option to option
//! (`components/app/selection-pill.tsx`), and the segmented control built on it
//! (`components/app/segmented.tsx`).
//!
//! As in the Tauri app, the pill is one element placed under the selected option: the group
//! reads the option's laid-out rect (`Cx::layout`, which re-runs the view when it moves) and
//! places the pill there, under the labels (`paint_order(-1)`), so a new selection springs it
//! across on the soft spring at `SLOW` (`data-slide:duration-(--motion-slow) ease-spring`).
//! Before the options have a layout, and with nothing selected, there is no pill: it fades in
//! where it belongs on its first frame and out over `FAST` when the selection clears, instead of
//! sliding. (A `matched` flight was tried first: it draws above the whole window, so the moving
//! pill covered the labels; the motion gate caught it.)

use rune::{el, row, text, Cx, El, Key, SpringPair, TextStyle, Transition};

use crate::theme::{motion, palette::fixed, radius, Theme};

/// How the pill looks: the segmented control's raised chip, or a caller's own fill and depth
/// (the sidebar's `selected` rows).
#[derive(Clone, Debug)]
pub struct PillLook {
    pub fill: rune::Color,
    pub radius: f32,
    pub depth: Vec<rune::Shadow>,
    pub sheen: bool,
}

impl PillLook {
    /// `rounded-md bg-surface-raised shadow-soft sheen`.
    pub fn raised(theme: &Theme) -> PillLook {
        PillLook {
            fill: theme.c.surface_raised,
            radius: radius::MD,
            depth: theme.depth.soft.clone(),
            sheen: true,
        }
    }
}

/// The pill at `rect` (in its group's coordinates), to add as a child of the group.
pub fn pill(rect: [f32; 4], look: &PillLook) -> El {
    let mut p = el()
        .key("pill")
        .absolute(rect[0], rect[1])
        .size(rect[2], rect[3])
        .radius(look.radius)
        .fill(look.fill)
        .paint_order(-1)
        .token_motion(motion::SOFT)
        .transition(Transition::opacity().springs(SpringPair::new(
            motion::soft_spring(motion::FAST),
            motion::soft_spring(motion::FAST),
        )))
        .a11y_hidden();
    if look.sheen {
        let [(_, top), (_, bottom)] = fixed::SHEEN;
        p = p.gradient(top.color(), bottom.color());
    }
    look.depth.iter().fold(p, |p, s| p.box_shadow(*s))
}

/// One option of a [`Segmented`] control.
#[derive(Clone, Debug, PartialEq)]
pub struct Segment {
    pub value: String,
    pub label: String,
    /// The option's tooltip (`title`).
    pub description: Option<String>,
    /// The option's element key instead of `option-<value>` (galleries key parity samples).
    pub sample: Option<String>,
}

impl Segment {
    pub fn new(value: impl Into<String>, label: impl Into<String>) -> Segment {
        Segment {
            value: value.into(),
            label: label.into(),
            description: None,
            sample: None,
        }
    }
    pub fn description(mut self, d: impl Into<String>) -> Self {
        self.description = Some(d.into());
        self
    }
    pub fn sample(mut self, key: impl Into<String>) -> Self {
        self.sample = Some(key.into());
        self
    }
    fn key(&self, group: &str) -> String {
        self.sample
            .clone()
            .unwrap_or_else(|| format!("{group}:{}", self.value))
    }
}

/// `md` options are 28 tall in 12 point type, `sm` 24 in 11.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SegmentSize {
    #[default]
    Md,
    Sm,
}

/// The option after `current` in direction `d` (±1), wrapping.
pub fn step(count: usize, current: usize, d: i32) -> usize {
    let n = count as i32;
    ((current as i32 + d).rem_euclid(n.max(1))) as usize
}

/// Arrow keys move between options: right and down forward, left and up back.
fn direction(key: &Key) -> i32 {
    match key {
        Key::Right | Key::Down => 1,
        Key::Left | Key::Up => -1,
        _ => 0,
    }
}

/// A radio group drawn as a segmented control. `key` names the group (its pill's matched id).
pub struct Segmented {
    key: String,
    label: String,
    options: Vec<Segment>,
    value: String,
    size: SegmentSize,
}

impl Segmented {
    pub fn new(key: impl Into<String>, label: impl Into<String>, value: impl Into<String>) -> Self {
        Segmented {
            key: key.into(),
            label: label.into(),
            options: Vec::new(),
            value: value.into(),
            size: SegmentSize::Md,
        }
    }
    pub fn option(mut self, s: Segment) -> Self {
        self.options.push(s);
        self
    }
    pub fn size(mut self, s: SegmentSize) -> Self {
        self.size = s;
        self
    }

    /// `on_change` receives the chosen option's value. `cx` places the pill from the last
    /// layout.
    pub fn build<S: 'static, T>(
        self,
        theme: &Theme,
        cx: &Cx<T>,
        on_change: impl Fn(&mut S, &str) + Clone + 'static,
    ) -> El {
        let c = &theme.c;
        let (h, size) = match self.size {
            SegmentSize::Md => (28.0, 12.0),
            SegmentSize::Sm => (24.0, 11.0),
        };
        let font = TextStyle::new(size, 500.0).lh(size + 4.0);
        let selected = self.options.iter().position(|o| o.value == self.value);
        let focus_at = selected.unwrap_or(0);
        let look = PillLook::raised(theme);
        let values: Vec<String> = self.options.iter().map(|o| o.value.clone()).collect();
        let keys: Vec<String> = self.options.iter().map(|o| o.key(&self.key)).collect();
        // The selected option in the group's coordinates, once both have a layout.
        let placed = selected.and_then(|i| {
            let (g, o) = (cx.layout(&self.key)?, cx.layout(&keys[i])?);
            Some([o[0] - g[0], o[1] - g[1], o[2], o[3]])
        });
        let mut group = row()
            .key(self.key.clone())
            .role("radiogroup")
            .label(self.label)
            .padding(2.0)
            .radius(radius::LG)
            .fill(c.foreground.alpha(0.04));
        for s in &theme.depth.well {
            group = group.box_shadow(*s);
        }
        for (i, o) in self.options.into_iter().enumerate() {
            let on = Some(i) == selected;
            let (pick, value) = (on_change.clone(), o.value.clone());
            let (change, values, keys) = (on_change.clone(), values.clone(), keys.clone());
            let mut b = row()
                .key(keys[i].clone())
                .role("radio")
                .label(o.label.clone())
                .checked(on)
                .focusable(true)
                .tab_index(if i == focus_at { 0 } else { -1 })
                .h(h)
                .px(10.0)
                .center()
                .radius(radius::MD)
                .press_scale(motion::press::SCALE)
                .on_press(move |s: &mut S| pick(s, &value))
                .on_key(move |s: &mut S, e, tx| {
                    let d = direction(&e.key);
                    if d == 0 {
                        return false;
                    }
                    let n = step(values.len(), i, d);
                    change(s, &values[n]);
                    tx.focus(keys[n].clone(), true);
                    true
                });
            if let Some(d) = o.description {
                b = b.help(d);
            }
            let ink = if on { c.foreground } else { c.muted_foreground };
            let mut label = text(o.label).font(font).color(ink).a11y_hidden();
            if !on {
                label = label.group_color(c.foreground);
                b = b.hover_group();
            }
            group = group.child(b.child(label));
        }
        match placed {
            Some(rect) => group.child(pill(rect, &look)),
            None => group,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arrows_wrap_around_the_options() {
        assert_eq!(step(3, 0, -1), 2);
        assert_eq!(step(3, 2, 1), 0);
        assert_eq!(step(3, 1, 1), 2);
        assert_eq!(direction(&Key::Down), 1);
        assert_eq!(direction(&Key::Up), -1);
    }
}
