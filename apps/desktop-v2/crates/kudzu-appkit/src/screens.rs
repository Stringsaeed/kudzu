//! RUNE-GAP(#146): delete this adapter when Rune refreshes its display list at runtime.
pub use crate::geometry::Screen;
use crate::geometry::{Point, Rect};
use objc2_app_kit::{NSEvent, NSScreen};
use objc2_foundation::{MainThreadMarker, NSMouseInRect, NSPoint};

/// A fresh snapshot on every call; no startup cache survives display changes.
pub fn snapshot(mtm: MainThreadMarker) -> Vec<Screen> {
    NSScreen::screens(mtm)
        .iter()
        .map(|s| Screen::from_native(&s))
        .collect()
}

pub fn pointer(_: MainThreadMarker) -> Point {
    let p = NSEvent::mouseLocation();
    Point { x: p.x, y: p.y }
}

pub fn screen_at(screens: &[Screen], point: Point) -> Option<Screen> {
    screens
        .iter()
        .copied()
        .find(|s| NSMouseInRect(NSPoint::new(point.x, point.y), s.frame.native(), false))
        .or_else(|| screens.first().copied())
}

pub fn under_pointer(mtm: MainThreadMarker) -> Option<Screen> {
    screen_at(&snapshot(mtm), pointer(mtm))
}

/// Settings midpoint wins, then the companion's screen, then AppKit's main screen.
pub fn for_settings(
    mtm: MainThreadMarker,
    settings: Rect,
    companion: Option<Screen>,
) -> Option<Screen> {
    settings_screen(
        &snapshot(mtm),
        settings,
        companion.or_else(|| NSScreen::mainScreen(mtm).map(|s| Screen::from_native(&s))),
    )
}

pub fn settings_screen(
    screens: &[Screen],
    settings: Rect,
    fallback: Option<Screen>,
) -> Option<Screen> {
    screens
        .iter()
        .copied()
        .find(|s| s.frame.contains(settings.midpoint()))
        .or(fallback)
}

pub fn union_frames(screens: &[Screen]) -> Option<Rect> {
    screens.iter().map(|s| s.frame).reduce(Rect::union)
}

pub fn canvas(mtm: MainThreadMarker) -> Option<Rect> {
    union_frames(&snapshot(mtm))
}
