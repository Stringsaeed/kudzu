//! Inline group-name editor.

use super::actions;
use crate::shell::MainState;
use crate::theme::Theme;
use rune::{text_input, El};

pub(super) fn view(s: &MainState, theme: &Theme) -> El {
    text_input(&s.sidebar.edit, theme.ty.control_sm)
        .key("sidebar-group-rename")
        .single_line()
        .on_edit(|st: &mut MainState| &mut st.sidebar.edit)
        .on_submit(|st: &mut MainState, _| {
            if !st.is_composing() {
                actions::commit_rename(st);
            }
        })
        .on_physical_input(|st: &mut MainState, event, _| {
            if matches!(event, rune::PhysicalInput::FocusLost) && !st.sidebar.edit.is_composing() {
                actions::commit_rename(st);
            }
            false
        })
        .on_input_key(|st: &mut MainState, ev, tx| {
            if st.is_composing() {
                return false;
            }
            if ev.key == rune::Key::Escape {
                st.sidebar.rename = None;
                st.sidebar.edit.focused = false;
                tx.blur();
                true
            } else {
                false
            }
        })
        .h(28.0)
}
