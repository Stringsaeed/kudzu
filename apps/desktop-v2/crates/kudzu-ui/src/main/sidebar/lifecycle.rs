//! Sidebar keyboard and change polling.

use crate::shell::MainState;

pub fn handle_key(s: &mut MainState, ev: &rune::KeyEvent, tx: &mut rune::Tx) -> bool {
    if s.is_composing() {
        return false;
    }
    if ev.key == rune::Key::Escape && s.sidebar.rename.is_some() {
        s.sidebar.rename = None;
        s.sidebar.edit.focused = false;
        tx.blur();
        return true;
    }
    if ev.key == rune::Key::Escape && s.sidebar.menu.take().is_some() {
        return true;
    }
    false
}

pub fn poll(s: &mut MainState) -> bool {
    let revision = s.svc.clipboard.watch().snapshot().0;
    let enabled = s.svc.settings.watch().get().clipboard_enabled;
    let status_revision = s.svc.expansion.status().revision();
    let changed = s.sidebar.clipboard_revision != Some(revision)
        || s.sidebar.clipboard_enabled != Some(enabled)
        || s.sidebar.status_revision != Some(status_revision);
    s.sidebar.clipboard_revision = Some(revision);
    s.sidebar.clipboard_enabled = Some(enabled);
    s.sidebar.status_revision = Some(status_revision);
    changed
}
