//! Corner radii. The scale is `--radius` (0.625rem, 10 pt) times the multipliers of
//! `globals.css:74`; components also use a few radii of their own.

/// `--radius`.
pub const BASE: f32 = 10.0;

/// `--radius-sm` … `--radius-4xl`.
pub const SM: f32 = 6.0;
pub const MD: f32 = 8.0;
pub const LG: f32 = 10.0;
pub const XL: f32 = 14.0;
pub const XL2: f32 = 18.0;
pub const XL3: f32 = 22.0;
pub const XL4: f32 = 26.0;

/// Radii components set directly rather than from the scale: text marks (2), the clipboard
/// source icon and the sidebar's Dev ribbon (3), the checkbox (4), keycaps (5), the `xs` and
/// `sm` buttons' `min(md, 10px)` / `min(md, 12px)` caps (both resolve to `MD` today), and the
/// permission companion's app icon (20).
pub const MARK: f32 = 2.0;
pub const TAG: f32 = 3.0;
pub const CHECKBOX: f32 = 4.0;
pub const KEYCAP: f32 = 5.0;
pub const BUTTON_XS: f32 = if MD < 10.0 { MD } else { 10.0 };
pub const BUTTON_SM: f32 = if MD < 12.0 { MD } else { 12.0 };
pub const APP_ICON: f32 = 20.0;

/// Window clip radii, set on the native window rather than drawn (§1.3).
pub const PANEL_WINDOW: f32 = 18.0;
pub const FORM_WINDOW: f32 = 16.0;

/// The named scale, smallest first, by its CSS suffix (for the gallery and the table test).
pub const SCALE: [(&str, f32); 7] = [
    ("sm", SM),
    ("md", MD),
    ("lg", LG),
    ("xl", XL),
    ("2xl", XL2),
    ("3xl", XL3),
    ("4xl", XL4),
];

#[cfg(test)]
mod tests {
    use super::super::css;
    use super::*;

    #[test]
    fn the_scale_is_the_css_calc() {
        let sheet = css::Sheet::parse(css::GLOBALS);
        assert_eq!(sheet.length("radius", false), BASE);
        for (suffix, ours) in SCALE {
            let factor = sheet.radius_factor(&format!("radius-{suffix}"));
            assert!(
                (BASE * factor - ours).abs() < 1e-4,
                "--radius-{suffix}: {} vs {ours}",
                BASE * factor
            );
        }
    }
}
