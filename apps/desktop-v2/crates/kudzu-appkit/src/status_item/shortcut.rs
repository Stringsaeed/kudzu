//! RUNE-GAP(#141): delete when Rune supplies runtime tray key equivalents and hint labels.
use objc2_app_kit::{NSEventModifierFlags, NSMenuItem};
use objc2_foundation::NSString;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Shortcut {
    pub key: String,
    pub command: bool,
    pub shift: bool,
    pub option: bool,
    pub control: bool,
}

impl Shortcut {
    pub fn command(key: &str) -> Self {
        Self {
            key: key.into(),
            command: true,
            ..Self::default()
        }
    }

    pub fn text(&self) -> String {
        format!(
            "{}{}{}{}{}",
            if self.control { "⌃" } else { "" },
            if self.option { "⌥" } else { "" },
            if self.shift { "⇧" } else { "" },
            if self.command { "⌘" } else { "" },
            self.key.to_uppercase()
        )
    }

    /// S40 maps a Rune key to this value in palette, picker and form `on_key`.
    /// A true result routes to Close Windows and consumes the key.
    pub fn closes_windows(&self) -> bool {
        self.command
            && !self.shift
            && !self.option
            && !self.control
            && self.key.eq_ignore_ascii_case("q")
    }

    pub(crate) fn apply(&self, item: &NSMenuItem) {
        let mut flags = NSEventModifierFlags::empty();
        if self.command {
            flags |= NSEventModifierFlags::Command;
        }
        if self.shift {
            flags |= NSEventModifierFlags::Shift;
        }
        if self.option {
            flags |= NSEventModifierFlags::Option;
        }
        if self.control {
            flags |= NSEventModifierFlags::Control;
        }
        item.setKeyEquivalent(&NSString::from_str(&self.key.to_lowercase()));
        item.setKeyEquivalentModifierMask(flags);
    }
}
