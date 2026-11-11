//! RUNE-GAP(#563): replace this setter with Rune's app appearance API when available.
use objc2_app_kit::{NSAppearance, NSAppearanceNameAqua, NSAppearanceNameDarkAqua, NSApplication};
use objc2_foundation::MainThreadMarker;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Preference {
    Light,
    Dark,
    System,
}

impl Preference {
    /// The UI passes `|| cx.system_dark()`: forced preferences never consult it.
    /// Rune's system_dark mirrors a forced app appearance. After apply(System), its cached
    /// value can stay forced for one turn; S40 rebuilds every window on ThemeChanged.
    pub fn resolve_dark(self, system_dark: impl FnOnce() -> bool) -> bool {
        match self {
            Self::Light => false,
            Self::Dark => true,
            Self::System => system_dark(),
        }
    }
}

/// Apply the same preference that resolves the UI theme, before rebuilding every window.
/// System clears the override so native menus, traffic lights and materials follow macOS.
/// The Rune theme becomes current on ThemeChanged, not necessarily in this apply turn.
pub fn apply(mtm: MainThreadMarker, preference: Preference) -> Result<(), &'static str> {
    let appearance = match preference {
        Preference::System => None,
        Preference::Light | Preference::Dark => {
            // SAFETY: these immutable NSString constants are provided by the linked AppKit.
            let name = unsafe {
                if preference == Preference::Dark {
                    NSAppearanceNameDarkAqua
                } else {
                    NSAppearanceNameAqua
                }
            };
            Some(NSAppearance::appearanceNamed(name).ok_or("AppKit appearance unavailable")?)
        }
    };
    NSApplication::sharedApplication(mtm).setAppearance(appearance.as_deref());
    Ok(())
}
