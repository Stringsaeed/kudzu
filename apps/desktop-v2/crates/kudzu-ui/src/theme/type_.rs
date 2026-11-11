//! Text styles, read from the Tauri components' classes (FEATURES §3).
//!
//! CSS facts these follow:
//! - `body` is 13 px on a 20 px line (`text-[13px] leading-5`), and a `text-[Npx]` class sets no
//!   line height, so such text keeps the inherited 20 px line unless it says otherwise.
//! - Tailwind's `text-sm` is 14/20, `text-xs` 12/16, `leading-tight` 1.25, `leading-relaxed`
//!   1.625, `leading-none` 1; `font-medium` 500, `font-semibold` 600.
//! - `tracking-[-0.01em]` is −0.01 × size points.

use rune::{Family, TextStyle};

/// The inherited line (`leading-5`).
const LINE: f32 = 20.0;
const RELAXED: f32 = 1.625;
const TABULAR: &str = "'tnum' 1";

const fn sans(size: f32, weight: f32) -> TextStyle {
    TextStyle::new(size, weight).lh(LINE)
}

const fn em(size: f32, em: f32) -> f32 {
    size * em
}

/// Every Kudzu text style. Sans text uses the sans slot (SF Pro, then Figtree), mono the mono
/// slot (Paper Mono).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Type {
    /// 13/20 regular: the body face everywhere.
    pub body: TextStyle,
    pub body_medium: TextStyle,
    pub body_strong: TextStyle,
    /// Metadata: 10, 10.5, 11 and 11.5 on the inherited line.
    pub meta_10: TextStyle,
    pub meta_10_5: TextStyle,
    pub meta_11: TextStyle,
    pub meta_11_5: TextStyle,
    /// 11 px medium, wide tracking: section labels (`tracking-wide`, 0.025em).
    pub label: TextStyle,
    /// 12.5/relaxed: settings descriptions.
    pub description: TextStyle,
    /// Buttons: default and `lg` 14/20 medium, `sm` 0.8rem (12.8) medium, `xs` 12/16 medium.
    pub control: TextStyle,
    pub control_sm: TextStyle,
    pub control_xs: TextStyle,
    /// Inputs: 14/20 regular (`md:text-sm`).
    pub input: TextStyle,
    /// Keycaps: 11 medium, line 1, tabular (`Kbd`).
    pub keycap: TextStyle,
    /// Clipboard preview body: 13.5/relaxed.
    pub preview: TextStyle,
    /// Dialog titles and the command menu field: 15.
    pub title: TextStyle,
    pub command: TextStyle,
    /// Palette and picker search field: 17, −0.01em.
    pub search: TextStyle,
    /// Snippet name in the editor: 22, tight, semibold, −0.015em.
    pub name: TextStyle,
    /// Permission companion headline: 23 semibold, −0.035em.
    pub headline: TextStyle,
    /// Onboarding headline: 28 semibold, −0.025em.
    pub headline_large: TextStyle,
    /// Insight tiles: 26, line 1, semibold, −0.02em, tabular figures.
    pub stat: TextStyle,
    /// Template editor: Paper Mono 13 on a 1.7 line.
    pub mono: TextStyle,
    /// Template chips: 11.5 medium, line 1.
    pub chip: TextStyle,
}

/// The one set of styles; type does not change with the theme.
pub const TYPE: Type = Type {
    body: sans(13.0, 400.0),
    body_medium: sans(13.0, 500.0),
    body_strong: sans(13.0, 600.0),
    meta_10: sans(10.0, 400.0),
    meta_10_5: sans(10.5, 400.0),
    meta_11: sans(11.0, 400.0),
    meta_11_5: sans(11.5, 400.0),
    label: sans(11.0, 500.0).tracking(em(11.0, 0.025)),
    description: TextStyle::new(12.5, 400.0).lh(12.5 * RELAXED),
    control: sans(14.0, 500.0),
    control_sm: sans(12.8, 500.0),
    control_xs: TextStyle::new(12.0, 500.0).lh(16.0),
    input: sans(14.0, 400.0),
    keycap: TextStyle::new(11.0, 500.0).lh(11.0).features(TABULAR),
    preview: TextStyle::new(13.5, 400.0).lh(13.5 * RELAXED),
    title: sans(15.0, 600.0),
    command: sans(15.0, 400.0),
    search: sans(17.0, 400.0).tracking(em(17.0, -0.01)),
    name: TextStyle::new(22.0, 600.0)
        .lh(22.0 * 1.25)
        .tracking(em(22.0, -0.015)),
    headline: sans(23.0, 600.0).tracking(em(23.0, -0.035)),
    headline_large: sans(28.0, 600.0).tracking(em(28.0, -0.025)),
    stat: TextStyle::new(26.0, 600.0)
        .lh(26.0)
        .tracking(em(26.0, -0.02))
        .features(TABULAR),
    mono: TextStyle::new(13.0, 400.0)
        .family(Family::Mono)
        .lh(13.0 * 1.7),
    chip: TextStyle::new(11.5, 500.0).lh(11.5),
};

impl Type {
    /// Every style by name, largest first (the gallery's type ramp).
    pub fn ramp(&self) -> [(&'static str, TextStyle); 24] {
        [
            ("headline_large", self.headline_large),
            ("stat", self.stat),
            ("headline", self.headline),
            ("name", self.name),
            ("search", self.search),
            ("title", self.title),
            ("command", self.command),
            ("control", self.control),
            ("input", self.input),
            ("preview", self.preview),
            ("body_strong", self.body_strong),
            ("body_medium", self.body_medium),
            ("body", self.body),
            ("mono", self.mono),
            ("control_sm", self.control_sm),
            ("description", self.description),
            ("control_xs", self.control_xs),
            ("chip", self.chip),
            ("meta_11_5", self.meta_11_5),
            ("label", self.label),
            ("keycap", self.keycap),
            ("meta_11", self.meta_11),
            ("meta_10_5", self.meta_10_5),
            ("meta_10", self.meta_10),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn body_is_13_on_20() {
        assert_eq!(
            (TYPE.body.size, TYPE.body.line_px(), TYPE.body.weight),
            (13.0, 20.0, 400.0)
        );
    }

    #[test]
    fn mono_is_paper_mono_13_on_1_7() {
        assert_eq!(TYPE.mono.family, Family::Mono);
        assert!((TYPE.mono.line_px() - 22.1).abs() < 1e-4);
    }

    #[test]
    fn stats_use_tabular_figures() {
        assert_eq!(TYPE.stat.features, "'tnum' 1");
        assert_eq!(TYPE.keycap.features, "'tnum' 1");
    }

    #[test]
    fn tracking_is_em_times_size() {
        assert!((TYPE.search.tracking + 0.17).abs() < 1e-5);
        assert!((TYPE.headline.tracking + 0.805).abs() < 1e-5);
    }

    #[test]
    fn ramp_runs_from_largest_to_smallest() {
        let sizes: Vec<f32> = TYPE.ramp().iter().map(|(_, s)| s.size).collect();
        assert!(sizes.windows(2).all(|w| w[0] >= w[1]), "{sizes:?}");
    }
}
