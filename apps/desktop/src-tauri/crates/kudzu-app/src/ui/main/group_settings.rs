//! Group settings (`components/app/main/group-settings.tsx`): name, on/off, insertion method and
//! app rule for a group; snippets inside (and in subgroups) inherit them. Edits stay in a draft
//! until Save (or Return in the name field).

use rune::{col, row, text, Cx, El, TextEdit};
use uuid::Uuid;

use super::MainUi;
use crate::ui::kit::app_picker::{AppPicker, AppPickerState};
use crate::ui::kit::button::{Button, Variant};
use crate::ui::kit::dialog::{self, Dialog};
use crate::ui::kit::input::Input;
use crate::ui::kit::segmented::Segmented;
use crate::ui::kit::switch::Switch;
use crate::ui::theme::{radius, text as ty, weight, WeightOf};
use crate::ui::theme_for;
use kudzu_core::{AppRule, AppRuleMode, GroupRecord, InsertionMethod};

#[derive(Default)]
pub struct GroupSettingsState {
    pub draft: Option<GroupRecord>,
    pub name: TextEdit,
    pub apps: AppPickerState,
}

/// Opens the dialog on group `id` (a fresh draft, the name focused).
pub fn open(s: &mut MainUi, id: Uuid) {
    let Some(g) = s.group(id).cloned() else { return };
    let mut name = TextEdit::new(g.name.clone());
    name.set_caret(g.name.len());
    name.focused = true;
    s.group_dialog = GroupSettingsState { draft: Some(g), name, apps: AppPickerState::default() };
    s.group_settings = Some(id);
}

pub fn close(s: &mut MainUi) {
    s.group_settings = None;
    s.group_dialog.draft = None;
}

/// `onSubmit`: save the draft (an empty name keeps the old one) and close.
fn save(s: &mut MainUi) {
    let (Some(id), Some(mut draft)) = (s.group_settings, s.group_dialog.draft.clone()) else { return };
    let name = s.group_dialog.name.text.trim().to_string();
    draft.name = if name.is_empty() { s.group(id).map_or(draft.name.clone(), |g| g.name.clone()) } else { name };
    s.save_group(draft);
    close(s);
}

fn draft(s: &mut MainUi) -> Option<&mut GroupRecord> {
    s.group_dialog.draft.as_mut()
}

fn setting(th: &crate::ui::theme::Theme, title: &str, description: &str) -> El {
    col()
        .shrink()
        .min_w(0.0)
        .child(text(title.to_string()).font(ty::BASE.weight_of(weight::MEDIUM)).color(th.t.foreground))
        .child(text(description.to_string()).font(ty::XS).color(th.t.muted_foreground).wrap())
}

pub fn view(s: &MainUi, cx: &mut Cx<MainUi>, layer: &mut Vec<El>) {
    let (Some(id), Some(d)) = (s.group_settings, s.group_dialog.draft.as_ref()) else { return };
    if s.group(id).is_none() {
        return;
    }
    let th = theme_for(cx);
    let t = &th.t;

    let name = Input::new(&s.group_dialog.name)
        .label("Group name")
        .height(36.0)
        .on_edit(|s: &mut MainUi| &mut s.group_dialog.name)
        .on_submit(|s: &mut MainUi, _| save(s))
        .build(th)
        .grow(1.0);
    let on = d.is_enabled;
    let switch = row()
        .gap(8.0)
        .center_y()
        .child(Switch::new(on).label("Enabled").on_toggle(|s: &mut MainUi| {
            if let Some(d) = draft(s) {
                d.is_enabled = !d.is_enabled;
            }
        }).build(th))
        .child(text(if on { "On" } else { "Off" }).font(ty::XS).color(t.muted_foreground));

    let insertion = match d.insertion {
        None => "default",
        Some(InsertionMethod::Type) => "type",
        Some(InsertionMethod::Paste) => "paste",
        Some(InsertionMethod::Automatic) => "default",
    };
    let insert_by = Segmented::new("group-insertion", "Insertion method")
        .option("default", "Default")
        .option("type", "Type")
        .option("paste", "Paste")
        .selected(insertion)
        .on_change(|s: &mut MainUi, v: &str| {
            if let Some(d) = draft(s) {
                d.insertion = match v {
                    "type" => Some(InsertionMethod::Type),
                    "paste" => Some(InsertionMethod::Paste),
                    _ => None,
                };
            }
        })
        .build(th, cx);

    let rule = match d.rule.as_ref().map(|r| r.mode) {
        None => "none",
        Some(AppRuleMode::Allow) => "allow",
        Some(AppRuleMode::Deny) => "deny",
    };
    let rule_control = Segmented::new("group-rule", "App rule")
        .option("none", "All apps")
        .option("allow", "Only in…")
        .option("deny", "Except…")
        .selected(rule)
        .on_change(|s: &mut MainUi, v: &str| {
            if let Some(d) = draft(s) {
                let ids = d.rule.as_ref().map(|r| r.bundle_ids.clone()).unwrap_or_default();
                d.rule = match v {
                    "allow" => Some(AppRule { mode: AppRuleMode::Allow, bundle_ids: ids, insertion: d.rule.as_ref().and_then(|r| r.insertion) }),
                    "deny" => Some(AppRule { mode: AppRuleMode::Deny, bundle_ids: ids, insertion: d.rule.as_ref().and_then(|r| r.insertion) }),
                    _ => None,
                };
            }
        })
        .build(th, cx);
    let mut apps = col()
        .gap(12.0)
        .child(row().gap(16.0).center_y().justify_between().child(setting(th, "Apps", "Where snippets in this group can expand.")).child(rule_control));
    if let Some(r) = &d.rule {
        apps = apps.child(
            AppPicker::new("group-apps", &r.bundle_ids, &s.group_dialog.apps, |s: &mut MainUi| &mut s.group_dialog.apps)
                .on_change(|s: &mut MainUi, ids, _| {
                    if let Some(rule) = draft(s).and_then(|d| d.rule.as_mut()) {
                        rule.bundle_ids = ids;
                    }
                })
                .build(cx, layer),
        );
    }

    let form = col()
        .key("group-settings-form")
        .gap(20.0)
        .child(row().gap(12.0).center_y().child(name).child(switch))
        .child(row().gap(16.0).center_y().justify_between().child(setting(th, "Insert by", "How text is delivered for snippets in this group.")).child(insert_by))
        .child(apps);
    let footer = dialog::footer(
        cx,
        16.0,
        radius::XXL,
        [
            Button::new("Cancel").variant(Variant::Ghost).on_press(close).build(th),
            Button::new("Save").on_press(save).build(th).default_button(),
        ],
    );

    Dialog::new("group-settings")
        .open(true)
        .on_open_change(|s: &mut MainUi, open, _| {
            if !open {
                close(s);
            }
        })
        .max_width(520.0)
        .gap(20.0)
        .radius(radius::XXL)
        .float()
        .ring(false)
        .label("Group settings")
        .child(dialog::header(cx, "Group settings", Some("Snippets in this group (and its subgroups) inherit these unless they set their own.")).gap(4.0))
        .child(form)
        .child(footer)
        .build(cx, layer);
}
