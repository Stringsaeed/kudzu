//! CSS Color 4 binary search with local MINDE, targeting sRGB only.
//! https://www.w3.org/TR/css-color-4/#binsearch
//! Rune's Color::from_oklab clips before we can measure deltaE, so retain the unbounded
//! conversion here. Alpha is preserved, and never participates in the distance metric.
use rune::Color;

use super::palette::Oklch;

fn lab(v: Oklch) -> [f32; 3] {
    let (s, c) = v.h.to_radians().sin_cos();
    [v.l, v.c * c, v.c * s]
}

pub(super) fn linear(v: Oklch) -> [f32; 3] {
    let [ll, a, b] = lab(v);
    let l = (ll + 0.396_337_78 * a + 0.215_803_76 * b).powi(3);
    let m = (ll - 0.105_561_346 * a - 0.063_854_17 * b).powi(3);
    let s = (ll - 0.089_484_18 * a - 1.291_485_5 * b).powi(3);
    [
        4.076_741_7 * l - 3.307_711_6 * m + 0.230_969_94 * s,
        -1.268_438 * l + 2.609_757_4 * m - 0.341_319_38 * s,
        -0.004_196_086_3 * l - 0.703_418_6 * m + 1.707_614_7 * s,
    ]
}

fn in_gamut(rgb: [f32; 3]) -> bool {
    rgb.iter().all(|v| (0.0..=1.0).contains(v))
}

fn clip(rgb: [f32; 3], alpha: f32) -> Color {
    let [r, g, b] = rgb.map(|v| v.clamp(0.0, 1.0));
    Color {
        r,
        g,
        b,
        a: alpha.clamp(0.0, 1.0),
    }
}

fn delta(clipped: Color, current: Oklch) -> f32 {
    let actual = clipped.to_oklab();
    lab(current)
        .iter()
        .zip(actual)
        .map(|(a, b)| (a - b).powi(2))
        .sum::<f32>()
        .sqrt()
}

pub(super) fn map(origin: Oklch) -> Color {
    if origin.l >= 1.0 {
        return Color::WHITE.alpha(origin.a);
    }
    if origin.l <= 0.0 {
        return Color::BLACK.alpha(origin.a);
    }
    let rgb = linear(origin);
    let mut clipped = clip(rgb, origin.a);
    if in_gamut(rgb) || delta(clipped, origin) < 0.02 {
        return clipped;
    }
    let (mut min, mut max, mut min_in_gamut) = (0.0, origin.c, true);
    while max - min > 0.0001 {
        let chroma = (min + max) / 2.0;
        let current = Oklch {
            c: chroma,
            ..origin
        };
        let rgb = linear(current);
        if min_in_gamut && in_gamut(rgb) {
            min = chroma;
            continue;
        }
        clipped = clip(rgb, origin.a);
        let e = delta(clipped, current);
        if e < 0.02 {
            if 0.02 - e < 0.0001 {
                return clipped;
            }
            min_in_gamut = false;
            min = chroma;
        } else {
            max = chroma;
        }
    }
    clipped
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn css_examples_and_lightness_limits() {
        // CSS Color 4 §14.1.3: Display-P3 yellow and its chroma-reduced sRGB boundary.
        // Local MINDE permits a small clip, retaining more chroma than the boundary example.
        for (input, expected) in [
            (Oklch::new(0.96476, 0.24503, 110.23), [254, 255, 0]),
            (Oklch::new(0.96476, 0.21094, 110.23), [253, 254, 0]),
            (Oklch::new(1.1, 0.3, 30.0), [255, 255, 255]),
            (Oklch::new(-0.1, 0.3, 30.0), [0, 0, 0]),
        ] {
            let c = map(input.alpha(0.37));
            for (actual, expected) in [c.r, c.g, c.b]
                .map(rune::color::srgb_encode)
                .into_iter()
                .zip(expected)
            {
                assert!(
                    (actual * 255.0 - expected as f32).abs() <= 1.0,
                    "{input:?}: {c:?}"
                );
            }
            assert_eq!(c.a, 0.37);
        }
    }

    #[test]
    fn only_chip_time_changes_at_eight_bit_precision() {
        let mut changed = Vec::new();
        for (name, tokens) in [
            ("light", super::super::palette::LIGHT),
            ("dark", super::super::palette::DARK),
        ] {
            for (key, v) in tokens.entries() {
                // Ignore floating point noise at neutral white's boundary.
                if linear(v).iter().any(|c| *c < -0.000001 || *c > 1.000001) {
                    let before = rune::oklcha(v.l, v.c, v.h, v.a).hex();
                    let after = v.color().hex();
                    if before != after {
                        changed.push((name, key, before, after));
                    }
                }
            }
        }
        assert_eq!(
            changed,
            vec![(
                "light",
                "chip_time",
                "#006398ff".to_owned(),
                "#006396ff".to_owned()
            )]
        );
    }
}
