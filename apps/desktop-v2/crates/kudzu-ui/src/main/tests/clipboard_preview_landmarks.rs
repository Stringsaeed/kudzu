//! The preview pane's landmarks for the kind scenes against the Tauri reference
//! (`reference/metrics.json`), within 2 pt (label-text widths within 4), in both themes. The
//! comparator needs the sidebar too, so `clipboard-code`, `-color`, `-file`, `-image` and
//! `-link` stay in its not-yet-built list until S73. The sidebar's duplicate crumb names are
//! absent here, so the breadcrumb's root crumb is occurrence 1.

use super::clipboard_landmarks::{both, Landmark};
use crate::main::clipboard::scenes;

const FOOTER_COMMON: &[Landmark] = &[
    ("region", "Preview", 1, [578.0, 50.0, 594.0, 702.0]),
    ("button", "Copy", 1, [594.0, 710.5, 95.609, 28.0]),
    ("button", "Delete", 1, [1128.0, 710.5, 28.0, 28.0]),
];

const IMAGE: &[Landmark] = &[
    (
        "navigation",
        "Clipboard item breadcrumb",
        1,
        [602.0, 63.5, 194.781, 20.0],
    ),
    (
        "button",
        "Pin — keep forever",
        1,
        [1096.0, 710.5, 28.0, 28.0],
    ),
];

const LINK: &[Landmark] = &[
    (
        "navigation",
        "Clipboard item breadcrumb",
        1,
        [602.0, 63.5, 131.297, 20.0],
    ),
    (
        "button",
        "Open in browser",
        1,
        [602.0, 280.969, 139.938, 28.0],
    ),
    ("region", "Copy activity", 1, [602.0, 332.969, 546.0, 78.0]),
    (
        "heading",
        "Copied 3 times",
        1,
        [602.0, 332.969, 546.0, 20.0],
    ),
    ("button", "Save as snippet", 1, [1000.0, 710.5, 28.0, 28.0]),
    ("button", "Open link", 1, [1032.0, 710.5, 28.0, 28.0]),
    ("button", "Hide as a secret", 1, [1064.0, 710.5, 28.0, 28.0]),
    (
        "button",
        "Pin — keep forever",
        1,
        [1096.0, 710.5, 28.0, 28.0],
    ),
];

const FILE: &[Landmark] = &[
    (
        "navigation",
        "Clipboard item breadcrumb",
        1,
        [602.0, 63.5, 264.563, 20.0],
    ),
    (
        "button",
        "Open Kudzu — Services Agreement.pdf",
        1,
        [1086.0, 178.656, 24.0, 24.0],
    ),
    (
        "button",
        "Show Kudzu — Services Agreement.pdf in Finder",
        1,
        [1112.0, 178.656, 24.0, 24.0],
    ),
    ("button", "Open", 1, [1032.0, 710.5, 28.0, 28.0]),
    ("button", "Show in Finder", 1, [1064.0, 710.5, 28.0, 28.0]),
    (
        "button",
        "Pin — keep forever",
        1,
        [1096.0, 710.5, 28.0, 28.0],
    ),
];

const CODE: &[Landmark] = &[
    (
        "navigation",
        "Clipboard item breadcrumb",
        1,
        [602.0, 63.5, 546.0, 20.0],
    ),
    ("button", "Save as snippet", 1, [1032.0, 710.5, 28.0, 28.0]),
    ("button", "Hide as a secret", 1, [1064.0, 710.5, 28.0, 28.0]),
    (
        "button",
        "Pin — keep forever",
        1,
        [1096.0, 710.5, 28.0, 28.0],
    ),
];

const COLOR: &[Landmark] = &[
    (
        "navigation",
        "Clipboard item breadcrumb",
        1,
        [602.0, 63.5, 126.875, 20.0],
    ),
    ("button", "Copy HEX value", 1, [1112.0, 342.156, 24.0, 24.0]),
    ("button", "Copy RGB value", 1, [1112.0, 378.156, 24.0, 24.0]),
    ("button", "Copy HSL value", 1, [1112.0, 414.156, 24.0, 24.0]),
    (
        "button",
        "Copy OKLCH value",
        1,
        [1112.0, 450.656, 24.0, 24.0],
    ),
    ("region", "Copy activity", 1, [602.0, 504.656, 546.0, 78.0]),
    (
        "heading",
        "Copied 3 times",
        1,
        [602.0, 504.656, 546.0, 20.0],
    ),
    ("button", "Save as snippet", 1, [1032.0, 710.5, 28.0, 28.0]),
    ("button", "Hide as a secret", 1, [1064.0, 710.5, 28.0, 28.0]),
    (
        "button",
        "Pin — keep forever",
        1,
        [1096.0, 710.5, 28.0, 28.0],
    ),
];

#[test]
fn clipboard_image_preview_matches_the_reference() {
    both("clipboard-image", scenes::image, &[FOOTER_COMMON, IMAGE]);
}

#[test]
fn clipboard_link_preview_matches_the_reference() {
    both("clipboard-link", scenes::link, &[FOOTER_COMMON, LINK]);
}

#[test]
fn clipboard_file_preview_matches_the_reference() {
    both("clipboard-file", scenes::file, &[FOOTER_COMMON, FILE]);
}

#[test]
fn clipboard_code_preview_matches_the_reference() {
    both("clipboard-code", scenes::code, &[FOOTER_COMMON, CODE]);
}

#[test]
fn clipboard_color_preview_matches_the_reference() {
    both("clipboard-color", scenes::color, &[FOOTER_COMMON, COLOR]);
}
