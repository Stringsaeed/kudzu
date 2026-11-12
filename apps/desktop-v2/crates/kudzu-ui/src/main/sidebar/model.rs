//! Transient sidebar state.

use rune::TextEdit;
use std::collections::HashSet;
use uuid::Uuid;

#[derive(Clone, Debug, Default)]
pub struct State {
    pub menu: Option<(Uuid, f32, f32)>,
    pub rename: Option<Uuid>,
    pub edit: TextEdit,
    pub collapsed: HashSet<Uuid>,
    pub drag: Option<(crate::dnd::DragPayload, f32, f32)>,
    pub drop_target: Option<crate::dnd::DropTarget>,
    pub(super) clipboard_revision: Option<kudzu_app::persist::Revision>,
    pub(super) status_revision: Option<kudzu_app::persist::Revision>,
    pub(super) clipboard_enabled: Option<bool>,
}

impl State {
    pub fn is_composing(&self) -> bool {
        self.edit.is_composing()
    }

    pub fn is_editing_text(&self) -> bool {
        self.edit.focused
    }

    pub fn blur_inputs(&mut self) {
        self.edit.focused = false;
    }
}
