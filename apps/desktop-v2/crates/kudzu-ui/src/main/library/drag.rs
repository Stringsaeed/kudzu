//! Rows as drag sources (LIB-11's source half). S42 owns the drop targets and the ghost in
//! `dnd.rs` and reads [`payload`] while a drag is under way.
//!
//! The payload is snapshotted on Down: the multi-selection when the row is in it, else the row
//! alone. It becomes a drag once the pointer travels [`THRESHOLD`]. No library change happens
//! here; every phase is `live_only`, since nothing the list draws depends on it.

use rune::{Drag, Phase, Tx};
use uuid::Uuid;

use crate::dnd::DragPayload;
use crate::shell::MainState;

/// Pointer travel, in points, before a press becomes a drag.
pub const THRESHOLD: f32 = 4.0;

#[derive(Debug, Default)]
pub struct Source {
    /// Taken on Down, waiting for the threshold.
    pending: Option<DragPayload>,
    /// The drag under way.
    active: Option<DragPayload>,
    /// RUNE-GAP(#551): a release after a drag inside the row also fires `on_press`. Delete this
    /// flag, and [`take_press`], once a drag no longer ends in a press.
    swallow_press: bool,
}

/// What is being dragged, once a drag has started.
pub fn payload(s: &MainState) -> Option<&DragPayload> {
    s.library.drag.active.as_ref()
}

/// The ids a drag from row `id` carries (`snippet-list.tsx` `onDragStart`).
fn carried(s: &MainState, id: Uuid) -> Vec<Uuid> {
    let bulk = s.library.selection.bulk();
    if bulk.contains(&id) {
        bulk.to_vec()
    } else {
        vec![id]
    }
}

/// `on_drag` of the row for `id`.
pub(super) fn gesture(s: &mut MainState, id: Uuid, d: &mut Drag, tx: &mut Tx) {
    let carried = (d.phase == Phase::Down).then(|| carried(s, id));
    let source = &mut s.library.drag;
    match d.phase {
        Phase::Down => {
            source.swallow_press = false;
            source.active = None;
            source.pending = carried.map(DragPayload::Snippets);
        }
        Phase::Move => {
            if source.active.is_none() && d.dx.hypot(d.dy) >= THRESHOLD {
                source.active = source.pending.take();
            }
            crate::dnd::source_move(&mut s.sidebar, source.active.clone(), d, tx);
        }
        Phase::Up => {
            crate::dnd::source_release(&mut s.sidebar, source.active.clone(), d, tx);
            source.pending = None;
            if source.active.take().is_some() {
                source.swallow_press = true;
            }
        }
    }
    tx.live_only();
}

/// True for a real click; false for the press Rune #551 adds after a drag.
pub(super) fn take_press(s: &mut MainState) -> bool {
    !std::mem::take(&mut s.library.drag.swallow_press)
}
