//! Save, draft and icon requests for the group dialog.
//!
//! A save records the writer watch's revision. A later revision that carries an error is this
//! save's failure, even when the error value compares equal to the one already showing.
//! `parent_id` and `sort_index` are read from the live library at save time. `is_enabled`,
//! `insertion` and `rule` are written from the draft even when the user has not touched them,
//! so an edit that lands while the dialog is open is reverted. Tauri's dialog does the same.

use std::sync::Arc;

use kudzu_app::persist::{PersistError, Revision};
use kudzu_core::library::model::{AppRule, AppRuleMode, GroupRecord};

use crate::shell::MainState;
use crate::widgets::app_picker;

#[derive(Clone)]
struct Submitted {
    name: String,
    enabled: bool,
    insertion: Option<kudzu_core::library::model::InsertionMethod>,
    rule: Option<AppRule>,
}

#[derive(Clone)]
pub(super) struct PendingSave {
    revision: Revision,
    watch_at: Revision,
    submitted: Submitted,
}

/// Open `id` and load its draft before the next frame.
pub fn open(s: &mut MainState, id: uuid::Uuid) {
    s.frame.group_settings_id = Some(id);
    load_draft(s);
}

pub(super) fn saving(s: &MainState) -> bool {
    s.group_settings.pending.is_some() && s.group_settings.error.is_none()
}

pub(super) fn set_rule(s: &mut MainState, mode: Option<AppRuleMode>) {
    match mode {
        None => {
            s.group_settings.rule = None;
            close_list(s);
        }
        Some(mode) => match &mut s.group_settings.rule {
            Some(rule) if rule.mode == mode => {}
            Some(rule) => {
                rule.mode = mode;
                rule.insertion = None;
            }
            None => {
                s.group_settings.rule = Some(AppRule {
                    mode,
                    bundle_ids: Vec::new(),
                    insertion: None,
                });
            }
        },
    }
}

pub(super) fn toggle_app(s: &mut MainState, id: String) {
    let Some(rule) = s.group_settings.rule.as_mut() else {
        return;
    };
    if let Some(index) = rule.bundle_ids.iter().position(|v| v == &id) {
        rule.bundle_ids.remove(index);
    } else {
        rule.bundle_ids.push(id);
    }
}

pub(super) fn set_picker_open(s: &mut MainState, open: bool) {
    let opening = open && !s.group_settings.picker_open;
    s.group_settings.picker_open = open;
    if opening {
        s.group_settings.name.focused = false;
        s.group_settings.query.focused = true;
    }
}

pub(super) fn set_picker_active(s: &mut MainState, index: usize) {
    s.group_settings.picker_active = index;
}

pub(super) fn save(s: &mut MainState) {
    if saving(s) {
        return;
    }
    let (watch_at, already) = s.svc.library.saved().snapshot();
    if let Some(error) = &already.error {
        s.group_settings.error = Some(kept(error));
    } else {
        s.group_settings.error = None;
    }
    let Some(group) = draft_record(s) else {
        return;
    };
    let submitted = Submitted {
        name: s.group_settings.name.text.clone(),
        enabled: s.group_settings.enabled,
        insertion: s.group_settings.insertion,
        rule: s.group_settings.rule.clone(),
    };
    match s.svc.library.update_group(group) {
        Err(error) => {
            s.group_settings.error = Some(error.to_string());
            s.group_settings.pending = None;
        }
        Ok((_, revision)) => {
            let saved = s.svc.library.saved().get();
            if saved.durable >= revision && draft_matches(s, &submitted) {
                super::dismiss(s);
            } else {
                s.group_settings.pending = Some(PendingSave {
                    revision,
                    watch_at,
                    submitted,
                });
            }
        }
    }
}

pub(super) fn poll_save(s: &mut MainState) -> bool {
    let Some(pending) = s.group_settings.pending.clone() else {
        return false;
    };
    let (rev, saved) = s.svc.library.saved().snapshot();
    if saved.durable >= pending.revision {
        if draft_matches(s, &pending.submitted) {
            super::dismiss(s);
        } else {
            s.group_settings.pending = None;
            s.group_settings.error = None;
        }
        return true;
    }
    if rev > pending.watch_at {
        if let Some(error) = &saved.error {
            let message = kept(error);
            if s.group_settings.error.as_deref() != Some(message.as_str()) {
                s.group_settings.error = Some(message);
                return true;
            }
        }
    }
    false
}

pub(super) fn load_draft(s: &mut MainState) {
    s.group_settings.open_for = s.frame.group_settings_id;
    s.group_settings.picker_open = false;
    s.group_settings.picker_active = 0;
    s.group_settings.query = rune::TextEdit::new("");
    s.group_settings.pending = None;
    s.group_settings.error = None;
    let Some(id) = s.frame.group_settings_id else {
        s.group_settings.blur_inputs();
        return;
    };
    let library = s.svc.library.watch().get();
    if let Some(group) = library.groups.iter().find(|group| group.id == id).cloned() {
        s.group_settings.name = rune::TextEdit::new(&group.name);
        s.group_settings.enabled = group.is_enabled;
        s.group_settings.insertion = group.insertion;
        s.group_settings.rule = group.rule;
    } else {
        s.group_settings.name = rune::TextEdit::new("");
        s.group_settings.enabled = true;
        s.group_settings.insertion = None;
        s.group_settings.rule = None;
    }
    s.group_settings.name.focused = true;
}

pub(super) fn request_icons(s: &mut MainState) {
    let installed = s.svc.apps.installed().get();
    let chosen = chosen_ids(s);
    let needed = app_picker::icons_needed(
        chosen,
        &installed,
        &s.group_settings.query.text,
        s.group_settings.picker_open,
    );
    let apps = Arc::clone(&s.svc.apps);
    for (id, size) in needed {
        s.group_settings.icons.request(&apps, &id, size);
    }
}

pub(super) fn chosen_ids(s: &MainState) -> &[String] {
    s.group_settings
        .rule
        .as_ref()
        .map(|rule| rule.bundle_ids.as_slice())
        .unwrap_or(&[])
}

fn draft_record(s: &MainState) -> Option<GroupRecord> {
    let id = s.frame.group_settings_id?;
    let library = s.svc.library.watch().get();
    let mut group = library
        .groups
        .iter()
        .find(|group| group.id == id)
        .cloned()
        .unwrap_or(GroupRecord {
            id,
            name: String::new(),
            parent_id: None,
            sort_index: 0,
            is_enabled: true,
            insertion: None,
            rule: None,
        });
    group.id = id;
    group.name = s.group_settings.name.text.clone();
    group.is_enabled = s.group_settings.enabled;
    group.insertion = s.group_settings.insertion;
    // Untouched fields are still written. `parent_id` and `sort_index` stay the current record's.
    group.rule = s.group_settings.rule.clone();
    Some(group)
}

fn draft_matches(s: &MainState, submitted: &Submitted) -> bool {
    s.group_settings.name.text == submitted.name
        && s.group_settings.enabled == submitted.enabled
        && s.group_settings.insertion == submitted.insertion
        && s.group_settings.rule == submitted.rule
}

fn kept(error: &PersistError) -> String {
    format!("{error} The change is kept and will be retried.")
}

fn close_list(s: &mut MainState) {
    s.group_settings.picker_open = false;
    s.group_settings.picker_active = 0;
    s.group_settings.query.clear();
    s.group_settings.query.focused = false;
}
