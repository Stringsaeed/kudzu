//! A trigger drawn as a mono token (`components/app/trigger-chip.tsx`): `h-5`, 5 point corners,
//! `px-1.5`, 11.5 point mono, `iris-soft` with iris ink, or `foreground/5%` and muted ink when
//! `muted`. Spaces show as `␣`, so a trailing space in a trigger is visible.

use rune::{row, text, El, Family, TextStyle};

use crate::theme::{radius, Theme};

/// What the chip shows for `trigger`.
pub fn shown(trigger: &str) -> String {
    trigger.replace(' ', "␣")
}

pub fn trigger_chip(theme: &Theme, trigger: &str, muted: bool) -> El {
    let c = &theme.c;
    let (fill, ink) = if muted {
        (c.foreground.alpha(0.05), c.muted_foreground)
    } else {
        (c.iris_soft, c.iris_foreground)
    };
    // `font-mono text-[11.5px] leading-none tracking-tight`.
    let style = TextStyle {
        family: Family::Mono,
        tracking: -0.025 * 11.5,
        ..TextStyle::new(11.5, 400.0).lh(11.5)
    };
    row()
        .role("text")
        .label(trigger.to_owned())
        .h(20.0)
        .shrink()
        .px(6.0)
        .center_y()
        .radius(radius::KEYCAP)
        .fill(fill)
        .child(
            text(shown(trigger))
                .font(style)
                .color(ink)
                .truncate()
                .a11y_hidden(),
        )
}

#[cfg(test)]
mod tests {
    use super::*;
    use rune::{Cx, Env, Runtime};

    #[test]
    fn spaces_show_as_open_boxes() {
        assert_eq!(shown(";sig "), ";sig␣");
        assert_eq!(shown("a b"), "a␣b");
    }

    #[test]
    fn the_chip_draws_open_boxes_and_reads_the_trigger() {
        let view = |_: &(), _: &mut Cx<()>| {
            rune::row().child(trigger_chip(Theme::light(), ";sig ", false).key("chip"))
        };
        let mut rt = Runtime::new((), view, (200.0, 60.0), Env::new(true, 1.0));
        rt.frame(0.0);
        let drawn = rt.elements();
        assert!(drawn.iter().any(|e| e.text.as_deref() == Some(";sig␣")));
        let chip = drawn
            .iter()
            .find(|e| e.key.as_deref() == Some("chip"))
            .unwrap();
        assert_eq!(chip.rect[3], 20.0);
        assert!(rt.accessibility_json().to_string().contains("\";sig \""));
    }
}
