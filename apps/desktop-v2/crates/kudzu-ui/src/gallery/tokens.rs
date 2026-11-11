//! The `tokens` scene: every colour swatch, the 12 depth samples and the type ramp, in the theme
//! `--theme` picks. Its goldens (`tests/golden/gallery-tokens.{light,dark}.png`) catch token
//! regressions.

use rune::{col, el, row, text, El};

use crate::theme::{radius, Theme};

/// The window size, in points.
pub const SIZE: (f32, f32) = (1160.0, 900.0);

const GUTTER: f32 = 24.0;
const SWATCH: (f32, f32) = (96.0, 40.0);
const SWATCHES_PER_ROW: usize = 7;
const DEPTH: (f32, f32) = (96.0, 52.0);
const DEPTH_PER_ROW: usize = 6;

pub fn view(theme: &Theme) -> El {
    let c = &theme.c;
    let left = col().gap(GUTTER).child(swatches(theme)).child(depth(theme));
    row()
        .key("tokens")
        .w_full()
        .h_full()
        .padding(GUTTER)
        .gap(GUTTER * 1.5)
        .fill(c.background)
        .child(left)
        .child(type_ramp(theme))
}

fn heading(theme: &Theme, label: &str) -> El {
    text(label)
        .font(theme.ty.label)
        .color(theme.c.muted_foreground)
}

fn swatches(theme: &Theme) -> El {
    let entries = theme.tokens.entries();
    let rows = entries
        .chunks(SWATCHES_PER_ROW)
        .enumerate()
        .map(|(r, chunk)| {
            let cells = chunk.iter().enumerate().map(|(i, (name, value))| {
                swatch(theme, r * SWATCHES_PER_ROW + i, name, value.color())
            });
            row().gap(10.0).children(cells)
        });
    col()
        .key("swatches")
        .gap(10.0)
        .child(heading(theme, "COLOUR"))
        .children(rows)
}

fn swatch(theme: &Theme, index: usize, name: &str, colour: rune::Color) -> El {
    let chip = el()
        .key(format!("swatch-fill-{index}"))
        .size(SWATCH.0, SWATCH.1)
        .radius(radius::MD)
        .fill(colour)
        .border(1.0, theme.c.hairline);
    col()
        .key(format!("swatch-{index}"))
        .label(name.to_owned())
        .gap(3.0)
        .child(chip)
        .child(
            text(name.replace('_', "-"))
                .font(theme.ty.meta_10)
                .color(theme.c.muted_foreground)
                .truncate()
                .w(SWATCH.0),
        )
}

fn depth(theme: &Theme) -> El {
    let entries = theme.depth.entries();
    let rows = entries.chunks(DEPTH_PER_ROW).map(|chunk| {
        let cells = chunk.iter().map(|(name, shadows)| {
            let sample = shadows.iter().fold(
                el().size(DEPTH.0, DEPTH.1)
                    .radius(radius::LG)
                    .fill(theme.c.surface_raised),
                |sample, s| sample.box_shadow(*s),
            );
            col()
                .key(format!("depth-{name}"))
                .gap(8.0)
                .child(sample)
                .child(
                    text(name.replace('_', "-"))
                        .font(theme.ty.meta_10)
                        .color(theme.c.muted_foreground),
                )
        });
        row().gap(22.0).children(cells)
    });
    let card = col()
        .padding(GUTTER)
        .gap(22.0)
        .radius(radius::XL2)
        .fill(theme.c.card)
        .children(rows);
    col()
        .key("depth")
        .gap(10.0)
        .child(heading(theme, "DEPTH"))
        .child(card)
}

fn type_ramp(theme: &Theme) -> El {
    let lines = theme.ty.ramp().into_iter().map(|(name, style)| {
        let sample = text("Kudzu ;sig 0123")
            .font(style)
            .color(theme.c.foreground)
            .single_line();
        let spec = format!(
            "{name}  {}/{}",
            style.size,
            (style.line_px() * 100.0).round() / 100.0
        );
        row()
            .key(format!("type-{name}"))
            .gap(12.0)
            .child(
                text(spec)
                    .font(theme.ty.meta_10)
                    .color(theme.c.faint)
                    .w(112.0),
            )
            .child(sample)
    });
    col()
        .key("type")
        .gap(6.0)
        .child(heading(theme, "TYPE"))
        .children(lines)
}
