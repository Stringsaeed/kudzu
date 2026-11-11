//! User-initiated actions for the permission companion (`S66`).

use kudzu_app::ports::PermissionKind;
use objc2_foundation::MainThreadMarker;

use crate::permissions::model::UNBUNDLED_REVEAL_NOTICE;
use crate::shell::windows::PermissionsState;

pub fn close(s: &mut PermissionsState) {
    s.close();
}

/// Header close: hide the companion and return to `main` even while Help is open.
pub fn dismiss(s: &mut PermissionsState) {
    s.dismiss();
}

pub fn select_kind(s: &mut PermissionsState, kind: PermissionKind) {
    s.model.notice = None;
    s.stop_guide();
    s.svc.permissions.select(kind);
    s.spawn_refresh();
    let _ = s.sync();
}

pub fn begin_selected(s: &mut PermissionsState) {
    if !s.model.loaded || s.is_opening() {
        return;
    }
    s.model.notice = None;
    s.stop_guide();
    let kind = s.model.snap.selected;
    s.spawn_begin(kind);
    let _ = s.sync();
}

pub fn press_app_icon(s: &mut PermissionsState) {
    s.model.notice = None;
    reveal_or_notify(s);
}

pub fn reveal_app(s: &mut PermissionsState) {
    s.model.help_open = false;
    s.model.notice = None;
    reveal_or_notify(s);
}

fn reveal_or_notify(s: &mut PermissionsState) {
    if let Some(bundle) = s.model.bundle_path.clone() {
        if s.shell.native() {
            if let Some(mtm) = MainThreadMarker::new() {
                if let Err(err) = kudzu_appkit::app_bundle::reveal_in_finder(mtm, &bundle) {
                    s.model.notice = Some(err.to_owned());
                    return;
                }
            }
        }
        s.revealed_paths.push(bundle);
    } else {
        s.model.notice = Some(UNBUNDLED_REVEAL_NOTICE.to_owned());
    }
}

pub fn restart_app(s: &mut PermissionsState) {
    s.model.help_open = false;
    s.model.notice = None;
    s.stop_guide();
    s.spawn_restart();
}

pub fn toggle_help(s: &mut PermissionsState) {
    s.model.help_open = !s.model.help_open;
}

pub fn close_help(s: &mut PermissionsState) {
    s.model.help_open = false;
}

pub fn on_icon_drag_start(s: &mut PermissionsState) {
    s.guide.on_drag_start();
    s.shell.set_guide_window(None);
}

pub fn on_icon_drag_end(s: &mut PermissionsState) {
    s.guide.on_drag_end();
    s.spawn_refresh();
}
