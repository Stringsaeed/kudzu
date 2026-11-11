//! shadcn's `SelectTrigger` and `SelectContent` (`components/ui/select.tsx`) as rune-kit's
//! [`rune_kit::Select`] (`SelectAppearance::Field`), decorated.
//!
//! Verdict in `PARITY_MATRIX.md`: **decorated** (D6, after S06). rune-kit's field trigger is 28
//! tall with 6 point corners, 13 point text 8 points in, the base background and the `input` (well)
//! depth, a hover tint; its list surface has 6 point corners. Kudzu's trigger is `h-8` (32),
//! text at `pl-2.5` plus the border (11), the chevron 9 from the end (`pr-2` plus the border),
//! `rounded-lg` (10), 14 point text (`text-sm`, the items too), `bg-surface-raised` with the `raised` depth and the `sheen`, no hover tint,
//! `hover:shadow-raised-hover` and `data-[state=open]:shadow-pressed`; the list is `rounded-lg`.
//! Built with a kit whose base background is `surface-raised` and whose field depth is empty,
//! the depth is drawn by [`super::press`]'s layers (`RUNE-GAP(#360)`) and the list's radius set
//! on what `Select::build` pushed (`RUNE-GAP(#573)`). Keyboard focus draws shadcn's 3 point
//! `ring/50` halo around rune-kit's 1 point ring-coloured border (D21).

use std::sync::LazyLock;

use rune::{Color, El, Shadow};
use rune_kit::{Kit, Select, State};

use super::menu::round;
use super::press::{layer as face_layer, raised_layers, Faces, Look, Press};
use crate::theme::{palette::fixed, radius, Theme};

/// `h-8`.
pub const HEIGHT: f32 = 32.0;
/// rune-kit's value row starts 8 in; `pl-2.5` plus the 1 point border is 11.
const LEAD: f32 = 3.0;
/// rune-kit's chevron sits 2 inside a 20 point box after `pr(6)`; `pr-2` plus the border is 9.
const TRAIL: f32 = 7.0;

fn kit(theme: &Theme) -> &'static Kit {
    static LIGHT: LazyLock<Kit> = LazyLock::new(|| field_kit(Theme::light()));
    static DARK: LazyLock<Kit> = LazyLock::new(|| field_kit(Theme::dark()));
    if theme.dark {
        &DARK
    } else {
        &LIGHT
    }
}

fn field_kit(theme: &Theme) -> Kit {
    let mut k = theme.kit().clone();
    k.c.bg_base = theme.c.surface_raised;
    // `text-sm` on the trigger and the items; rune-kit draws both in `ui_compact` (13).
    k.ty.ui_compact = rune::TextStyle {
        line_height: 16.0,
        ..theme.ty.input
    };
    // `Elevation::input` falls back to the neutral button's layers when it has none.
    k.elevation = k.elevation.with_input(Vec::new());
    k.elevation.button_neutral.clear();
    k
}

/// `select.build(kit, layer)` with Kudzu's trigger and list. `open` and `state` are the ones
/// given to the `Select` (rune-kit does not report them back).
pub fn build(select: Select, theme: &Theme, layer: &mut Vec<El>, open: bool, state: State) -> El {
    let from = layer.len();
    let trigger = select.build(kit(theme), layer);
    round(layer, from, radius::LG);
    let [(_, top), (_, bottom)] = fixed::SHEEN;
    let look = match state {
        _ if open => Look::Pressed,
        State::Hover => Look::Hover,
        State::Pressed => Look::Pressed,
        State::Disabled => Look::Rest,
        _ => Look::Live,
    };
    let d = &theme.depth;
    let trigger = trigger
        .h(HEIGHT)
        .pl(LEAD)
        .pr(TRAIL)
        .radius(radius::LG)
        .fill(theme.c.surface_raised)
        .hover(Color::TRANSPARENT);
    // `hover:shadow-raised-hover` only: the face keeps its fill under the pointer.
    let faces = Faces {
        rest: theme.c.surface_raised,
        hover: theme.c.surface_raised,
        sheen: Some((top.color(), bottom.color())),
    };
    let press = Press::new("select");
    let c = &theme.c;
    // `focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/50` (D21): the
    // halo as the focus ring; rune-kit's 1 point inside ring is the border.
    let trigger = raised_layers(
        trigger.focus_ring(3.0, 0.0, c.ring.alpha(0.5)),
        radius::LG,
        faces,
        &d.raised,
        &d.raised_hover,
        &d.pressed,
        look,
        &press,
    );
    if state != State::Focus || open {
        return trigger;
    }
    // A forced focus (galleries): the border over the face, the halo outside.
    let border = face_layer("focus-border", radius::LG, Color::TRANSPARENT, &[])
        .paint_order(0)
        .box_shadow(Shadow {
            spread: 1.0,
            color: c.ring,
            inset: true,
            ..Shadow::default()
        });
    trigger.child(border).outline(3.0, 0.0, c.ring.alpha(0.5))
}
