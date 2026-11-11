//! RUNE-GAP(#360: hover and press states for shadows and child geometry): delete this module
//! once an element can take a shadow (and a child's frame) per hover and press state.
//!
//! The raised controls change their depth under the pointer (`hover:shadow-raised-hover`,
//! `active:shadow-pressed`, `active:bg-none`), and the switch's knob stretches 1.2× from one edge
//! while the switch is held. Rune swaps a fill under hover and press (`El::hover`,
//! `El::press`) but not a shadow or a child's frame, and a view knows neither state. So:
//!
//! - **hover** is a pair of layers inside the control, crossfaded by `group_opacity` with the
//!   control as the hover group. Each layer is the control's face: its fill (rest or hovered),
//!   its shadows and, at rest, its sheen. Rune draws an element's outer shadow under its whole
//!   box and lets the element's own fill hide the inside (CSS clips it instead), so a layer
//!   must be filled or its drop shadow shows through the face;
//! - **press** is a [`Live`] value the control's drag handler sets on Down and clears on Up
//!   (`Tx::live_only`, so the view does not rebuild), bound to the pressed layer's opacity, or
//!   to the knob's frame with `live_rect`. The control keeps its `on_press`: Rune delivers the
//!   drag's Up and then the press.
//!
//! A rebuild while a press is held (another event) starts a new value at rest, so the look lets
//! go early rather than sticking.

use rune::{el, Color, El, Live, Phase, Shadow};

use crate::theme::motion;

/// The press spring: the soft curve at `FAST` (the `press` utility sinks in `FAST`).
pub fn press_spring() -> rune::Spring {
    motion::soft_spring(motion::FAST)
}

/// A layer filling its parent, drawn over the parent's fill and under its content.
pub fn layer(key: &str, radius: f32, fill: Color, shadows: &[Shadow]) -> El {
    shadows.iter().fold(
        el().key(key.to_owned())
            .pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0))
            .radius(radius)
            .fill(fill)
            .paint_order(-1)
            .a11y_hidden(),
        |e, s| e.box_shadow(*s),
    )
}

/// Which look a control shows, forced (galleries) or live.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Look {
    Live,
    Rest,
    Hover,
    Pressed,
}

/// The faces a raised control shows: its fill at rest and hovered (pressed keeps the hovered
/// fill, as `:active` implies `:hover`), and its sheen at rest (`active:bg-none` drops it).
#[derive(Clone, Copy, Debug)]
pub struct Faces {
    pub rest: Color,
    pub hover: Color,
    pub sheen: Option<(Color, Color)>,
}

/// A raised control's depth: `rest` (with its sheen, if any) and `hover` crossfaded by hover,
/// both fading out for `pressed` while held. `control` must be the hover group.
pub fn raised_layers(
    control: El,
    radius: f32,
    faces: Faces,
    rest: &[Shadow],
    hover: &[Shadow],
    pressed: &[Shadow],
    look: Look,
) -> El {
    let mut rest_l = layer("depth-rest", radius, faces.rest, rest);
    if let Some((top, bottom)) = faces.sheen {
        rest_l = rest_l.gradient(top, bottom);
    }
    let hover_l = layer("depth-hover", radius, faces.hover, hover);
    let pressed_l = layer("depth-pressed", radius, faces.hover, pressed);
    let (rest_o, hover_o) = match look {
        Look::Hover => ((0.0, 0.0), (1.0, 1.0)),
        Look::Pressed => ((0.0, 0.0), (0.0, 0.0)),
        _ => ((1.0, 0.0), (0.0, 1.0)),
    };
    let stack = |e: El| {
        e.child(
            rest_l
                .group_opacity(rest_o.0, rest_o.1)
                .opacity_motion(motion::press::PAINT),
        )
        .child(
            hover_l
                .group_opacity(hover_o.0, hover_o.1)
                .opacity_motion(motion::press::PAINT),
        )
    };
    match look {
        Look::Live => {
            let (live, unheld) = (Live::new(0.0), Live::new(1.0));
            let (down, up) = (live.clone(), unheld.clone());
            // One drag handler per element: the pressed layer and the raised pair read the
            // same press, the pair through its complement.
            let control = control.hover_group().on_drag_any(move |d, tx| {
                let on =
                    matches!(d.phase, Phase::Down) || (d.phase == Phase::Move && !d.cancelled());
                down.set(if on { 1.0 } else { 0.0 });
                up.set(if on { 0.0 } else { 1.0 });
                tx.live_only();
            });
            let raised = stack(layer("depth", radius, Color::TRANSPARENT, &[]))
                .live_opacity(unheld.smooth(press_spring()));
            control
                .child(raised)
                .child(pressed_l.live_opacity(live.smooth(press_spring())))
        }
        Look::Pressed => stack(control.hover_group()).child(pressed_l),
        _ => stack(control.hover_group()).child(pressed_l.opacity(0.0)),
    }
}
