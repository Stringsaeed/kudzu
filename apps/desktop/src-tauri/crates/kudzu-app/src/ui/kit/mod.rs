//! Kudzu's component layer: the shadcn primitives in `components/ui/` (radix-nova, restyled
//! with Kudzu's depth tokens) and the shared app components in `components/app/*.tsx`, as Rune
//! builders. Screens use these, never raw styling, so every surface matches.

pub mod alert_dialog;
pub mod app_icon;
pub mod app_picker;
pub mod badge;
pub mod breadcrumb;
pub mod breadcrumbs;
pub mod button;
pub mod checkbox;
pub mod collapsible;
pub mod command;
pub mod context_menu;
pub mod dialog;
pub mod dropdown_menu;
pub mod formatted_text;
pub mod gallery;
pub mod input;
pub mod input_group;
pub mod item;
pub mod kbd;
pub mod label;
pub mod popover;
pub mod radio_group;
pub mod scroll_area;
pub mod segmented;
pub mod select;
pub mod selection_pill;
pub mod separator;
pub mod shortcut;
pub mod slider;
pub mod switch;
pub mod tabs;
pub mod textarea;
pub mod toast;
pub mod toggle;
pub mod toggle_group;
pub mod tooltip;
pub mod trigger_chip;
pub mod wordmark;

use rune::{Cx, El};

// MARK: Overlays
//
// Overlays (tooltip, popover, dialog, alert_dialog, dropdown_menu, context_menu, select, the
// app picker, toasts) follow rune-kit's pattern: a builder takes the screen's state for that
// overlay plus an accessor to it, and `build(cx, &mut layer)` returns the trigger in place while
// pushing the open content into `layer`. The screen collects one `Vec<El>` per view and hands it
// to `root` (as `el().children(layer)` with the toast region), so content escapes every clip and
// paints above the page.
//
// - Open state lives in the screen: `bool` + `on_open_change(|s, open, tx|)` for popovers and
//   dialogs; `MenuState` / `SelectState` / `CommandState` / `AppPickerState` fields (with a
//   `|s| &mut s.x` accessor) for components that also track a highlighted row or a search.
//   Tooltip hover state is the kit's own, so tooltips need none.
// - Anchored content uses `El::anchored(_fit)` against the trigger's key (`{key}-trigger`),
//   flips and stays inside the window; dialogs and the item-aligned select position themselves
//   in window points, which is why `root` makes the overlay layer cover the window.
// - Every open surface is a Rune modal: focus moves in, Tab stays, Escape and presses outside
//   close it through the screen's handler, and focus returns to the trigger.
// - Presence: `popover::pop` (fade, zoom 95 %, 8 pt slide toward the trigger, `ease-pop` in,
//   `ease-out` out) and `popover::zoom` for dialogs, with the scrim fading on its own.

/// A window's root: the page, with overlays (menus, dialogs, toasts) mounted above it so they
/// escape every clip (rune-kit's `root`). The overlay layer covers the window, so overlays that
/// place themselves in window points (dialogs, the item-aligned select, toasts) land there.
pub fn root<S: 'static>(page: El, overlays: El, _cx: &mut Cx<S>) -> El {
    rune_kit::root(page, vec![rune::el().key("kit-overlays").absolute(0.0, 0.0).w_full().h_full().child(overlays)])
}
