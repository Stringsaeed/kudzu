//! `components/app/trigger-chip.tsx`: a trigger as a mono token, spaces shown as `␣`.

// The kit's whole API is ported now; screens adopt it as they port.
#![allow(dead_code)]

use rune::{row, text, Color, El, Family, TextStyle};

use crate::ui::theme::{weight, Theme};

/// `font-mono text-[11.5px] leading-none tracking-tight`.
pub const FONT: TextStyle = TextStyle::new(11.5, weight::NORMAL).family(Family::Mono).lh(11.5).tracking(-0.2875);

/// `<TriggerChip trigger muted />`: iris-tinted, or muted (`bg-foreground/[0.05] text-muted-foreground`).
pub fn trigger_chip(th: &Theme, trigger: &str, muted: bool) -> El {
    let t = &th.t;
    let (bg, fg): (Color, Color) = if muted { (Theme::tint(t.foreground, 0.05), t.muted_foreground) } else { (t.iris_soft, t.iris_foreground) };
    row()
        .h(20.0)
        .min_w(0.0)
        .shrink()
        .px(6.0)
        .center_y()
        .radius(5.0)
        .fill(bg)
        .child(text(trigger.replace(' ', "␣")).font(FONT).color(fg).truncate().min_w(0.0).shrink())
        .label(trigger.to_string())
}
