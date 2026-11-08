//! Theme preference (`lib/theme.ts`): which appearance a window paints.

use serde::{Deserialize, Serialize};

/// The user's appearance setting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ThemePreference {
    #[default]
    Dark,
    Light,
    /// Follows the OS appearance as it changes.
    System,
}

/// Whether to paint dark, given the OS appearance (`prefers-color-scheme: dark`).
pub fn is_dark(preference: ThemePreference, system_dark: bool) -> bool {
    preference == ThemePreference::Dark || (preference == ThemePreference::System && system_dark)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_appearance() {
        assert!(is_dark(ThemePreference::Dark, false));
        assert!(!is_dark(ThemePreference::Light, true));
        assert!(is_dark(ThemePreference::System, true));
        assert!(!is_dark(ThemePreference::System, false));
        assert_eq!(ThemePreference::default(), ThemePreference::Dark);
        assert_eq!(serde_json::to_string(&ThemePreference::System).unwrap(), "\"system\"");
    }
}
