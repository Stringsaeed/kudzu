//! Group settings: name, on/off, insertion and the app rule (`PLAN.md` S55, RUL-7, RUL-8).
//!
//! The panel is composed here. `kit_ext::dialog` keeps Close in the header row, so the heading
//! cannot be the reference 488 pt. Save waits until the library writer reports the revision
//! durable and leaves the dialog open with the error when the write fails (PLAN D5).
//!
//! Rune gives Escape in a focused text field to the editor, which blurs it, so the name field
//! closes the dialog from `TextInput::on_key` and the picker's search closes only the list.

use std::sync::Arc;

use kudzu_app::persist::Revision;
use kudzu_app::watch::Subscription;
use kudzu_core::library::model::{AppRule, AppRuleMode, GroupRecord, InsertionMethod};
use rune::{col, el, row, text, Cx, El, Key, KeyEvent, TextEdit, TextStyle};

use crate::shell::MainState;
use crate::theme::{icons, radius, Theme};
use crate::widgets::app_icon::IconCache;
use crate::widgets::app_picker::{self, AppPicker};
use crate::widgets::kit_ext::button::{Button, Size, Variant};
use crate::widgets::kit_ext::input::Input;
use crate::widgets::kit_ext::switch::Switch;
use crate::widgets::selection_pill::{Segment, Segmented};

const DIALOG_W: f32 = 520.0;
/// Insert and app rows. 18.5625 + 16 = 34.5625, which centres the 32 pt segmented so its
/// radios land on the reference y (the 13/500 title keeps a line shorter than the inherited 20).
const ROW_H: f32 = 34.5625;
const DESCRIPTION: &str =
    "Snippets in this group (and its subgroups) inherit these unless they set their own.";

struct PendingSave {
    revision: Revision,
    error_at_submit: Option<kudzu_app::persist::PersistError>,
}

/// Draft for the open dialog. The icon cache outlives one group so a repeat open does not
/// read the catalogue again.
pub struct State {
    open_for: Option<uuid::Uuid>,
    pub(crate) name: TextEdit,
    pub(crate) enabled: bool,
    pub(crate) insertion: Option<InsertionMethod>,
    pub(crate) rule: Option<AppRuleMode>,
    pub(crate) bundle_ids: Vec<String>,
    pending: Option<PendingSave>,
    pub(crate) error: Option<String>,
    pub(crate) query: TextEdit,
    picker_open: bool,
    icons: IconCache,
    save_sub: Option<Subscription>,
}

impl State {
    pub fn new(wake: rune::Waker) -> Self {
        Self {
            open_for: None,
            name: TextEdit::new(""),
            enabled: true,
            insertion: None,
            rule: None,
            bundle_ids: Vec::new(),
            pending: None,
            error: None,
            query: TextEdit::new(""),
            picker_open: false,
            icons: IconCache::new(wake),
            save_sub: None,
        }
    }

    pub fn is_composing(&self) -> bool {
        self.name.is_composing() || self.query.is_composing()
    }

    pub fn is_editing_text(&self) -> bool {
        self.name.focused || self.query.focused
    }

    pub fn blur_inputs(&mut self) {
        self.name.focused = false;
        self.query.focused = false;
    }

    pub fn picker_open(&self) -> bool {
        self.picker_open
    }
}

/// Close the app list and leave the dialog up.
pub fn close_picker(s: &mut MainState) {
    s.group_settings.picker_open = false;
}

/// Pull icon bytes and the save result. Runs on every poll, including when the library
/// revision is unchanged: a durable write does not bump the snapshot the window already has.
pub fn sync(s: &mut MainState) -> bool {
    let mut changed = false;
    if s.group_settings.save_sub.is_none() {
        let main = s.shell.handles().main.clone();
        s.group_settings.save_sub = Some(s.svc.library.saved().subscribe(move || main.wake()));
        changed = true;
    }
    changed |= s.group_settings.icons.poll();
    changed |= poll_save(s);
    if s.group_settings.open_for != s.frame.group_settings_id {
        load_draft(s);
        changed = true;
    }
    if s.frame.group_settings_id.is_some() {
        request_icons(s);
    }
    changed
}

pub fn view(s: &MainState, cx: &Cx<MainState>, theme: &Theme, overlays: &mut Vec<El>) {
    if s.frame.group_settings_id.is_none() {
        return;
    }
    let c = &theme.c;
    let installed = s.svc.apps.installed().get();
    let mut picker_overlays = Vec::new();
    let mut form = col()
        .w_full()
        .px(16.0)
        .pt(16.0)
        .gap(20.0)
        .child(header(theme))
        .child(name_row(s, theme))
        .child(insert_row(s, cx, theme))
        .child(apps_section(s, cx, theme, &installed, &mut picker_overlays));
    if let Some(message) = &s.group_settings.error {
        form = form.child(
            text(message.clone())
                .key("group-settings-error")
                .font(theme.ty.body)
                .color(c.destructive),
        );
    }

    let mut panel = col()
        .key("group-settings-dialog")
        .role("dialog")
        .label("Group settings")
        .w(DIALOG_W)
        .radius(radius::XL2)
        .fill(c.popover)
        .token_motion(theme.kit().motion.overlay)
        .on_key(|st: &mut MainState, ev, _| {
            if bare_escape(ev) && st.group_settings.picker_open {
                close_picker(st);
                true
            } else {
                false
            }
        })
        .modal(true, |st: &mut MainState| dismiss(st))
        .child(form)
        .child(el().h(20.0).w_full())
        .child(footer(theme))
        .child(close_button(theme));
    for shadow in &theme.depth.float {
        panel = panel.box_shadow(*shadow);
    }
    for extra in picker_overlays {
        panel = panel.child(extra);
    }

    overlays.push(
        el().key("group-settings-scrim")
            .absolute(0.0, 0.0)
            .w_full()
            .h_full()
            .fill(theme.kit().c.scrim)
            .token_motion(theme.kit().motion.overlay)
            .a11y_hidden(),
    );
    overlays.push(
        el().key("group-settings-layer")
            .absolute(0.0, 0.0)
            .w_full()
            .h_full()
            .padding(16.0)
            .center()
            .child(panel),
    );
}

fn header(theme: &Theme) -> El {
    let c = &theme.c;
    col()
        .w_full()
        .gap(4.0)
        .child(
            el().role("heading")
                .label("Group settings")
                .w_full()
                .h(16.0)
                .child(
                    text("Group settings")
                        .font(TextStyle::new(16.0, 500.0).lh(16.0))
                        .color(c.foreground),
                ),
        )
        .child(
            el().w_full().h(40.0).child(
                text(DESCRIPTION)
                    .font(theme.ty.input)
                    .color(c.muted_foreground)
                    .wrap(),
            ),
        )
}

fn name_row(s: &MainState, theme: &Theme) -> El {
    let on = s.group_settings.enabled;
    let word = if on { "On" } else { "Off" };
    row()
        .w_full()
        .h(36.0)
        .items_center()
        .gap(12.0)
        .child(
            Input::new(&s.group_settings.name)
                .with(|field| {
                    field
                        .height(36.0)
                        .on_edit(|st: &mut MainState| &mut st.group_settings.name)
                        .on_key(|st: &mut MainState, ev, _| {
                            if !bare_escape(ev) {
                                return false;
                            }
                            // The editor consumes Escape as blur, so the field closes the dialog.
                            if st.group_settings.picker_open {
                                close_picker(st);
                            } else {
                                dismiss(st);
                            }
                            true
                        })
                })
                // The name belongs on the 36 pt shell. rune-kit labels the inner editor, which
                // is inset and only a line tall, so the reference textbox would miss.
                .build(theme)
                .key("group-name")
                .label("Group name")
                .grow(1.0)
                .min_w(0.0),
        )
        .child(
            row()
                .items_center()
                .gap(8.0)
                .child(
                    Switch::new(on)
                        .label(word)
                        .on_toggle(|st: &mut MainState| {
                            st.group_settings.enabled = !st.group_settings.enabled;
                        })
                        .build(theme)
                        .key("group-enabled"),
                )
                .child(
                    text(word)
                        .font(TextStyle::new(12.0, 400.0).lh(16.0))
                        .color(theme.c.muted_foreground),
                ),
        )
}

fn insert_row(s: &MainState, cx: &Cx<MainState>, theme: &Theme) -> El {
    row()
        .w_full()
        .h(ROW_H)
        .items_center()
        .justify_between()
        .gap(16.0)
        .child(labeled(
            theme,
            "Insert by",
            "How text is delivered for snippets in this group.",
        ))
        .child(
            Segmented::new(
                "group-insertion",
                "Insertion method",
                insertion_value(s.group_settings.insertion),
            )
            .option(Segment::new("default", "Default"))
            .option(Segment::new("type", "Type"))
            .option(Segment::new("paste", "Paste"))
            .build(theme, cx, |st: &mut MainState, value| {
                st.group_settings.insertion = match value {
                    "type" => Some(InsertionMethod::Type),
                    "paste" => Some(InsertionMethod::Paste),
                    _ => None,
                };
            }),
        )
}

fn apps_section(
    s: &MainState,
    cx: &Cx<MainState>,
    theme: &Theme,
    installed: &[kudzu_core::system::AppInfo],
    overlays: &mut Vec<El>,
) -> El {
    let value = rule_value(s.group_settings.rule);
    let mut section = col().w_full().gap(12.0).child(
        row()
            .w_full()
            .h(ROW_H)
            .items_center()
            .justify_between()
            .gap(16.0)
            .child(labeled(
                theme,
                "Apps",
                "Where snippets in this group can expand.",
            ))
            .child(
                Segmented::new("group-rule", "App rule", value)
                    .option(Segment::new("none", "All apps"))
                    .option(Segment::new("allow", "Only in…"))
                    .option(Segment::new("deny", "Except…"))
                    .build(theme, cx, |st: &mut MainState, value| {
                        st.group_settings.rule = match value {
                            "allow" => Some(AppRuleMode::Allow),
                            "deny" => Some(AppRuleMode::Deny),
                            _ => {
                                st.group_settings.bundle_ids.clear();
                                st.group_settings.picker_open = false;
                                None
                            }
                        };
                    }),
            ),
    );
    if s.group_settings.rule.is_some() {
        let origin = cx
            .layout("group-settings-dialog")
            .map(|r| [r[0], r[1]])
            .unwrap_or([0.0, 0.0]);
        section = section.child(
            AppPicker::new(
                &s.group_settings.bundle_ids,
                installed,
                &s.group_settings.query,
                &s.group_settings.icons,
            )
            .open(s.group_settings.picker_open)
            .origin(origin)
            .build(
                theme,
                cx,
                |st: &mut MainState| &mut st.group_settings.query,
                toggle_app,
                |st: &mut MainState, open| st.group_settings.picker_open = open,
                overlays,
            ),
        );
    }
    section
}

fn footer(theme: &Theme) -> El {
    let c = &theme.c;
    col()
        .key("group-settings-footer")
        .w_full()
        .fill(c.muted.alpha(0.5))
        .child(el().h(1.0).w_full().fill(c.border))
        .child(
            row()
                .w_full()
                .padding(16.0)
                .gap(8.0)
                .justify_end()
                .child(
                    Button::new("Cancel")
                        .variant(Variant::Ghost)
                        .on_press(|st: &mut MainState| dismiss(st))
                        .build(theme)
                        .key("group-settings-cancel"),
                )
                .child(
                    Button::new("Save")
                        .on_press(|st: &mut MainState| save(st))
                        .build(theme)
                        .key("group-settings-save"),
                ),
        )
}

fn close_button(theme: &Theme) -> El {
    Button::new("Close")
        .variant(Variant::Ghost)
        .size(Size::IconSm)
        .icon(&icons::X)
        .on_press(|st: &mut MainState| dismiss(st))
        .build(theme)
        .key("group-settings-close")
        .absolute(484.0, 8.0)
}

fn labeled(theme: &Theme, title: &str, hint: &str) -> El {
    col()
        .child(
            text(title)
                .font(TextStyle::new(13.0, 500.0).lh(18.5625))
                .color(theme.c.foreground),
        )
        .child(
            text(hint)
                .font(TextStyle::new(12.0, 400.0).lh(16.0))
                .color(theme.c.muted_foreground),
        )
}

fn insertion_value(method: Option<InsertionMethod>) -> &'static str {
    match method {
        None => "default",
        Some(InsertionMethod::Type) => "type",
        Some(InsertionMethod::Paste) => "paste",
        Some(InsertionMethod::Automatic) => "automatic",
    }
}

fn rule_value(mode: Option<AppRuleMode>) -> &'static str {
    match mode {
        Some(AppRuleMode::Allow) => "allow",
        Some(AppRuleMode::Deny) => "deny",
        None => "none",
    }
}

fn toggle_app(s: &mut MainState, id: String) {
    if let Some(index) = s.group_settings.bundle_ids.iter().position(|v| v == &id) {
        s.group_settings.bundle_ids.remove(index);
    } else {
        s.group_settings.bundle_ids.push(id);
    }
}

fn dismiss(s: &mut MainState) {
    s.frame.group_settings_id = None;
    s.group_settings.pending = None;
    s.group_settings.picker_open = false;
    s.group_settings.name.focused = false;
    s.group_settings.query.focused = false;
}

fn save(s: &mut MainState) {
    if s.group_settings.pending.is_some() {
        return;
    }
    let Some(group) = draft_record(s) else {
        return;
    };
    match s.svc.library.update_group(group) {
        Err(error) => s.group_settings.error = Some(error.to_string()),
        Ok((_, revision)) => {
            let saved = s.svc.library.saved().get();
            if saved.durable >= revision {
                dismiss(s);
            } else {
                s.group_settings.error = None;
                s.group_settings.pending = Some(PendingSave {
                    revision,
                    error_at_submit: saved.error,
                });
            }
        }
    }
}

fn draft_record(s: &MainState) -> Option<GroupRecord> {
    let id = s.frame.group_settings_id?;
    let mut group = s
        .library_snapshot()
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
    group.rule = s.group_settings.rule.map(|mode| AppRule {
        mode,
        bundle_ids: s.group_settings.bundle_ids.clone(),
        insertion: None,
    });
    Some(group)
}

fn poll_save(s: &mut MainState) -> bool {
    let Some(pending) = &s.group_settings.pending else {
        return false;
    };
    let saved = s.svc.library.saved().get();
    if saved.durable >= pending.revision {
        dismiss(s);
        return true;
    }
    if let Some(error) = saved.error {
        if Some(error)
            != s.group_settings
                .pending
                .as_ref()
                .and_then(|p| p.error_at_submit)
        {
            let message = error.to_string();
            if s.group_settings.error.as_deref() != Some(message.as_str()) {
                s.group_settings.error = Some(message);
                return true;
            }
        }
    }
    false
}

fn load_draft(s: &mut MainState) {
    s.group_settings.open_for = s.frame.group_settings_id;
    s.group_settings.picker_open = false;
    s.group_settings.query = TextEdit::new("");
    s.group_settings.pending = None;
    s.group_settings.error = None;
    let Some(id) = s.frame.group_settings_id else {
        s.group_settings.blur_inputs();
        return;
    };
    if let Some(group) = s
        .library_snapshot()
        .groups
        .iter()
        .find(|group| group.id == id)
        .cloned()
    {
        s.group_settings.name = TextEdit::new(&group.name);
        s.group_settings.enabled = group.is_enabled;
        s.group_settings.insertion = group.insertion;
        s.group_settings.rule = group.rule.as_ref().map(|rule| rule.mode);
        s.group_settings.bundle_ids = group
            .rule
            .as_ref()
            .map(|rule| rule.bundle_ids.clone())
            .unwrap_or_default();
    } else {
        s.group_settings.name = TextEdit::new("");
        s.group_settings.enabled = true;
        s.group_settings.insertion = None;
        s.group_settings.rule = None;
        s.group_settings.bundle_ids.clear();
    }
    s.group_settings.name.focused = true;
}

fn request_icons(s: &mut MainState) {
    let installed = s.svc.apps.installed().get();
    let needed = app_picker::icons_needed(
        &s.group_settings.bundle_ids,
        &installed,
        &s.group_settings.query.text,
        s.group_settings.picker_open,
    );
    let apps = Arc::clone(&s.svc.apps);
    for (id, size) in needed {
        s.group_settings.icons.request(&apps, &id, size);
    }
}

fn bare_escape(ev: &KeyEvent) -> bool {
    ev.key == Key::Escape && !ev.mods.cmd && !ev.mods.ctrl && !ev.mods.alt && !ev.mods.shift
}
