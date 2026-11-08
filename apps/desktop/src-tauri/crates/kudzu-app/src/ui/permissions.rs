//! The permission companion window (`components/app/permissions/permission-center.tsx`, the
//! `/permissions` route): a small always-on-top window that walks the user through granting
//! Accessibility (and optionally Input Monitoring) in System Settings, detects the grant live,
//! and confirms it.
//!
//! ```text
//! ┌ bg-popover, 18 pt corners, 420 × 660 ─────────┐
//! │ ⛨ Permissions for Kudzu                    ✕ │  h-12
//! │        ░░ iris-soft glow ░░                   │
//! │             [ app icon ]   ← native drag view │
//! │      Let Kudzu do the typing.                 │
//! │  [ Accessibility        Required ]            │
//! │  [ Input Monitoring     Optional ]            │
//! │        status / hint (min 72)                 │
//! │  [ Open System Settings ↗ ]                   │
//! │        ⓘ Don't see the app…                   │
//! └───────────────────────────────────────────────┘
//! ```
//!
//! The OS approval itself stays in System Settings; `backend::permissions` drives the native
//! half (the draggable app icon over its slot, the guide window beside Settings). Status is
//! polled once a second while the window shows (the web's `setInterval`), as well as taken from
//! `Event::Status`. [`setup_card`] and [`permissions_ready`] (`permission-setup.tsx`) are shared
//! with Settings and onboarding.

use std::ffi::c_void;
use std::sync::OnceLock;
use std::time::Duration;

use rune::{icon, rich, row, text, Align, AppHandle, Color, Cursor, Cx, El, Key, KeyEvent, LoopSpec, Pixels, Span, Spring, SpringPair, Transition, Tx, col};

use kudzu_engine::{EngineStatus, TapMode};

use crate::backend::core::{Event, Win};
use crate::backend::permissions::{self as native_perm, PermissionKind};
use crate::backend::{commands, windows};
use crate::ui::icons;
use crate::ui::kit::button::{layer, over, tween, Button, Size, Variant, TW_EASE};
use crate::ui::theme::{motion, radius, text as ty, weight, Theme, Utilities, WeightOf};
use crate::ui::theme_for;

/// What happened after "Open System Settings" (`Handoff` in permission-center.tsx).
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub enum Handoff {
    #[default]
    Idle,
    Opening,
    Waiting,
    Error(String),
}

pub struct PermissionsUi {
    pub status: Option<EngineStatus>,
    pub selected: PermissionKind,
    pub app_name: String,
    pub accessibility_name: &'static str,
    pub handoff: Handoff,
    pub help: bool,
    /// The native drag view sits over the icon (the app runs from a bundle).
    pub drag_available: bool,
    /// Five seconds of "allowed but not connected": offer a restart.
    pub restart_offered: bool,
    restart_gen: u64,
    needs_connection: bool,
    selected_granted: bool,
    icon_hover: bool,
    /// The icon slot's last laid-out rect, in window points.
    icon_rect: Option<[f32; 4]>,
    /// A rect laid out before the NSWindow existed, still to install.
    pending_rect: Option<[f32; 4]>,
}

impl PermissionsUi {
    pub fn init() -> PermissionsUi {
        let ctx = native_perm::permission_context();
        let status = crate::backend::core::try_core().map(|c| commands::status_of(&c));
        let mut s = PermissionsUi {
            status: None,
            selected: ctx.kind,
            app_name: ctx.app_name,
            accessibility_name: ctx.accessibility_name,
            handoff: Handoff::Idle,
            help: false,
            drag_available: false,
            restart_offered: false,
            restart_gen: 0,
            needs_connection: false,
            selected_granted: false,
            icon_hover: false,
            icon_rect: None,
            pending_rect: None,
        };
        if let Some(st) = status {
            s.set_status(st);
        }
        s
    }

    fn ready(&self) -> bool {
        self.status.as_ref().is_some_and(permissions_ready)
    }

    fn granted(&self, kind: PermissionKind) -> bool {
        self.status.as_ref().is_some_and(|st| match kind {
            PermissionKind::Accessibility => st.permissions.accessibility,
            PermissionKind::InputMonitoring => st.permissions.input_monitoring,
        })
    }

    fn name(&self, kind: PermissionKind) -> &'static str {
        match kind {
            PermissionKind::Accessibility => self.accessibility_name,
            PermissionKind::InputMonitoring => "Input Monitoring",
        }
    }

    /// A new status (poll or event). True when anything the view shows changed.
    fn set_status(&mut self, status: EngineStatus) -> bool {
        if self.status.as_ref() == Some(&status) {
            return false;
        }
        self.status = Some(status);
        self.derive();
        true
    }

    /// The web's effects on `selectedGranted` and `needsConnection`.
    fn derive(&mut self) {
        let granted = self.granted(self.selected);
        if granted && !self.selected_granted {
            // `if (selectedGranted) stopPermissionGuide()`.
            native_perm::stop_guide(companion_window());
        }
        self.selected_granted = granted;
        let needs = self.selected == PermissionKind::Accessibility && granted && !self.ready();
        if needs != self.needs_connection {
            self.needs_connection = needs;
            self.restart_gen += 1;
            self.restart_offered = false;
            if needs {
                let generation = self.restart_gen;
                std::thread::spawn(move || {
                    std::thread::sleep(Duration::from_secs(5));
                    if let Some(h) = HANDLE.get() {
                        h.update_if(move |s, _| {
                            let due = s.restart_gen == generation && s.needs_connection && !s.restart_offered;
                            if due {
                                s.restart_offered = true;
                            }
                            due
                        });
                    }
                });
            }
        }
    }

    /// `kudzu://permission-show`: the companion opens for `kind`.
    fn reset(&mut self, kind: PermissionKind) {
        let ctx = native_perm::permission_context();
        self.app_name = ctx.app_name;
        self.accessibility_name = ctx.accessibility_name;
        self.selected = kind;
        self.handoff = Handoff::Idle;
        self.help = false;
        if crate::backend::core::try_core().is_some() {
            let st = commands::engine_status();
            self.status = Some(st);
        }
        self.derive();
        if let Some(r) = self.pending_rect.or(self.icon_rect) {
            self.install_icon(r);
        }
    }

    fn fail(&mut self, e: String) {
        self.handoff = Handoff::Error(e);
    }

    fn choose(&mut self, kind: PermissionKind) {
        self.selected = kind;
        self.handoff = Handoff::Idle;
        native_perm::stop_guide(companion_window());
        self.derive();
    }

    fn close(&mut self) {
        native_perm::stop_guide(companion_window());
        if let Some(core) = crate::backend::core::try_core() {
            core.shell().hide(Win::Permissions);
            windows::show_main();
        }
    }

    fn begin(&mut self) {
        self.handoff = Handoff::Opening;
        let kind = self.selected;
        // Next turn, so "Opening Settings…" draws first, as the web's await does.
        if let Some(h) = HANDLE.get() {
            h.update(move |s, _| match native_perm::begin_permission(companion_window(), kind) {
                Ok(()) => s.handoff = Handoff::Waiting,
                Err(e) => s.fail(e),
            });
        }
    }

    fn reveal(&mut self) {
        if let Err(e) = native_perm::reveal_permission_app(companion_window()) {
            self.fail(e);
        }
    }

    fn restart(&mut self) {
        if let Err(e) = native_perm::restart_permission_app(companion_window()) {
            self.fail(e);
        }
    }

    /// `installPermissionIcon(getBoundingClientRect())`, whenever the slot moves.
    fn install_icon(&mut self, r: [f32; 4]) {
        let window = companion_window();
        if window.is_null() {
            // Not created yet (or headless): leave the rect unrecorded so the next layout,
            // poll, focus or show retries.
            self.pending_rect = Some(r);
            return;
        }
        self.pending_rect = None;
        self.icon_rect = Some(r);
        match native_perm::install_permission_icon(window, r[0] as f64, r[1] as f64, r[2] as f64, r[3] as f64) {
            Ok(available) => self.drag_available = available,
            Err(e) => self.fail(e),
        }
    }

    /// Installs the icon if a layout arrived before the window existed.
    fn retry_icon(&mut self) -> bool {
        match self.pending_rect {
            Some(r) => {
                self.install_icon(r);
                self.pending_rect.is_none()
            }
            None => false,
        }
    }

    /// `window.addEventListener("focus", refresh)`.
    fn refresh(&mut self) -> bool {
        let mut changed = self.retry_icon();
        if crate::backend::core::try_core().is_some() {
            changed |= self.set_status(commands::engine_status());
        }
        changed
    }
}

pub fn apply(s: &mut PermissionsUi, ev: &Event, _tx: &mut Tx) -> bool {
    match ev {
        Event::Status(status) => s.set_status(status.clone()),
        // `on(Events.settings, applyTheme)`: the shell refreshes every window on a theme change.
        _ => false,
    }
}

static HANDLE: OnceLock<AppHandle<PermissionsUi>> = OnceLock::new();

/// The window's handle; also starts the once-a-second status poll that runs while it shows.
pub fn set_handle(h: AppHandle<PermissionsUi>) {
    if HANDLE.set(h.clone()).is_err() {
        return;
    }
    let _ = std::thread::Builder::new().name("kudzu-permission-poll".into()).spawn(move || {
        loop {
            std::thread::sleep(Duration::from_secs(1));
            if !h.is_visible() || crate::backend::core::try_core().is_none() {
                continue;
            }
            let status = commands::engine_status();
            h.update_if(move |s, _| {
                let installed = s.retry_icon();
                s.set_status(status) || installed
            });
        }
    });
}

/// Opens the companion for `kind` (`show_permissions`).
pub fn show(kind: PermissionKind) {
    *native_perm::SELECTION.lock() = kind;
    if let Some(h) = HANDLE.get() {
        h.update(move |s, _| s.reset(kind));
        h.open();
    }
}

/// The window closed (⌘W or the shell): stop the Settings guide.
/// The window gained or lost focus: refresh status at once on focus, as the web did.
pub fn on_focus(s: &mut PermissionsUi, focused: bool, _tx: &mut Tx) {
    if focused {
        s.refresh();
    }
}

pub fn on_close(_s: &mut PermissionsUi, _tx: &mut Tx) {
    native_perm::stop_guide(companion_window());
}

/// The companion's `NSWindow`, found by its title (the window is built in `ui::run`).
#[cfg(target_os = "macos")]
fn companion_window() -> *mut c_void {
    use objc2::msg_send;
    use objc2::runtime::{AnyClass, AnyObject};
    use objc2_foundation::NSString;
    if rune::is_headless() {
        return std::ptr::null_mut();
    }
    // SAFETY: plain AppKit messaging on the main thread (Rune handlers and updates run there).
    unsafe {
        let Some(class) = AnyClass::get(c"NSApplication") else { return std::ptr::null_mut() };
        let app: *mut AnyObject = msg_send![class, sharedApplication];
        if app.is_null() {
            return std::ptr::null_mut();
        }
        let list: *mut AnyObject = msg_send![app, windows];
        if list.is_null() {
            return std::ptr::null_mut();
        }
        let n: usize = msg_send![list, count];
        for i in 0..n {
            let w: *mut AnyObject = msg_send![list, objectAtIndex: i];
            if w.is_null() {
                continue;
            }
            let title: *mut NSString = msg_send![w, title];
            if !title.is_null() && (*title).to_string() == TITLE {
                return w as *mut c_void;
            }
        }
    }
    std::ptr::null_mut()
}

#[cfg(not(target_os = "macos"))]
fn companion_window() -> *mut c_void {
    std::ptr::null_mut()
}

/// The window's title (`AppSpec::new` in `ui::run`).
const TITLE: &str = "Kudzu permissions";

/// `/icon.png`: the app's artwork, decoded once.
pub fn app_icon() -> &'static Pixels {
    static ICON: OnceLock<Pixels> = OnceLock::new();
    ICON.get_or_init(|| {
        let p = Pixels::new();
        if let Ok(img) = image::load_from_memory(include_bytes!("../../../../../public/icon.png")) {
            let rgba = img.to_rgba8();
            let (w, h) = rgba.dimensions();
            p.set(w, h, rgba.into_raw());
        }
        p
    })
}

// MARK: View

const fn sans(size: f32, lh: f32) -> rune::TextStyle {
    rune::TextStyle::new(size, weight::NORMAL).lh(lh)
}
/// `text-[10px]` (the body's `leading-5`).
const XXS: rune::TextStyle = sans(10.0, 20.0);
/// `text-[11px] leading-relaxed`.
const XS2_RELAXED: rune::TextStyle = sans(11.0, 17.875);
/// `text-[11px]` (the body's `leading-5`).
const XS2: rune::TextStyle = sans(11.0, 20.0);
/// `text-xs leading-relaxed`.
const XS_RELAXED: rune::TextStyle = sans(12.0, 19.5);
/// `text-[13px] leading-relaxed`.
const BODY: rune::TextStyle = sans(13.0, 21.125);
/// `text-[23px] font-semibold tracking-[-0.035em]` (the body's `leading-5`).
const HEADLINE: rune::TextStyle = rune::TextStyle::new(23.0, weight::SEMIBOLD).lh(20.0).tracking(-0.805);

/// `.permission-success`: up 4 pt from 97 %, fading in, 240 ms ease-out.
fn success_in() -> Transition {
    let spring = Spring::new(0.24, 0.0);
    Transition::offset(0.0, 4.0).combined(Transition::scale(0.97)).springs(SpringPair::new(spring, spring))
}

fn spinner(size: f32, c: Color) -> El {
    icon(&icons::LOADER_CIRCLE, size).color(c).looping(LoopSpec::rotation(1.0)).a11y_hidden()
}

pub fn view(s: &PermissionsUi, cx: &mut Cx<PermissionsUi>) -> El {
    let th = theme_for(cx);
    let t = &th.t;
    let ready = s.ready();
    let granted = s.selected_granted;
    let needs = s.needs_connection;
    let app = s.app_name.clone();

    let headline = if ready {
        "You’re all set."
    } else if needs {
        "Access is allowed."
    } else {
        "Let Kudzu do the typing."
    };
    let detail = match &s.status {
        None => "Checking access on this Mac…",
        Some(st) if ready => {
            if st.can_expand {
                "Your shortcuts can now become full sentences."
            } else if st.paused {
                "Permissions are set. Expansion is currently paused."
            } else {
                "Permissions are set. Close the secure input field to expand."
            }
        }
        Some(_) if needs => "One last step to connect Kudzu to your keyboard.",
        Some(_) => "A little permission to turn your triggers into text, wherever you write.",
    };

    // Header: `flex h-12 items-center gap-2 px-5 text-xs text-muted-foreground`.
    let header = row()
        .key("perm-header")
        .h(48.0)
        .center_y()
        .gap(8.0)
        .px(20.0)
        .window_drag()
        .child(icon(&icons::SHIELD_CHECK, 14.0).color(t.muted_foreground).a11y_hidden())
        .child(text(format!("Permissions for {app}")).font(ty::XS).color(t.muted_foreground).grow(1.0).min_w(0.0))
        .child(
            Button::icon_only(&icons::X, "Close permission setup")
                .variant(Variant::Ghost)
                .height(24.0)
                .radius(radius::FULL)
                .icon_size(14.0)
                .on_press(|s: &mut PermissionsUi| s.close())
                .build(th),
        );

    // The glow: `absolute inset-x-0 top-0 h-44 radial-gradient(ellipse at top, iris-soft,
    // transparent 70%)`. CSS's default `farthest-corner` ellipse over 420 × 176 has radii
    // √2 × (210, 176).
    let glow = rune::el()
        .key("perm-glow")
        .pin(Some(0.0), Some(0.0), Some(0.0), None)
        .h(176.0)
        .radial_gradient([0.5, 0.0], [210.0 * std::f32::consts::SQRT_2, 176.0 * std::f32::consts::SQRT_2], [(0.0, t.iris_soft), (0.7, t.iris_soft.alpha(0.0))])
        .a11y_hidden()
        .inert();

    // The app icon: a button the native drag view covers once installed.
    let icon_button = rune::el()
        .key("perm-app-icon")
        .size(80.0, 80.0)
        .radius(20.0)
        .role("button")
        .label(format!("Show {app} in Finder to add it to System Settings"))
        .help("Drag this app into System Settings")
        .cursor(Cursor::Grab)
        .focusable(true)
        .focus_ring(2.0, 4.0, t.ring)
        .scale(if s.icon_hover { 1.05 } else { 1.0 })
        .scale_motion(tween(0.15, TW_EASE))
        .on_hover_change(|s: &mut PermissionsUi, hover, _| s.icon_hover = hover)
        .on_layout(|s: &mut PermissionsUi, r| {
            if s.icon_rect != Some(r) || !s.drag_available {
                s.install_icon(r);
            }
        })
        .on_press(|s: &mut PermissionsUi| s.reveal())
        .child(rune::el().size(80.0, 80.0).pixels(app_icon()).a11y_hidden());
    let hint = row()
        .mt(8.0)
        .h(16.0)
        .center_y()
        .gap(4.0)
        .child(icon(&icons::MOUSE_POINTER_2, 12.0).color(t.muted_foreground).a11y_hidden())
        .child(
            text(if s.drag_available { "Drag into System Settings to add the app" } else { "Your app. Your permission." })
                .font(XXS)
                .color(t.muted_foreground),
        );
    let intro = col()
        .key("perm-intro")
        .w_full()
        .pt(12.0)
        .items_center()
        .child(icon_button)
        .child(hint)
        .child(text(headline).font(HEADLINE).color(t.popover_foreground).mt(20.0).align(Align::Center))
        .child(text(detail).font(BODY).color(t.muted_foreground).mt(8.0).max_w(310.0).wrap().align(Align::Center));

    // The two permissions.
    let items = col()
        .key("perm-items")
        .role("group")
        .label("Permissions")
        .w_full()
        .mt(24.0)
        .gap(8.0)
        .child(permission_item(s, th, PermissionKind::Accessibility))
        .child(permission_item(s, th, PermissionKind::InputMonitoring));

    let status_area = col().key("perm-status").w_full().mt(16.0).min_h(72.0).items_center().child(status_view(s, th));

    // The action.
    let action = if ready && granted {
        Button::new("Back to Kudzu").icon_after(&icons::CHECK).height(40.0).full_width().on_press(|s: &mut PermissionsUi| s.close()).build(th)
    } else if needs && s.restart_offered {
        Button::new(format!("Restart {app}")).height(40.0).full_width().on_press(|s: &mut PermissionsUi| s.restart()).build(th)
    } else if s.selected == PermissionKind::InputMonitoring && granted {
        Button::new("Set up app control")
            .icon_after(&icons::ARROW_UP_RIGHT)
            .height(40.0)
            .full_width()
            .on_press(|s: &mut PermissionsUi| s.choose(PermissionKind::Accessibility))
            .build(th)
    } else {
        let opening = s.handoff == Handoff::Opening;
        Button::new(if opening { "Opening Settings…" } else { "Open System Settings" })
            .id("perm-open-settings")
            .icon_after(&icons::ARROW_UP_RIGHT)
            .height(40.0)
            .full_width()
            .disabled(s.status.is_none() || opening)
            .on_press(|s: &mut PermissionsUi| s.begin())
            .build(th)
    };
    let help_toggle = help_toggle(s, th);
    let bottom = col().key("perm-actions").w_full().gap(8.0).pt(8.0).child(action).child(help_toggle);
    let footer = text("Your typing is never recorded or sent anywhere.").font(XXS).color(t.faint).mt(12.0).align(Align::Center).w_full();

    let mut body = col()
        .key("perm-body")
        .grow(1.0)
        .min_h(0.0)
        .px(24.0)
        .pb(20.0)
        .child(glow)
        .child(intro)
        .child(items)
        .child(status_area)
        .child(rune::el().grow(1.0))
        .child(bottom)
        .child(footer);
    if s.help {
        body = body.child(help_panel(s, th));
    }

    // `section`: the whole window (`vibrant:` drops the browser preview's padding and border).
    col()
        .key("perm-root")
        .role("dialog")
        .label("Kudzu permissions")
        .w_full()
        .h_full()
        .radius(18.0)
        .clip_rounded(18.0)
        .fill(t.popover)
        .elevate(&t.elev_raised)
        .on_window_focus(|s: &mut PermissionsUi, focused, tx| on_focus(s, focused, tx))
        .on_key(|s: &mut PermissionsUi, ev: &KeyEvent, _| {
            if ev.key == Key::Escape {
                s.close();
                true
            } else {
                false
            }
        })
        .child(header)
        .child(body)
}

/// One permission row (`Item variant="outline" size="sm"` as a toggle button).
fn permission_item(s: &PermissionsUi, th: &Theme, kind: PermissionKind) -> El {
    let t = &th.t;
    let granted = s.granted(kind);
    let selected = s.selected == kind;
    let key = match kind {
        PermissionKind::Accessibility => "ax",
        PermissionKind::InputMonitoring => "im",
    };
    let r = radius::XL;
    let (border, fill) = if selected {
        // Opaque over the popover: CSS paints outer shadows only outside the box, so the
        // translucent fill mustn't show `shadow-card` through it.
        (Theme::tint(t.iris, 0.4), over(Theme::tint(t.iris_soft, 0.5), t.popover))
    } else {
        (t.hairline, Theme::tint(t.surface, 0.5))
    };
    let media = rune::el()
        .size(32.0, 32.0)
        .center()
        .radius(radius::LG)
        .fill(if granted { Theme::tint(t.success, 0.1) } else { Theme::tint(t.foreground, 0.05) })
        .motion(Spring::new(0.1, 0.0))
        .child(
            icon(if kind == PermissionKind::Accessibility { &icons::SHIELD_CHECK } else { &icons::KEYBOARD }, 16.0)
                .color(if granted { t.success } else { t.muted_foreground })
                .a11y_hidden(),
        );
    let content = col()
        .grow(1.0)
        .min_w(0.0)
        .gap(2.0)
        .child(text(s.name(kind)).font(rune::TextStyle::new(13.0, weight::MEDIUM).lh(17.875)).color(t.popover_foreground))
        .child(
            text(if kind == PermissionKind::Accessibility { "Insert snippets into other apps." } else { "Additional keyboard monitoring access." })
                .font(rune::TextStyle::new(11.0, weight::NORMAL).lh(16.5))
                .color(t.muted_foreground)
                .wrap()
                .max_lines(2),
        );
    let state_font = rune::TextStyle::new(10.0, weight::MEDIUM).lh(20.0);
    let actions = if granted {
        row()
            .gap(4.0)
            .center_y()
            .child(icon(&icons::CHECK, 14.0).color(t.success).key(format!("perm-allowed-{key}")).transition(check_in()).a11y_hidden())
            .child(text("Allowed").key("label").font(state_font).color(t.success))
    } else {
        row().gap(4.0).center_y().child(
            text(if kind == PermissionKind::Accessibility { "Required" } else { "Optional" }).key("label").font(state_font).color(t.muted_foreground),
        )
    };
    let tw = tween(0.1, TW_EASE);
    let mut e = row()
        .key(format!("perm-item-{key}"))
        .w_full()
        .center_y()
        .gap(12.0)
        .px(15.0)
        .py(13.0)
        .radius(r)
        .fill(fill)
        .border(1.0, border)
        .motion(Spring::new(0.1, 0.0))
        .role("button")
        .label(s.name(kind))
        .checked(selected)
        .focusable(true)
        .focus_ring(3.0, 0.0, Theme::tint(t.ring, 0.5));
    if selected {
        e = e.elevate(&t.elev_card);
    } else {
        e = e.hover_group().hover_motion(tw, tw).child(layer(0.0).key("hover").radius(r).fill(t.surface).group_opacity(0.0, 1.0));
    }
    e.child(layer(0.0).key("focus-border").radius(r).focus_ring(1.0, -1.0, t.ring))
        .child(media.key("media"))
        .child(content.key("content"))
        .child(actions.key("actions"))
        .on_press(move |s: &mut PermissionsUi| s.choose(kind))
}

/// The message under the permissions (`aria-live`).
fn status_view(s: &PermissionsUi, th: &Theme) -> El {
    let t = &th.t;
    let ready = s.ready();
    let granted = s.selected_granted;
    let medium_xs = ty::XS.weight_of(weight::MEDIUM);
    if let Handoff::Error(message) = &s.handoff {
        return text(message.clone())
            .key("perm-error")
            .role("alert")
            .font(XS_RELAXED)
            .color(t.destructive)
            .wrap()
            .align(Align::Center)
            .w_full()
            .padding(12.0)
            .radius(radius::LG)
            .fill(Theme::tint(t.destructive, 0.1));
    }
    if ready && granted {
        let line = s
            .status
            .as_ref()
            .map(|st| if st.can_expand { "Ready to expand in your other apps.".to_string() } else { st.summary.clone() })
            .unwrap_or_default();
        return col()
            .key("perm-success")
            .items_center()
            .gap(8.0)
            .py(8.0)
            .transition(success_in())
            .child(
                row()
                    .center_y()
                    .gap(6.0)
                    .child(icon(&icons::CHECK, 16.0).color(t.success).a11y_hidden())
                    .child(text("Access confirmed").font(medium_xs).color(t.success)),
            )
            .child(text(line).font(XS2).color(t.muted_foreground).wrap().align(Align::Center));
    }
    if granted {
        let line = if s.needs_connection {
            if s.restart_offered {
                format!("macOS may need a restart of {} to apply access. We’ll bring you back here.", s.app_name)
            } else {
                "Connecting to your keyboard…".to_string()
            }
        } else {
            format!("Next, allow {} so Kudzu can insert text.", s.accessibility_name)
        };
        return col()
            .key("perm-granted")
            .items_center()
            .gap(8.0)
            .py(4.0)
            .child(
                row()
                    .center_y()
                    .gap(6.0)
                    .child(icon(&icons::CHECK, 16.0).color(t.success).a11y_hidden())
                    .child(text("Access confirmed").font(medium_xs).color(t.success)),
            )
            .child(text(line).font(XS2_RELAXED).color(t.muted_foreground).wrap().align(Align::Center));
    }
    if s.handoff == Handoff::Waiting {
        let first = format!("Turn on {} in {}.", s.app_name, s.name(s.selected));
        let strong_start = "Turn on ".len();
        let strong_end = strong_start + s.app_name.len();
        let all = format!("{first}\nIf it isn’t listed, drag the icon into the list.");
        let spans = vec![Span::new(strong_start, strong_end).weight(weight::MEDIUM).color(t.popover_foreground)];
        return col()
            .key("perm-waiting")
            .items_center()
            .gap(8.0)
            .py(4.0)
            .child(
                row()
                    .center_y()
                    .gap(6.0)
                    .child(spinner(14.0, t.popover_foreground))
                    .child(text("Waiting for macOS").font(medium_xs).color(t.popover_foreground)),
            )
            .child(rich(all, spans).font(XS2_RELAXED).color(t.muted_foreground).wrap().align(Align::Center));
    }
    let line = if granted {
        "Access is allowed. If typing is still unavailable, quit and reopen Kudzu after checking the switch."
    } else {
        "We’ll open the right page and stay beside you. macOS may ask you to confirm with Touch ID or your password."
    };
    text(line).key("perm-hint").font(XS2_RELAXED).color(t.muted_foreground).wrap().align(Align::Center).w_full().px(12.0).py(8.0)
}

/// "Don't see the app, or already allowed?" / "Hide help".
fn help_toggle(s: &PermissionsUi, th: &Theme) -> El {
    let t = &th.t;
    let label = if s.help { "Hide help" } else { "Don’t see the app, or already allowed?" };
    let tw = tween(crate::ui::theme::MOTION_FAST, TW_EASE);
    row()
        .key("perm-help-toggle")
        .w_full()
        .center()
        .gap(6.0)
        .py(6.0)
        .radius(radius::MD)
        .role("button")
        .label(label)
        .expanded(s.help)
        .focusable(true)
        .focus_ring(2.0, 0.0, t.ring)
        .hover_group()
        .hover_motion(tw, tw)
        .child(icon(&icons::CIRCLE_QUESTION_MARK, 14.0).color(t.muted_foreground).group_color(t.foreground).a11y_hidden())
        .child(text(label).font(XS2).color(t.muted_foreground).group_color(t.foreground))
        .on_press(|s: &mut PermissionsUi| s.help = !s.help)
}

/// The help card over the bottom of the window.
fn help_panel(s: &PermissionsUi, th: &Theme) -> El {
    let t = &th.t;
    let app = s.app_name.clone();
    let steps = [
        format!("Open Privacy & Security → {}.", s.name(s.selected)),
        format!("Drag the app icon into the list, or use the + button to select {app}."),
        "Turn on its switch and approve the macOS prompt.".to_string(),
    ];
    let list = col().role("list").mt(8.0).gap(8.0).children(steps.into_iter().enumerate().map(|(i, step)| {
        row()
            .role("listitem")
            .items_start()
            .child(rune::el().w(16.0).child(text(format!("{}.", i + 1)).font(XS_RELAXED).color(t.muted_foreground).align(Align::End).w_full().pr(4.0)))
            .child(text(step).font(XS_RELAXED).color(t.muted_foreground).wrap().grow(1.0).min_w(0.0))
    }));
    let head = row()
        .center_y()
        .justify_between()
        .child(text(format!("Add {app} to the list")).font(ty::BASE.weight_of(weight::SEMIBOLD)).color(t.popover_foreground))
        .child(
            Button::icon_only(&icons::X, "Close help")
                .variant(Variant::Ghost)
                .height(24.0)
                .icon_size(14.0)
                .on_press(|s: &mut PermissionsUi| s.help = false)
                .build(th),
        );
    col()
        .key("perm-help")
        .pin(Some(16.0), None, Some(16.0), Some(16.0))
        .padding(17.0)
        .radius(radius::XL)
        .fill(t.popover)
        .border(1.0, t.hairline)
        .elevate(&t.elev_raised)
        .transition(Transition::opacity())
        .child(head)
        .child(list)
        .child(
            text(format!("Already on? Quit and reopen {app} if macOS hasn’t applied the change. Make sure you enabled this copy of the app."))
                .font(XS_RELAXED)
                .color(t.muted_foreground)
                .wrap()
                .mt(12.0),
        )
        .child(
            rune::el().mt(12.0).w_full().child(
                Button::new("Show this app in Finder")
                    .variant(Variant::Outline)
                    .size(Size::Sm)
                    .icon(&icons::FOLDER_OPEN)
                    .full_width()
                    .on_press(|s: &mut PermissionsUi| s.reveal())
                    .build(th),
            ),
        )
        .child(
            rune::el().mt(4.0).w_full().child(
                Button::new(format!("Restart {app}"))
                    .id("perm-help-restart")
                    .variant(Variant::Ghost)
                    .size(Size::Sm)
                    .full_width()
                    .on_press(|s: &mut PermissionsUi| s.restart())
                    .build(th),
            ),
        )
}

// MARK: Shared with Settings and onboarding (`permission-setup.tsx`)

/// `permissionsReady`: Accessibility granted, events can be posted and the tap is live.
pub fn permissions_ready(status: &EngineStatus) -> bool {
    status.permissions.accessibility && status.can_post_events && status.tap != TapMode::Off
}

/// `animate-check` (`check-in`): fades in from 40 % scale on `--spring-pop`. Rune's presence
/// scale floor for small glyphs is 50 %.
pub fn check_in() -> Transition {
    Transition::scale(0.5).allow_small_scale().springs(SpringPair::new(motion::pop(), motion::pop()))
}

/// `<PermissionSetup status />`: the card that opens the companion.
pub fn setup_card<S: 'static>(th: &Theme, status: &EngineStatus) -> El {
    let t = &th.t;
    let ready = permissions_ready(status);
    let title = if ready { "Permissions are set" } else { "Enable typing in other apps" };
    let detail = if ready {
        if status.can_expand { "Kudzu is ready to expand your snippets.".to_string() } else { status.summary.clone() }
    } else {
        "A guided setup, with every step beside you.".to_string()
    };
    let badge = rune::el()
        .size(40.0, 40.0)
        .center()
        .radius(radius::XL)
        .fill(t.iris_soft)
        .transition(still())
        .child(icon(&icons::SHIELD_CHECK, 20.0).color(t.iris_foreground).a11y_hidden().transition(still()));
    let head = row()
        .center_y()
        .gap(12.0)
        .transition(still())
        .child(badge)
        .child(
            rune::col()
                .grow(1.0)
                .min_w(0.0)
                .transition(still())
                .child(text(title).font(ty::BASE.weight_of(weight::SEMIBOLD)).color(t.foreground).transition(still()))
                .child(text(detail).font(ty::XS).color(t.muted_foreground).mt(2.0).wrap().transition(still())),
        );
    let mark = |granted: bool, other: &'static rune::Icon, key: &str| -> El {
        if granted {
            icon(&icons::CHECK, 14.0).color(t.success).key(format!("perm-check-{key}")).transition(check_in()).a11y_hidden()
        } else {
            icon(other, 14.0).color(t.muted_foreground).key(format!("perm-icon-{key}")).a11y_hidden().transition(still())
        }
    };
    let perms = row()
        .center_y()
        .gap(8.0)
        .mt(16.0)
        .mb(16.0)
        .transition(still())
        .child(mark(status.permissions.accessibility, &icons::SHIELD_CHECK, "ax"))
        .child(text("Accessibility").font(ty::XS).color(t.muted_foreground).transition(still()))
        .child(text("·").font(ty::XS).color(t.faint).mx(4.0).transition(still()))
        .child(mark(status.permissions.input_monitoring, &icons::KEYBOARD, "im"))
        .child(text("Input Monitoring").font(ty::XS).color(t.muted_foreground).transition(still()));
    let button = Button::new(if ready { "Review permissions" } else { "Set up permissions" })
        .variant(if ready { Variant::Outline } else { Variant::Default })
        .icon_after(&icons::ARROW_UP_RIGHT)
        .full_width()
        .on_press(|_: &mut S| show(PermissionKind::Accessibility))
        .build(th);
    rune::col().key("permission-setup").radius(radius::XL).fill(t.surface).padding(16.0).elevate(&t.elev_card).child(head).child(perms).child(button)
}

/// No presence fade: the card's parts appear with it, as the web's do (only the checks animate).
fn still() -> Transition {
    Transition::none()
}
