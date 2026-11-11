//! The eleven date and time formats the chip form offers, in menu order.
//!
//! Labels are the chip text (`Date · ISO date`). A pattern that is not in this list is shown
//! as written.

/// One preset: the ICU pattern stored on the tag, and the short label shown on the chip.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DatePreset {
    pub format: &'static str,
    pub label: &'static str,
}

/// Menu order from the Tauri editor (`DATE_FORMAT_PRESETS`).
pub const DATE_PRESETS: &[DatePreset] = &[
    DatePreset {
        format: "yyyy-MM-dd",
        label: "ISO date",
    },
    DatePreset {
        format: "MMMM d, yyyy",
        label: "Long date",
    },
    DatePreset {
        format: "MMM d, yyyy",
        label: "Medium date",
    },
    DatePreset {
        format: "M/d/yy",
        label: "Short date",
    },
    DatePreset {
        format: "d MMMM yyyy",
        label: "Day month year",
    },
    DatePreset {
        format: "EEEE",
        label: "Weekday",
    },
    DatePreset {
        format: "EEEE, MMMM d",
        label: "Weekday + date",
    },
    DatePreset {
        format: "HH:mm",
        label: "24-hour time",
    },
    DatePreset {
        format: "h:mm a",
        label: "12-hour time",
    },
    DatePreset {
        format: "HH:mm:ss",
        label: "Time with seconds",
    },
    DatePreset {
        format: "yyyy-MM-dd'T'HH:mm:ssXXX",
        label: "ISO timestamp",
    },
];

/// The preset list. Eleven entries.
pub fn date_presets() -> &'static [DatePreset] {
    DATE_PRESETS
}

/// The label for a known pattern, or `None` when the chip should show the pattern itself.
pub fn preset_label(format: &str) -> Option<&'static str> {
    DATE_PRESETS
        .iter()
        .find(|p| p.format == format)
        .map(|p| p.label)
}
