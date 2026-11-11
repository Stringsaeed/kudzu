//! The main window's scene: Kudzu's shell (toolbar band, 224 pt sidebar, a rounded card) over
//! one window-wide material, as the Tauri window has it (`bg-sidebar` on the root, the card
//! opaque on top).

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use rune::{col, el, oklch, oklcha, row, text, Color, Cx, El, TextStyle};
use rune_kit::shell::{PaneState, SplitView, Toolbar};
use rune_kit::Kit;

/// What sits behind the sidebar's translucent fill.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Material {
    /// `SystemMaterial::Popover` on a window-wide element (gap 3's plan).
    Popover,
    /// No Rune material: the probe puts a `NSVisualEffectView` (`sidebar`, behind window,
    /// follows window state) under the content view. This is what Tauri's `windowEffects` made.
    NativeSidebar,
}

pub struct State {
    pub material: Material,
    pub sidebar: PaneState,
    /// Bumped to force a rebuild, so the probe can read what the view sees now.
    pub touch: u32,
}

impl State {
    pub fn new() -> Self {
        Self {
            material: Material::Popover,
            sidebar: PaneState::new(224.0),
            touch: 0,
        }
    }
}

/// What the last built view saw, for the probe to read from another thread.
pub static SAW_DARK: AtomicBool = AtomicBool::new(false);
pub static SAW_INSET_X10: AtomicU32 = AtomicU32::new(0);
pub static BUILDS: AtomicU32 = AtomicU32::new(0);

struct Ink {
    fg: Color,
    muted: Color,
    faint: Color,
    card: Color,
    accent_pill: Color,
    sidebar_fill: Color,
    hairline: Color,
}

fn ink(dark: bool) -> Ink {
    if dark {
        Ink {
            fg: oklch(0.985, 0.001, 106.423),
            muted: oklch(0.709, 0.01, 56.259),
            faint: oklch(0.62, 0.01, 56.0),
            card: oklch(0.147, 0.004, 49.25),
            accent_pill: oklch(0.268, 0.007, 34.298),
            sidebar_fill: oklcha(0.216, 0.006, 56.043, 0.55),
            hairline: oklcha(1.0, 0.0, 0.0, 0.10),
        }
    } else {
        Ink {
            fg: oklch(0.147, 0.004, 49.25),
            muted: oklch(0.553, 0.013, 58.071),
            faint: oklch(0.65, 0.01, 58.0),
            card: oklch(1.0, 0.0, 0.0),
            accent_pill: oklch(0.97, 0.001, 106.424),
            sidebar_fill: oklcha(0.985, 0.001, 106.423, 0.72),
            hairline: oklch(0.923, 0.003, 48.717),
        }
    }
}

fn nav_row(label: &str, count: &str, selected: bool, i: &Ink) -> El {
    let weight = if selected { 500.0 } else { 400.0 };
    let mut r = row()
        .w_full()
        .h(28.0)
        .px(8.0)
        .gap(8.0)
        .center_y()
        .radius(6.0)
        .child(el().size(16.0, 16.0).radius(4.0).fill(if selected {
            oklch(0.623, 0.214, 259.815)
        } else {
            i.faint
        }))
        .child(
            text(label.to_owned())
                .font(TextStyle::new(13.0, weight))
                .color(i.fg)
                .grow(1.0),
        )
        .child(
            text(count.to_owned())
                .font(TextStyle::new(12.0, 400.0))
                .color(i.muted),
        );
    if selected {
        r = r.fill(i.accent_pill);
    }
    r
}

fn heading(label: &str, i: &Ink) -> El {
    text(label.to_owned())
        .font(TextStyle::new(12.0, 500.0))
        .color(i.muted)
        .px(8.0)
        .pt(14.0)
        .pb(4.0)
}

fn sidebar(i: &Ink) -> El {
    col()
        .key("sidebar-content")
        .w_full()
        .h_full()
        .px(8.0)
        .child(
            text("kudzu")
                .font(TextStyle::new(30.0, 800.0))
                .color(i.fg)
                .px(8.0)
                .pt(4.0)
                .pb(8.0),
        )
        .child(heading("Snippets", i))
        .child(nav_row("All snippets", "18", true, i))
        .child(nav_row("Favorites", "3", false, i))
        .child(nav_row("Recently used", "12", false, i))
        .child(nav_row("Turned off", "1", false, i))
        .child(heading("Groups", i))
        .child(nav_row("Work", "5", false, i))
        .child(nav_row("Developer", "4", false, i))
        .child(nav_row("Personal", "2", false, i))
        .child(heading("Clipboard", i))
        .child(nav_row("History", "85", false, i))
        .child(nav_row("Pinned", "2", false, i))
        .child(nav_row("Images", "3", false, i))
        .child(el().grow(1.0))
        .child(nav_row("Insights", "", false, i))
        .child(nav_row("Settings", "", false, i).mb(10.0))
}

fn card(i: &Ink) -> El {
    col()
        .key("card")
        .w_full()
        .padding(28.0)
        .gap(14.0)
        .child(
            text("Email signature")
                .font(TextStyle::new(22.0, 600.0))
                .color(i.fg),
        )
        .child(
            text("Triggers: ;sig")
                .font(TextStyle::new(13.0, 400.0))
                .color(i.muted),
        )
        .child(el().w_full().h(1.0).fill(i.hairline))
        .child(
            text("Best regards,")
                .font(TextStyle::new(13.5, 400.0))
                .color(i.fg),
        )
        .child(text("Saeed").font(TextStyle::new(13.5, 400.0)).color(i.fg))
}

pub fn view(s: &State, cx: &mut Cx<State>) -> El {
    let dark = cx.system_dark();
    SAW_DARK.store(dark, Ordering::Relaxed);
    SAW_INSET_X10.store(
        (cx.titlebar_inset() * 10.0).round() as u32,
        Ordering::Relaxed,
    );
    BUILDS.fetch_add(1, Ordering::Relaxed);
    let i = ink(dark);
    let mut k = Kit::neutral(dark);
    // The shell paints `bg_deep` under everything. Clear it so the material shows through.
    k.c.bg_deep = Color {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 0.0,
    };
    k.c.bg_base = i.card;
    k.c.text_base = i.fg;
    k.c.text_muted = i.muted;
    k.c.border_muted = i.hairline;
    let toolbar = Toolbar::new().title("Snippets  ›  Work  ›  Email  ›  Email signature");
    let split = SplitView::new(card(&i))
        .sidebar(&s.sidebar, |s: &mut State| &mut s.sidebar, sidebar(&i))
        .toolbar(toolbar)
        .titlebar_inset(cx.titlebar_inset())
        .frame_inset(8.0)
        .toolbar_in_panel()
        .sidebar_padding(10.0, 0.0)
        .build(&k);
    // The window-wide backdrop: the material (Popover only) with the translucent sidebar tint
    // over it, then the shell. The tint is a child, because a native material replaces its own
    // element's fill.
    let mut backdrop = el().key("backdrop").w_full().h_full();
    if s.material == Material::Popover {
        backdrop = backdrop.system_material(rune::SystemMaterial::Popover);
    }
    backdrop
        .child(
            el().key("tint")
                .w_full()
                .h_full()
                .fill(i.sidebar_fill)
                .absolute(0.0, 0.0),
        )
        .child(split)
}
