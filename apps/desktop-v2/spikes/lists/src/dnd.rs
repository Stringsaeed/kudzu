// RUNE-GAP(#357): replace rect registry and gesture routing with typed in-app targets.
// RUNE-GAP(#551): suppress the press Rune emits after a drag ends inside its source.
use crate::*;
pub struct Dnd {
    rects: [[f32; 4]; 2],
    active: bool,
    pub suppress_press: bool,
    payload: Vec<usize>,
    pub dropped: Vec<usize>,
    pub over: Option<usize>,
    point: Live<[f32; 2]>,
    opacity: Live<f32>,
    fills: [Live<Color>; 2],
}
pub fn sidebar(s: &State) -> El {
    col().w(180.0).h_full().children((0..2).map(|id| {
        text(if id == 0 {
            "Accept group"
        } else {
            "Reject group"
        })
        .key(format!("zone-{id}"))
        .w(180.0)
        .h(100.0)
        .live_fill(&s.dnd.fills[id])
        .on_drawn_layout_tx(move |s: &mut State, rect, tx| {
            s.dnd.rects[id] = rect;
            tx.live_only();
        })
    }))
}
pub fn ghost(s: &State) -> El {
    text(format!("{} selected", s.dnd.payload.len()))
        .key("ghost")
        .pin(Some(0.0), Some(0.0), None, None)
        .live_offset(&s.dnd.point)
        .live_opacity(&s.dnd.opacity)
        .inert()
}
pub fn gesture(s: &mut State, id: usize, d: &mut Drag, tx: &mut Tx) {
    let m = &mut s.dnd;
    match d.phase {
        Phase::Down => {
            m.payload = if s.selected.ids.contains(&id) {
                s.selected.ids.iter().copied().collect()
            } else {
                vec![id]
            };
            m.active = false;
            m.suppress_press = false;
        }
        Phase::Move => {
            if d.dx.hypot(d.dy) >= 4.0 {
                m.active = true;
            }
            let [x, y] = d.window_point();
            m.point.set([x + 12.0, y + 12.0]);
            m.opacity.set(if m.active { 1.0 } else { 0.0 });
            m.over = m
                .rects
                .iter()
                .position(|&[a, b, w, h]| x >= a && x < a + w && y >= b && y < b + h);
            for i in 0..2 {
                m.fills[i].set(if m.over == Some(i) {
                    oklch(0.5, 0.15, if i == 0 { 140.0 } else { 25.0 })
                } else {
                    oklch(0.2, 0.0, 0.0)
                });
            }
            tx.live_only();
        }
        Phase::Up => {
            m.suppress_press = m.active;
            if m.active && !d.cancelled() && m.over == Some(0) {
                m.dropped = m.payload.clone();
            }
            m.active = false;
            m.over = None;
            m.opacity.set(0.0);
            for fill in &m.fills {
                fill.set(oklch(0.2, 0.0, 0.0));
            }
        }
    }
}

impl Default for Dnd {
    fn default() -> Self {
        Self {
            rects: [[0.0; 4]; 2],
            active: false,
            suppress_press: false,
            payload: vec![],
            dropped: vec![],
            over: None,
            point: Live::new([0.0; 2]),
            opacity: Live::new(0.0),
            fills: std::array::from_fn(|_| Live::new(oklch(0.2, 0.0, 0.0))),
        }
    }
}
