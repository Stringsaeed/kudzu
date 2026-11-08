//! `components/ui/slider.tsx`: a single-thumb slider. Pressing the track jumps the thumb there,
//! dragging moves it, and the arrow keys, Page Up and Down (10 steps), Home and End step it. The
//! thumb rings (`ring-3 ring-ring/50`) on hover, while held and under keyboard focus, and its
//! click target reaches 8 pt past it (`after:-inset-2`).

// The kit's whole API is ported now; screens adopt it as they port.
#![allow(dead_code)]

use std::rc::Rc;

use rune::{Color, Cx, Drag, El, Spring, Tx};

use crate::ui::theme::{radius, Theme, Utilities};

use super::button::{is_pressed, layer, note_press, site_id, track_press, tween, State, TW_EASE};

/// Where a value comes from: a pointer position (0 to 1 along the track), a step count, or an end.
#[derive(Clone, Copy)]
enum Input {
    Track(f32),
    Steps(i32),
    End(bool),
}

/// Maps an input to a value.
type ToValue = Rc<dyn Fn(Input) -> f32>;
/// Attaches the change handler to an element: (element, press id, mapping, which part, thumb x).
type Bind = Rc<dyn Fn(El, String, ToValue, u8, f32) -> El>;

/// `<Slider value min max step onValueChange />`.
pub struct Slider {
    value: f32,
    min: f32,
    max: f32,
    step: f32,
    id: String,
    label: Option<String>,
    state: State,
    disabled: bool,
    bind: Option<Bind>,
}

impl Slider {
    #[track_caller]
    pub fn new(value: f32) -> Slider {
        Slider { value, min: 0.0, max: 100.0, step: 1.0, id: site_id(std::panic::Location::caller(), "slider"), label: None, state: State::Rest, disabled: false, bind: None }
    }
    /// `min` and `max` (0 to 100 by default).
    pub fn range(mut self, min: f32, max: f32) -> Self {
        self.min = min;
        self.max = max;
        self
    }
    pub fn step(mut self, s: f32) -> Self {
        self.step = s;
        self
    }
    /// The slider's key, unique in the window: its width is read from layout.
    pub fn id(mut self, id: impl Into<String>) -> Self {
        self.id = id.into();
        self
    }
    pub fn label(mut self, l: impl Into<String>) -> Self {
        self.label = Some(l.into());
        self
    }
    pub fn state(mut self, s: State) -> Self {
        self.state = s;
        self
    }
    pub fn disabled(mut self, d: bool) -> Self {
        self.disabled = d;
        self
    }
    /// `onValueChange(value)`, snapped to the step and clamped.
    pub fn on_change<S: 'static>(mut self, f: impl Fn(&mut S, f32) + 'static) -> Self {
        let f = Rc::new(f);
        // `which`: 0 the track, 1 the thumb's hit box (`left` its x in the track), 2 keys.
        self.bind = Some(Rc::new(move |e: El, id: String, value: Rc<dyn Fn(Input) -> f32>, which: u8, left: f32| {
            let (f, v) = (f.clone(), value.clone());
            match which {
                0 => e.on_drag(move |s: &mut S, d: &mut Drag, _tx: &mut Tx| {
                    note_press(&id, d.phase);
                    if !d.cancelled() && d.w > 0.0 {
                        f(s, v(Input::Track(d.x / d.w)));
                    }
                }),
                1 => e.on_drag(move |s: &mut S, d: &mut Drag, _tx: &mut Tx| {
                    note_press(&id, d.phase);
                    if !d.cancelled() {
                        f(s, v(Input::Track(left - 8.0 + d.x)));
                    }
                }),
                _ => {
                    let (f2, v2) = (f.clone(), v.clone());
                    e.on_step(move |s: &mut S, n: i32, _tx: &mut Tx| f(s, v(Input::Steps(n))))
                        .on_step_limit(move |s: &mut S, end: bool, _tx: &mut Tx| f2(s, v2(Input::End(end))))
                }
            }
        }));
        self
    }

    pub fn build<S>(self, th: &Theme, cx: &Cx<S>) -> El {
        let t = &th.t;
        let (min, max, step) = (self.min, self.max, self.step.max(f32::EPSILON));
        let span = (max - min).max(f32::EPSILON);
        let p = ((self.value - min) / span).clamp(0.0, 1.0);
        let w = cx.layout_size(&self.id).map_or(0.0, |s| s[0]);
        let enabled = !self.disabled;
        let held = enabled && (is_pressed(&self.id) || self.state == State::Pressed);
        // Radix keeps the thumb inside the track: its centre runs from 6 pt to `w - 6`. The root is
        // as tall as the track (Radix positions the thumb absolutely), so the thumb overhangs it.
        let left = p * (w - 12.0).max(0.0);
        let range = rune::el().pin(Some(0.0), Some(0.0), None, Some(0.0)).w_frac(p).fill(t.primary).a11y_hidden();
        let track = rune::el().key("track").grow(1.0).h(4.0).radius(radius::FULL).fill(t.muted).elevate(&t.elev_well).clip_rounded(radius::FULL).child(range);
        let ring = Theme::tint(t.ring, 0.5);
        // `transition-[color,box-shadow]` (150 ms).
        let tw = tween(0.15, TW_EASE);
        let mut ring_layer = layer(-3.0).radius(radius::FULL).border(3.0, ring).motion(Spring::new(0.15, 0.0)).key("ring");
        ring_layer = if held || self.state == State::Hover {
            ring_layer
        } else if enabled {
            ring_layer.group_opacity(0.0, 1.0)
        } else {
            ring_layer.opacity(0.0)
        };
        let mut thumb = rune::el()
            .key("thumb")
            .absolute(left, -4.0)
            .size(12.0, 12.0)
            .radius(radius::FULL)
            .fill(Color::WHITE)
            .border(1.0, t.ring)
            .elevate(&t.elev_knob)
            .hover_group()
            .hover_motion(tw, tw)
            .child(ring_layer);
        if enabled && self.state == State::Focus {
            thumb = thumb.outline(3.0, 0.0, ring);
        } else if enabled {
            thumb = thumb.focus_ring(3.0, 0.0, ring);
        }
        let mut e = rune::el().key(self.id.clone()).role("slider").w_full().h(4.0).center_y().child(track).range_value(self.value as f64, min as f64, max as f64).range_step(step as f64);
        if let Some(l) = &self.label {
            e = e.label(l.clone());
        }
        if !enabled {
            return e.child(thumb).disabled(true).layer_opacity(0.5);
        }
        let snap = move |v: f32| (min + ((v - min) / step).round() * step).clamp(min, max);
        let value = self.value;
        let wspan = w.max(1.0);
        let track_value: Rc<dyn Fn(Input) -> f32> = Rc::new(move |i| match i {
            Input::Track(x) => snap(min + x.clamp(0.0, 1.0) * span),
            Input::Steps(n) => snap(value + n as f32 * step),
            Input::End(end) => if end { max } else { min },
        });
        // The thumb's drag gives points along the track rather than a fraction.
        let tv = track_value.clone();
        let thumb_value: Rc<dyn Fn(Input) -> f32> = Rc::new(move |i| match i {
            Input::Track(x) => tv(Input::Track(x / wspan)),
            other => tv(other),
        });
        let hit = rune::el().key("hit").pin(Some(-8.0), Some(-8.0), Some(-8.0), Some(-8.0)).a11y_hidden();
        let (hit, e) = match &self.bind {
            Some(b) => {
                let hit = b(hit, self.id.clone(), thumb_value, 1, left);
                let e = b(e, self.id.clone(), track_value.clone(), 0, 0.0);
                (hit, b(e, self.id.clone(), track_value, 2, 0.0))
            }
            None => (track_press(hit, self.id.clone()), track_press(e, self.id.clone())),
        };
        e.child(thumb.child(hit)).focusable(true)
    }
}
