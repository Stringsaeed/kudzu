//! The Appearance setting, followed by every window. Each window's poll calls `sync`; the
//! main window also sets `NSApp.appearance` (RUNE-GAP(#563), in `kudzu_appkit::appearance`).
use crate::theme::{Appearance, Theme};
use kudzu_app::settings::{Settings, Theme as Preference};
use kudzu_app::watch::{Revision, Watch};

/// The Appearance a window last drew with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ThemeSync {
    seen: Option<Revision>,
    appearance: Appearance,
}

impl Default for ThemeSync {
    fn default() -> Self {
        Self {
            seen: None,
            appearance: Appearance::Dark,
        }
    }
}

impl ThemeSync {
    /// Pull the settings when they changed. True on the first pull, whatever the stored
    /// theme, and afterwards when the Appearance changed.
    pub fn sync(&mut self, settings: &Watch<Settings>) -> bool {
        let revision = settings.revision();
        if self.seen == Some(revision) {
            return false;
        }
        let first = self.seen.is_none();
        self.seen = Some(revision);
        let next = appearance(settings.get().theme);
        let changed = first || next != self.appearance;
        self.appearance = next;
        changed
    }

    pub fn appearance(&self) -> Appearance {
        self.appearance
    }

    /// The Appearance a view draws with. Headless (`--snap`, `--tree`, `--scene`), Rune's
    /// `--theme light|dark` sets the system appearance, so windows draw System and follow it;
    /// a running app follows Settings. (Before this, `main` drew the stored Dark headless.)
    pub fn drawn(&self, headless: bool) -> Appearance {
        if headless {
            Appearance::System
        } else {
            self.appearance
        }
    }

    /// The theme a view draws with. `system_dark` is `Cx::system_dark()`.
    pub fn theme(&self, system_dark: bool) -> &'static Theme {
        Theme::resolve(self.drawn(rune::is_headless()), system_dark)
    }

    /// Used by `--scene` setup so `--theme light|dark` drives the scene's theme.
    pub(crate) fn set_for_scene(&mut self, appearance: Appearance) {
        self.appearance = appearance;
    }
}

pub fn appearance(preference: Preference) -> Appearance {
    match preference {
        Preference::Dark => Appearance::Dark,
        Preference::Light => Appearance::Light,
        Preference::System => Appearance::System,
    }
}

/// The native preference for `NSApp.appearance`.
pub fn native(appearance: Appearance) -> kudzu_appkit::appearance::Preference {
    use kudzu_appkit::appearance::Preference as Native;
    match appearance {
        Appearance::Dark => Native::Dark,
        Appearance::Light => Native::Light,
        Appearance::System => Native::System,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kudzu_app::watch::Publisher;
    use std::sync::Arc;

    #[test]
    fn a_window_reports_a_change_once_and_ignores_unrelated_settings() {
        let settings = Publisher::new(Settings::default());
        let mut sync = ThemeSync::default();
        assert!(
            sync.sync(&settings.watch()),
            "the first pull applies the stored theme"
        );
        assert_eq!(sync.appearance(), Appearance::Dark);
        let mut light = Settings {
            theme: Preference::Light,
            ..Settings::default()
        };
        settings
            .replace(Arc::new(light.clone()))
            .notification
            .notify();
        assert!(sync.sync(&settings.watch()));
        assert!(!sync.sync(&settings.watch()));
        light.paused = true;
        settings.replace(Arc::new(light)).notification.notify();
        assert!(!sync.sync(&settings.watch()));
        assert_eq!(sync.appearance(), Appearance::Light);
        assert!(!sync.theme(true).dark);
    }

    /// O2 (S41 review): headless, `--theme light|dark` is the system appearance, so a window
    /// draws System whatever Settings stores (`main` drew the stored Dark). A running app
    /// follows Settings.
    #[test]
    fn a_headless_run_draws_the_system_appearance() {
        let sync = ThemeSync::default();
        assert_eq!(sync.appearance(), Appearance::Dark);
        assert_eq!(sync.drawn(true), Appearance::System);
        assert_eq!(sync.drawn(false), Appearance::Dark);
    }
}
