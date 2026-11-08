//! `components/ui/scroll-area.tsx` (Radix ScrollArea, `type="hover"`): a viewport whose thin
//! scroll bar (`w-2.5 p-px border-l border-transparent`, a `rounded-full bg-border` thumb) floats
//! over its right edge while the pointer is over the area and the content overflows. Wheel,
//! trackpad and Page Up/Down scroll it; the thumb drags.
//!
//! ```ignore
//! scroll_area::vertical(th, "shortcuts-scroll", content).h(420.0)
//! scroll_area::horizontal(th, "chips-scroll", chips)
//! ```

use rune::{El, ScrollSpec, col, row};
use rune_kit::{ScrollBar, ScrollBarStyle};

use crate::ui::theme::Theme;

/// The thumb Radix draws: 10 pt strip, 1 pt padding and a 1 pt left border leave 7 pt.
fn style(th: &Theme) -> ScrollBarStyle {
    ScrollBarStyle::native(th.t.border, th.t.border).width(7.0).hover_width(7.0).inside()
}

/// `<ScrollArea>` over `content`, scrolling vertically. Size the returned element as the area
/// (`.h(..)`, `.grow(1.0).min_h(0.0)`, …).
pub fn vertical(th: &Theme, key: &str, content: El) -> El {
    col().key(key.to_string()).w_full().min_h(0.0).grow(1.0).scroll(ScrollSpec { stick_bottom: false }).child(content).scroll_bar_styled(style(th))
}

/// `<ScrollArea>` with `<ScrollBar orientation="horizontal" />` (`h-2.5`, `border-t`).
pub fn horizontal(th: &Theme, key: &str, content: El) -> El {
    row().key(key.to_string()).w_full().scroll_x().child(content).scroll_bar_styled(style(th).horizontal())
}
