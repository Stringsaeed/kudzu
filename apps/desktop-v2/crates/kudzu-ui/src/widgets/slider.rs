//! RUNE-GAP(#149: rune-kit has no Slider): delete this module for `rune_kit::Slider` once it
//! ships one with Kudzu's metrics.
//!
//! The Insights typing-speed slider (`<input type="range" class="w-48 accent-[var(--iris)]">`),
//! drawn as shadcn's `Slider` (`components/ui/slider.tsx`): a 4 point capsule track in `muted`
//! with the `well` depth, the range in the accent (`iris`), and a 12 point white thumb with a
//! `ring` border and the `knob` depth that grows a 3 point `ring/50` halo on hover, focus and
//! drag. It is Rune core's slider role: [`El::range_value`] and [`El::on_step`] give the value,
//! the arrow keys (one step), Home and End; a press or drag on the track sets the value under
//! the pointer.

use rune::{el, Color, El, Phase, Shadow};

use crate::theme::{motion, Theme};

/// The thumb's side (`size-3`).
const THUMB: f32 = 12.0;
/// The track's height (`h-1`).
const TRACK: f32 = 4.0;

/// `value` moved by `steps` steps, clamped to `min..=max` and snapped to the step grid.
pub fn stepped(value: f64, steps: i32, min: f64, max: f64, step: f64) -> f64 {
    snap(value + steps as f64 * step, min, max, step)
}

/// The nearest value on the step grid from `min`, clamped.
pub fn snap(value: f64, min: f64, max: f64, step: f64) -> f64 {
    let v = if step > 0.0 {
        min + ((value - min) / step).round() * step
    } else {
        value
    };
    v.clamp(min, max)
}

/// The value under a point `x` points along a track `width` wide (the thumb's centre travels
/// from half a thumb in to half a thumb from the end).
pub fn at(x: f32, width: f32, min: f64, max: f64, step: f64) -> f64 {
    let span = (width - THUMB).max(1.0);
    let t = ((x - THUMB / 2.0) / span).clamp(0.0, 1.0) as f64;
    snap(min + t * (max - min), min, max, step)
}

pub struct Slider {
    value: f64,
    min: f64,
    max: f64,
    step: f64,
    width: f32,
    label: String,
    value_text: Option<String>,
    active: bool,
}

impl Slider {
    pub fn new(value: f64, min: f64, max: f64) -> Slider {
        Slider {
            value,
            min,
            max,
            step: 1.0,
            width: 192.0,
            label: String::new(),
            value_text: None,
            active: false,
        }
    }
    pub fn step(mut self, s: f64) -> Self {
        self.step = s;
        self
    }
    /// The track's width (`w-48` by default).
    pub fn width(mut self, w: f32) -> Self {
        self.width = w;
        self
    }
    /// The accessible name (`aria-label`).
    pub fn label(mut self, l: impl Into<String>) -> Self {
        self.label = l.into();
        self
    }
    /// What a reader hears for the value ("90 wpm"); the number by default.
    pub fn value_text(mut self, t: impl Into<String>) -> Self {
        self.value_text = Some(t.into());
        self
    }
    /// Draws the hover, focus or drag halo at rest (galleries).
    pub fn active(mut self, on: bool) -> Self {
        self.active = on;
        self
    }

    /// `on_change` receives the new value.
    pub fn build<S: 'static>(
        self,
        theme: &Theme,
        on_change: impl Fn(&mut S, f64) + Clone + 'static,
    ) -> El {
        let c = &theme.c;
        let (min, max, step, w) = (self.min, self.max, self.step, self.width);
        let value = snap(self.value, min, max, step);
        let t = if max > min {
            ((value - min) / (max - min)) as f32
        } else {
            0.0
        };
        let x = t * (w - THUMB);
        let mut track = el()
            .key("track")
            .absolute(0.0, (THUMB - TRACK) / 2.0)
            .size(w, TRACK)
            .radius(TRACK / 2.0)
            .fill(c.muted)
            .child(
                el().key("range")
                    .size(x + THUMB / 2.0, TRACK)
                    .radius(TRACK / 2.0)
                    .fill(c.iris)
                    .token_motion(motion::soft_spring(motion::FAST)),
            );
        for s in &theme.depth.well {
            track = track.box_shadow(*s);
        }
        let halo = Shadow {
            spread: 3.0,
            color: c.ring.alpha(0.5),
            ..Shadow::default()
        };
        let mut thumb = el()
            .key("thumb")
            .absolute(x, 0.0)
            .size(THUMB, THUMB)
            .radius(THUMB / 2.0)
            .fill(Color::WHITE)
            .border(1.0, c.ring)
            .token_motion(motion::soft_spring(motion::FAST));
        for s in &theme.depth.knob {
            thumb = thumb.box_shadow(*s);
        }
        if self.active {
            thumb = thumb.box_shadow(halo);
        }
        let (keys, limits, drag) = (on_change.clone(), on_change.clone(), on_change);
        let mut e = el()
            .key("slider")
            .label(self.label)
            .range_value(value, min, max)
            .range_step(step)
            .size(w, THUMB)
            .focusable(true)
            .focus_ring(3.0, 0.0, c.ring.alpha(0.5))
            .on_step(move |s: &mut S, d, _| keys(s, stepped(value, d, min, max, step)))
            .on_step_limit(move |s: &mut S, end, _| limits(s, if end { max } else { min }))
            .on_drag(move |s: &mut S, d, tx| {
                if !d.cancelled() && d.phase != Phase::Up {
                    drag(s, at(d.x, w, min, max, step));
                    tx.instant();
                }
            })
            .child(track)
            .child(thumb);
        if let Some(v) = self.value_text {
            e = e.a11y_value(v);
        }
        e
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rune::{Cx, Env, Key, KeyEvent, Mods, Runtime};

    #[test]
    fn steps_snap_and_clamp() {
        assert_eq!(stepped(90.0, 1, 10.0, 150.0, 5.0), 95.0);
        assert_eq!(stepped(150.0, 1, 10.0, 150.0, 5.0), 150.0);
        assert_eq!(stepped(12.0, -1, 10.0, 150.0, 5.0), 10.0);
        assert_eq!(snap(93.0, 10.0, 150.0, 5.0), 95.0);
    }

    #[test]
    fn the_pointer_picks_the_value_under_it() {
        assert_eq!(at(0.0, 192.0, 10.0, 150.0, 5.0), 10.0);
        assert_eq!(at(192.0, 192.0, 10.0, 150.0, 5.0), 150.0);
        assert_eq!(at(96.0, 192.0, 10.0, 150.0, 5.0), 80.0);
    }

    fn view(wpm: &f64, _: &mut Cx<f64>) -> El {
        rune::row().child(
            Slider::new(*wpm, 10.0, 150.0)
                .step(5.0)
                .label("Typing speed in words per minute")
                .value_text(format!("{wpm} wpm"))
                .build(Theme::light(), |s: &mut f64, v| *s = v),
        )
    }

    fn press(rt: &mut Runtime<f64>, key: Key) {
        rt.key(KeyEvent {
            key,
            mods: Mods::default(),
        });
        rt.frame(1.0 / 60.0);
    }

    #[test]
    fn the_tree_reads_a_slider_and_the_keys_step_it() {
        let mut rt = Runtime::new(90.0, view, (300.0, 40.0), Env::new(true, 1.0));
        rt.frame(0.0);
        let t = rt.accessibility_json().to_string();
        assert!(t.contains("\"slider\"") || t.contains("Slider"), "{t}");
        assert!(
            t.contains("Typing speed in words per minute") && t.contains("90 wpm"),
            "{t}"
        );
        rt.update(|_, tx| tx.focus("slider", true));
        rt.frame(1.0 / 60.0);
        press(&mut rt, Key::Right);
        assert_eq!(rt.state, 95.0);
        press(&mut rt, Key::Left);
        press(&mut rt, Key::Left);
        assert_eq!(rt.state, 85.0);
        press(&mut rt, Key::End);
        assert_eq!(rt.state, 150.0);
        press(&mut rt, Key::Home);
        assert_eq!(rt.state, 10.0);
    }
}
