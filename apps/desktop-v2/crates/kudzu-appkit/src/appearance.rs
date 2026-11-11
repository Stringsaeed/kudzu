//! RUNE-GAP(#563): replace this setter with Rune's app appearance API when available.
use objc2_app_kit::{NSAppearance, NSApplication};
use objc2_foundation::{MainThreadMarker, NSString};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Preference {
    Light,
    Dark,
    System,
}

impl Preference {
    /// The UI passes `|| cx.system_dark()`: forced preferences never consult it.
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
pub fn apply(mtm: MainThreadMarker, preference: Preference) -> Result<(), &'static str> {
    let appearance = match preference {
        Preference::System => None,
        Preference::Light | Preference::Dark => Some(
            NSAppearance::appearanceNamed(&NSString::from_str(if preference == Preference::Dark {
                "NSAppearanceNameDarkAqua"
            } else {
                "NSAppearanceNameAqua"
            }))
            .ok_or("AppKit appearance unavailable")?,
        ),
    };
    NSApplication::sharedApplication(mtm).setAppearance(appearance.as_deref());
    Ok(())
}
