//! Group settings: name, on/off, insertion and the app rule (`PLAN.md` S55, RUL-7, RUL-8).
//!
//! The panel is composed here. `kit_ext::dialog` keeps Close in the header row, so the heading
//! cannot be the reference 488 pt. Save stays open with "Saving…" until the writer reports the
//! revision durable. A failed write stays open and says the change is kept and will be retried.

mod commit;

use kudzu_app::watch::Subscription;
use kudzu_core::library::model::{AppRule, AppRuleMode, InsertionMethod};
use rune::{col, el, row, text, Cx, El, Key, KeyEvent, TextEdit, TextStyle};

use crate::shell::MainState;
use crate::theme::{icons, radius, Theme};
use crate::widgets::app_icon::IconCache;
use crate::widgets::app_picker::AppPicker;
use crate::widgets::kit_ext::button::{Button, Size, Variant};
use crate::widgets::kit_ext::input::Input;
use crate::widgets::kit_ext::switch::Switch;
use crate::widgets::selection_pill::{Segment, Segmented};

pub use commit::open;

const DIALOG_W: f32 = 520.0;
/// Insert and app rows. 18.5625 + 16 = 34.5625, which centres the 32 pt segmented so its
/// radios land on the reference y (the 13/500 title keeps a line shorter than the inherited 20).
const ROW_H: f32 = 34.5625;
const DESCRIPTION: &str =
    "Snippets in this group (and its subgroups) inherit these unless they set their own.";

/// Draft for the open dialog. The icon cache outlives one group so a repeat open does not
/// read the catalogue again.
pub struct State {
    open_for: Option<uuid::Uuid>,
    pub(crate) name: TextEdit,
    pub(crate) enabled: bool,
    pub(crate) insertion: Option<InsertionMethod>,
    pub(crate) rule: Option<AppRule>,
    pending: Option<commit::PendingSave>,
    pub(crate) error: Option<String>,
    pub(crate) query: TextEdit,
    picker_open: bool,
    pub(crate) picker_active: usize,
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
            pending: None,
            error: None,
            query: TextEdit::new(""),
            picker_open: false,
            picker_active: 0,
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
    changed |= commit::poll_save(s);
    if s.group_settings.open_for != s.frame.group_settings_id {
        commit::load_draft(s);
        changed = true;
    }
    if s.frame.group_settings_id.is_some() {
        commit::request_icons(s);
    }
    changed
}

pub fn view(s: &MainState, cx: &Cx<MainState>, theme: &Theme, overlays: &mut Vec<El>) {
    if s.frame.group_settings_id.is_none() {
        return;
    }
    let c = &theme.c;
    let saving = commit::saving(s);
    let installed = s.svc.apps.installed().get();
    let mut picker_overlays = Vec::new();
    let mut form = col()
        .w_full()
        .px(16.0)
        .pt(16.0)
        .gap(20.0)
        .child(header(theme))
        .child(name_row(s, theme, saving))
        .child(insert_row(s, cx, theme, saving))
        .child(apps_section(
            s,
            cx,
            theme,
            &installed,
            &mut picker_overlays,
            saving,
        ));
    if saving {
        form = form.inert();
    }
    if let Some(message) = &s.group_settings.error {
        form = form.child(
            text(message.clone())
                .key("group-settings-error")
                .role("alert")
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
            if st.group_settings.is_composing() {
                return false;
            }
            if bare_escape(ev) && st.group_settings.picker_open {
                close_picker(st);
                true
            } else {
                false
            }
        })
        .modal(true, |st: &mut MainState| {
            if st.group_settings.picker_open {
                close_picker(st);
            } else {
                dismiss(st);
            }
        })
        .child(form)
        .child(el().h(20.0).w_full())
        .child(footer(theme, saving))
        .child(close_button(theme));
    for shadow in &theme.depth.float {
        panel = panel.box_shadow(*shadow);
    }
    for mut extra in picker_overlays {
        if saving {
            extra = extra.inert();
        }
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

fn name_row(s: &MainState, theme: &Theme, saving: bool) -> El {
    let on = s.group_settings.enabled;
    let word = if on { "On" } else { "Off" };
    let mut field = Input::new(&s.group_settings.name)
        .with(|input| {
            input
                .height(36.0)
                .disabled(saving)
                .on_edit(|st: &mut MainState| &mut st.group_settings.name)
                .on_submit(|st: &mut MainState, _| commit::save(st))
                .on_key(|st: &mut MainState, ev, _| {
                    // RUNE-GAP(none; plan gap S55-escape): a focused field never reaches the
                    // panel's on_key, and the editor treats Escape as blur.
                    if st.group_settings.name.preedit().is_some()
                        || st.group_settings.name.is_composing()
                    {
                        return false;
                    }
                    if !bare_escape(ev) {
                        return false;
                    }
                    if st.group_settings.picker_open {
                        close_picker(st);
                    } else {
                        dismiss(st);
                    }
                    true
                })
        })
        // RUNE-GAP(none; plan gap S55-name): rune-kit draws a 2 point spacer and a 6 point
        // gap before the editor (3 + 2 + 6 = 11) and puts the name on that inset line.
        // The landmark sits on this 36 pt shell.
        .build(theme)
        .key("group-name")
        .label("Group name")
        .grow(1.0)
        .min_w(0.0);
    if saving {
        field = field.inert();
    }
    let mut switch = Switch::new(on)
        .label(word)
        .disabled(saving)
        .on_toggle(|st: &mut MainState| {
            st.group_settings.enabled = !st.group_settings.enabled;
        })
        .build(theme)
        .key("group-enabled");
    if saving {
        switch = switch.inert();
    }
    row()
        .w_full()
        .h(36.0)
        .items_center()
        .gap(12.0)
        .child(field)
        .child(
            row().items_center().gap(8.0).child(switch).child(
                text(word)
                    .font(TextStyle::new(12.0, 400.0).lh(16.0))
                    .color(theme.c.muted_foreground),
            ),
        )
}

fn insert_row(s: &MainState, cx: &Cx<MainState>, theme: &Theme, saving: bool) -> El {
    let mut control = Segmented::new(
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
    });
    if saving {
        control = control.inert();
    }
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
        .child(control)
}

fn apps_section(
    s: &MainState,
    cx: &Cx<MainState>,
    theme: &Theme,
    installed: &[kudzu_core::system::AppInfo],
    overlays: &mut Vec<El>,
    saving: bool,
) -> El {
    let value = rule_value(s.group_settings.rule.as_ref().map(|rule| rule.mode));
    let mut control = Segmented::new("group-rule", "App rule", value)
        .option(Segment::new("none", "All apps"))
        .option(Segment::new("allow", "Only in…"))
        .option(Segment::new("deny", "Except…"))
        .build(theme, cx, |st: &mut MainState, value| {
            commit::set_rule(
                st,
                match value {
                    "allow" => Some(AppRuleMode::Allow),
                    "deny" => Some(AppRuleMode::Deny),
                    _ => None,
                },
            );
        });
    if saving {
        control = control.inert();
    }
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
            .child(control),
    );
    if s.group_settings.rule.is_some() {
        let chosen = commit::chosen_ids(s);
        let mut picker = AppPicker::new(
            "app-picker",
            chosen,
            installed,
            &s.group_settings.query,
            &s.group_settings.icons,
        )
        .open(s.group_settings.picker_open)
        .active(s.group_settings.picker_active)
        .overlays(overlays)
        .build(
            theme,
            cx,
            |st: &mut MainState| &mut st.group_settings.query,
            commit::toggle_app,
            commit::set_picker_open,
            commit::set_picker_active,
        );
        if saving {
            picker = picker.inert();
        }
        section = section.child(picker);
    }
    section
}

fn footer(theme: &Theme, saving: bool) -> El {
    let c = &theme.c;
    let label = if saving { "Saving…" } else { "Save" };
    let mut save = Button::new(label)
        .disabled(saving)
        .on_press(|st: &mut MainState| commit::save(st))
        .build(theme)
        .key("group-settings-save")
        .default_button();
    if saving {
        save = save.inert();
    }
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
                .child(save),
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

fn dismiss(s: &mut MainState) {
    s.frame.group_settings_id = None;
    s.group_settings.pending = None;
    s.group_settings.picker_open = false;
    s.group_settings.name.focused = false;
    s.group_settings.query.focused = false;
}

fn bare_escape(ev: &KeyEvent) -> bool {
    ev.key == Key::Escape && !ev.mods.cmd && !ev.mods.ctrl && !ev.mods.alt && !ev.mods.shift
}
