//! Settings (`components/app/main/settings.tsx`): the section list, the section header and each
//! section's groups of rows.
//!
//! ```text
//! ┌ nav w-48 ──────┬ h-12 header ─────────────────────────────────┐
//! │ Settings       │ General                                      │
//! │ [General]      ├──────────────────────────────────────────────┤
//! │  Expansion     │   max-w 680, px/py 32, gap 32, .stagger        │
//! │  …             │   Group: title, description, card of Rows     │
//! └────────────────┴──────────────────────────────────────────────┘
//! ```
//!
//! Overlays (the retention select, the clear-history alert, app pickers) are collected while the
//! view runs and mounted at the window root by the frame through [`overlays`], so they escape the
//! main card's clip.

mod about;
mod agents;
mod apps;
mod clipboard;
mod data;
mod expansion;
mod general;

use std::cell::{Cell, RefCell};

use rune::{col, row, text, Cx, El, PhysicalInput, PhysicalKeyCode, ScrollSpec, TextEdit, TextStyle, Transition, Tx};

use kudzu_core::interchange::backup::BackupInfo;

use super::insights::{rise, stagger_delay, still, whole};
use super::{MainUi, SettingsSection, View};
use crate::backend::agent::AgentSetup;
use crate::backend::commands::EspansoDetection;
use crate::ui::kit::app_picker::AppPickerState;
use crate::ui::kit::button::{hover_fast, paint, track_press, Look, Skin, State};
use crate::ui::kit::select::SelectState;
use crate::ui::kit::selection_pill::SelectionPill;
use crate::ui::kit::shortcut::{accelerator_labels, Shortcut, ShortcutSize};
use crate::ui::kit::toast::Toast;
use crate::ui::theme::{radius, text as ty, weight, Theme, Utilities, WeightOf};
use crate::ui::theme_for;

/// `SETTINGS_SECTIONS`: the list in the nav. "shortcuts" is a section the store knows but the
/// nav doesn't list; it shows an empty page, as on the web.
pub const SECTIONS: [(SettingsSection, &str); 7] = [
    (SettingsSection::General, "General"),
    (SettingsSection::Expansion, "Expansion"),
    (SettingsSection::Clipboard, "Clipboard"),
    (SettingsSection::Agents, "AI agents"),
    (SettingsSection::Apps, "Apps"),
    (SettingsSection::Data, "Backups & data"),
    (SettingsSection::About, "About"),
];

/// Which shortcut a recorder edits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hotkey {
    Palette,
    Clipboard,
}

/// The page's component state (each section's `useState`s).
#[derive(Default)]
pub struct SettingsState {
    /// The section whose mount effects last ran (a section's `useEffect(…, [])`).
    mounted: Cell<Option<SettingsSection>>,
    /// The recorder waiting for a chord.
    pub recording: Option<Hotkey>,

    // Expansion
    /// The trigger prefix field.
    pub prefix: RefCell<TextEdit>,

    // Clipboard
    pub keep: SelectState,
    /// "Custom…" chosen (or the saved days match no preset).
    pub custom: Cell<bool>,
    /// The custom days field.
    pub days: RefCell<TextEdit>,
    /// Whether the days field had focus last pass (to commit on blur).
    days_focused: Cell<bool>,
    pub confirm_clear: bool,
    pub ignored: AppPickerState,

    // AI agents
    pub agent: Option<AgentSetup>,
    /// `agentAccess` the setup was last loaded for.
    agent_for: Cell<Option<bool>>,
    pub installing: bool,

    // Apps
    pub denied: AppPickerState,
    pub adding: AppPickerState,

    // Data
    pub backups: Vec<BackupInfo>,
    pub espanso: Vec<EspansoDetection>,

    // About
    pub version: Option<String>,
}

thread_local! {
    /// The overlays the last settings view built, for [`overlays`].
    static LAYER: RefCell<Vec<El>> = const { RefCell::new(Vec::new()) };
}

/// The settings overlays from this pass's view, to mount at the window root (empty unless
/// Settings is showing).
pub fn overlays(_cx: &mut Cx<MainUi>) -> El {
    let layer = LAYER.with(|l| std::mem::take(&mut *l.borrow_mut()));
    rune::el().key("settings-overlays").absolute(0.0, 0.0).w_full().h_full().children(layer)
}

fn page(s: &mut MainUi) -> &mut SettingsState {
    &mut s.settings_page
}

/// `setView({ kind: "settings", section })`.
fn go(s: &mut MainUi, section: SettingsSection) {
    s.view = View::Settings(Some(section));
}

/// A section's mount effects: what each component loaded or reset when it appeared.
fn mount(s: &MainUi, section: SettingsSection) {
    let st = &s.settings_page;
    if st.mounted.get() == Some(section) {
        return;
    }
    st.mounted.set(Some(section));
    match section {
        SettingsSection::Clipboard => {
            let days = s.settings.clipboard_retention_days;
            let preset = kudzu_core::client::clipboard::RETENTION_OPTIONS.iter().any(|(d, _)| *d == days);
            st.custom.set(!preset);
            if let Ok(mut edit) = st.days.try_borrow_mut() {
                edit.replace_text(if days == 0 { "7".to_string() } else { days.to_string() });
            }
            s.spawn(crate::backend::clipboard::clipboard_list, |s, info, _| s.clip.set(info));
        }
        SettingsSection::Agents => st.agent_for.set(None),
        SettingsSection::Data => data::reload(s, true),
        SettingsSection::About => {
            s.spawn(crate::backend::commands::app_version, |s, v, _| page(s).version = Some(v));
        }
        _ => {}
    }
}

pub fn view(s: &MainUi, cx: &mut Cx<MainUi>) -> El {
    let th = theme_for(cx);
    let t = &th.t;
    let section = match &s.view {
        View::Settings(Some(sec)) => *sec,
        _ => SettingsSection::General,
    };
    mount(s, section);
    agents::reload_if_needed(s, section);
    let mut layer: Vec<El> = Vec::new();

    // The nav: `relative isolate flex w-48 shrink-0 flex-col gap-px border-r border-hairline px-2.5`.
    let selected = SECTIONS.iter().any(|(id, _)| *id == section).then(|| format!("settings-sec-{}", key_of(section)));
    let mut nav = col()
        .key("settings-nav")
        .role("navigation")
        .label("Settings sections")
        .w(192.0)
        .h_full()
        .px(10.0)
        .gap(1.0)
        .border_sides([0.0, 1.0, 0.0, 0.0], t.hairline)
        .focus_group()
        .focus_follows()
        .child(SelectionPill::new("settings-nav", selected).radius(radius::MD).fill(t.accent).shadows(&t.elev_selected).build(cx))
        .child(
            row()
                .h(48.0)
                .px(10.0)
                .center_y()
                .window_drag()
                .child(text("Settings").font(ty::BASE.weight_of(weight::SEMIBOLD)).color(t.foreground)),
        );
    for (id, label) in SECTIONS.iter() {
        nav = nav.child(nav_item(th, *id, label, *id == section));
    }

    let title = SECTIONS.iter().find(|(id, _)| *id == section).map_or("", |(_, l)| l);
    let header = row()
        .key("settings-header")
        .h(48.0)
        .px(24.0)
        .center_y()
        .border_sides([0.0, 0.0, 1.0, 0.0], t.hairline)
        .window_drag()
        .child(text(title).font(ty::BASE.weight_of(weight::SEMIBOLD)).color(t.foreground));

    let children: Vec<El> = match section {
        SettingsSection::General => general::view(s, cx, &mut layer),
        SettingsSection::Expansion => expansion::view(s, cx),
        SettingsSection::Clipboard => clipboard::view(s, cx, &mut layer),
        SettingsSection::Agents => agents::view(s, cx),
        SettingsSection::Apps => apps::view(s, cx, &mut layer),
        SettingsSection::Data => data::view(s, cx),
        SettingsSection::About => about::view(s, cx),
        SettingsSection::Shortcuts => Vec::new(),
    };
    // `mx-auto flex max-w-[680px] flex-col gap-8 px-8 py-8 stagger`, keyed by section so a new
    // section's groups rise in.
    let body = still(col().key(format!("settings-body-{}", key_of(section))).w_full().max_w(680.0).px(32.0).py(32.0).gap(32.0))
        .children(children.into_iter().enumerate().map(|(i, c)| c.transition(rise()).enter_delay(stagger_delay(i))));
    let scroll = still(col().key("settings-scroll").w_full().grow(1.0).min_h(0.0).scroll(ScrollSpec { stick_bottom: false }))
        .child(still(col().w_full().items_center().child(body)));
    let main = still(col().key("settings-main").grow(1.0).min_w(0.0).h_full().min_h(0.0)).child(header).child(scroll);

    LAYER.with(|l| *l.borrow_mut() = layer);
    whole(row().key("settings-root").w_full().h_full().min_h(0.0)).child(nav).child(main)
}

fn key_of(section: SettingsSection) -> &'static str {
    match section {
        SettingsSection::General => "general",
        SettingsSection::Expansion => "expansion",
        SettingsSection::Clipboard => "clipboard",
        SettingsSection::Agents => "agents",
        SettingsSection::Apps => "apps",
        SettingsSection::Data => "data",
        SettingsSection::Shortcuts => "shortcuts",
        SettingsSection::About => "about",
    }
}

/// A nav button: `flex h-7 items-center rounded-md px-2.5 text-[13px] text-muted-foreground press
/// [--press-scale:0.985] hover:text-foreground`, plus `hover:bg-accent` unless current. Up and
/// Down move to the next section (the nav is a roving focus group that presses what it reaches).
fn nav_item(th: &Theme, id: SettingsSection, label: &str, current: bool) -> El {
    let t = &th.t;
    let key = format!("settings-sec-{}", key_of(id));
    let mut e = hover_fast(row())
        .key(key.clone())
        .role("button")
        .label(label.to_string())
        .h(28.0)
        .px(10.0)
        .center_y()
        .radius(radius::MD)
        .focusable(true)
        .focus_ring(2.0, 0.0, Theme::tint(t.ring, 0.5))
        .press_scale(0.985)
        .child(text(label.to_string()).key("label").font(ty::BASE).color(if current { t.foreground } else { t.muted_foreground }).group_color(t.foreground).a11y_hidden())
        .on_press(move |s: &mut MainUi| go(s, id));
    if !current {
        e = e.hover(t.accent);
    }
    e.focus_selected(current)
}

// MARK: Group and Row

/// `<Group title description>`: a titled card whose rows are divided by hairlines.
pub(super) fn group(th: &Theme, title: &str, description: Option<&str>, rows: Vec<El>) -> El {
    let t = &th.t;
    let mut head = col().px(4.0).child(text(title.to_string()).font(ty::BASE.weight_of(weight::SEMIBOLD)).color(t.foreground));
    if let Some(d) = description {
        head = head.child(text(d.to_string()).font(ty::XS).color(t.muted_foreground).wrap());
    }
    col().label(title.to_string()).gap(8.0).child(head).child(card(th, rows))
}

/// `divide-y divide-hairline rounded-xl bg-surface px-4 shadow-card`.
pub(super) fn card(th: &Theme, rows: Vec<El>) -> El {
    let t = &th.t;
    // `divide-y`: a 1 pt top border that, as in CSS, takes its own space.
    let rows = rows.into_iter().enumerate().map(|(i, r)| if i == 0 { r } else { whole(col()).pt(1.0).border_sides([1.0, 0.0, 0.0, 0.0], t.hairline).child(r) });
    whole(col()).radius(radius::XL).fill(t.surface).px(16.0).elevate(&t.elev_card).children(rows)
}

/// `<Row title description>`: `flex min-h-14 items-center gap-6 py-3`, the label beside its
/// controls (`flex shrink-0 items-center gap-2`).
pub(super) fn row_el(th: &Theme, title: impl Into<String>, description: Option<El>, controls: Vec<El>) -> El {
    let t = &th.t;
    let mut left = col().grow(1.0).min_w(0.0).child(text(title.into()).font(ty::BASE.weight_of(weight::MEDIUM)).color(t.foreground));
    if let Some(d) = description {
        left = left.child(d);
    }
    row().min_h(56.0).py(12.0).center_y().gap(24.0).child(left).child(row().center_y().gap(8.0).children(controls))
}

/// A plain `text-xs text-muted-foreground` description.
pub(super) fn desc(th: &Theme, s: impl Into<String>) -> El {
    text(s.into()).font(ty::XS).color(th.t.muted_foreground).wrap()
}

/// Inline description content: words and keycaps flowing as one sentence.
pub(super) enum Inline {
    Text(String),
    Keys(&'static str),
}

/// A `text-xs text-muted-foreground` sentence with inline `<Shortcut spoken />`s, wrapping
/// between words as inline content does.
pub(super) fn sentence(th: &Theme, parts: Vec<Inline>) -> El {
    let t = &th.t;
    let mut e = row().flex_wrap().center_y().min_w(0.0);
    for p in parts {
        match p {
            Inline::Text(s) => {
                let lead = s.starts_with(' ');
                let trail = s.ends_with(' ');
                let words: Vec<&str> = s.split_whitespace().collect();
                let n = words.len();
                for (i, w) in words.into_iter().enumerate() {
                    let pl = if i == 0 && lead { SPACE } else { 0.0 };
                    let pr = if i + 1 < n || trail { SPACE } else { 0.0 };
                    e = e.child(text(w.to_string()).font(ty::XS).color(t.muted_foreground).pl(pl).pr(pr));
                }
            }
            // An inline-flex keycap on the baseline grows its line box by about a point.
            Inline::Keys(combo) => e = e.child(Shortcut::new(combo).spoken().build(th).my(0.5)),
        }
    }
    e
}

/// A space at `text-xs` in SF.
const SPACE: f32 = 3.3;

// MARK: Hotkey recorder

/// `<HotkeyRecorder>`: shows the shortcut as keycaps; pressed, it waits for a chord ("Press
/// shortcut…", pulsing) and saves it, Escape or leaving cancels.
pub(super) fn hotkey_recorder(s: &MainUi, th: &Theme, which: Hotkey, label: &str) -> El {
    let t = &th.t;
    let value = match which {
        Hotkey::Palette => &s.settings.palette_hotkey,
        Hotkey::Clipboard => &s.settings.clipboard_hotkey,
    };
    let recording = s.settings_page.recording == Some(which);
    let id = format!("hotkey-{which:?}");
    let caps = accelerator_labels(value);
    let name = if recording { "Press the new shortcut, or Escape to cancel".to_string() } else { format!("{label}: {}. Press to change.", caps.join(" ")) };
    let mut e = hover_fast(row()).key(id.clone()).role("button").label(name).h(32.0).min_w(144.0).px(12.0).gap(4.0).center().radius(radius::LG).focusable(true);
    if recording {
        e = e
            .fill(t.iris_soft)
            .outline(2.0, 0.0, Theme::tint(t.iris, 0.6))
            .child(text("Press shortcut…").font(ty::XS).color(t.iris_foreground).looping(rune::LoopSpec::opacity(2.0, 0.0, 0.5, 1.0)));
    } else {
        let skin = Skin {
            shadows: t.elev_raised.clone(),
            hover_shadows: Some(t.elev_raised_hover.clone()),
            pressed_shadows: Some(t.elev_pressed.clone()),
            sheen: Some(false),
            focus_border: Some(t.ring),
            focus_ring: Some(Theme::tint(t.ring, 0.5)),
            ..Skin::flat(radius::LG, t.surface_raised)
        };
        let look = Look::of(State::Rest, crate::ui::kit::button::is_pressed(&id), true);
        e = paint(e, &skin, look).child(Shortcut::keys(caps).size(ShortcutSize::Md).build(th).a11y_hidden());
    }
    e = e
        .on_press(move |s: &mut MainUi| page(s).recording = Some(which))
        .on_physical_input(move |s: &mut MainUi, input, tx| record(s, which, input, tx));
    track_press(e, id)
}

fn record(s: &mut MainUi, which: Hotkey, input: &PhysicalInput, tx: &mut Tx) -> bool {
    if s.settings_page.recording != Some(which) {
        return false;
    }
    let PhysicalInput::Key { key, mods, pressed, repeat } = *input else {
        if matches!(input, PhysicalInput::FocusLost) {
            page(s).recording = None;
        }
        return matches!(input, PhysicalInput::Modifiers(_));
    };
    if !pressed || repeat {
        return true;
    }
    use PhysicalKeyCode::*;
    if matches!(key, ShiftLeft | ShiftRight | ControlLeft | ControlRight | AltLeft | AltRight | SuperLeft | SuperRight | Fn | FnLock) {
        return true;
    }
    if key == Escape {
        page(s).recording = None;
        return true;
    }
    let ev = kudzu_core::client::keys::KeyEvent {
        key: String::new(),
        code: format!("{key:?}"),
        meta: mods.logo,
        ctrl: mods.ctrl,
        alt: mods.alt,
        shift: mods.shift,
    };
    if let Some(accel) = kudzu_core::client::keys::accelerator_from_event(&ev, cfg!(target_os = "macos")) {
        page(s).recording = None;
        set_hotkey(s, which, accel);
    }
    let _ = tx;
    true
}

fn set_hotkey(s: &mut MainUi, which: Hotkey, accel: String) {
    let result = match which {
        Hotkey::Palette => crate::backend::commands::set_palette_hotkey(accel),
        Hotkey::Clipboard => crate::backend::clipboard::set_clipboard_hotkey(accel),
    };
    match result {
        Ok(settings) => {
            s.settings = std::sync::Arc::new(settings);
            s.toasts.push(Toast::success("Shortcut updated"));
        }
        Err(e) => s.toasts.push(Toast::error("That shortcut is taken").description(e)),
    }
}

// MARK: Small shared pieces

/// `<Loader2 className="animate-spin" />`.
pub(super) fn spinner(size: f32, c: rune::Color) -> El {
    rune::icon(&crate::ui::icons::LOADER_CIRCLE, size).color(c).looping(rune::LoopSpec::rotation(1.0)).a11y_hidden()
}

/// `text-[11px]` under the body's 20 pt line.
pub(super) const XS2_BODY: TextStyle = ty::XS2.lh(20.0);

/// `animate-check` on a small glyph.
pub(super) fn check_in() -> Transition {
    crate::ui::permissions::check_in()
}
