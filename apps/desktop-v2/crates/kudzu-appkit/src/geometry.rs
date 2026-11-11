//! RUNE-GAP(#146, #562, #574): delete shared point-space primitives with the last native adapter.
use objc2_app_kit::NSScreen;
use objc2_foundation::{NSPoint, NSRect, NSSize};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Screen {
    pub frame: Rect,
    pub visible: Rect,
    pub scale: f64,
}

impl Screen {
    pub(crate) fn from_native(screen: &NSScreen) -> Self {
        Self {
            frame: screen.frame().into(),
            visible: screen.visibleFrame().into(),
            scale: screen.backingScaleFactor(),
        }
    }
}

/// Points, never pixels. Screen coordinates use AppKit's bottom-left origin.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl Rect {
    pub fn max_x(self) -> f64 {
        self.x + self.width
    }
    pub fn max_y(self) -> f64 {
        self.y + self.height
    }
    pub fn midpoint(self) -> Point {
        Point {
            x: self.x + self.width / 2.,
            y: self.y + self.height / 2.,
        }
    }
    pub fn contains(self, point: Point) -> bool {
        point.x >= self.x && point.x < self.max_x() && point.y >= self.y && point.y < self.max_y()
    }
    pub fn intersects(self, other: Self) -> bool {
        self.x < other.max_x()
            && other.x < self.max_x()
            && self.y < other.max_y()
            && other.y < self.max_y()
    }
    pub fn union(self, other: Self) -> Self {
        let x = self.x.min(other.x);
        let y = self.y.min(other.y);
        Self {
            x,
            y,
            width: self.max_x().max(other.max_x()) - x,
            height: self.max_y().max(other.max_y()) - y,
        }
    }
    pub(crate) fn valid(self) -> bool {
        [self.x, self.y, self.width, self.height]
            .iter()
            .all(|v| v.is_finite())
            && self.width >= 0.
            && self.height >= 0.
    }
    pub(crate) fn native(self) -> NSRect {
        NSRect::new(
            NSPoint::new(self.x, self.y),
            NSSize::new(self.width, self.height),
        )
    }
    pub(crate) fn in_content(self, bounds: Self, flipped: bool) -> Self {
        let x = self.x.clamp(0., bounds.width);
        let y = self.y.clamp(0., bounds.height);
        let width = self.max_x().clamp(0., bounds.width).max(x) - x;
        let height = self.max_y().clamp(0., bounds.height).max(y) - y;
        Self {
            x: bounds.x + x,
            y: bounds.y
                + if flipped {
                    y
                } else {
                    bounds.height - y - height
                },
            width,
            height,
        }
    }
}

impl From<NSRect> for Rect {
    fn from(value: NSRect) -> Self {
        Self {
            x: value.origin.x,
            y: value.origin.y,
            width: value.size.width,
            height: value.size.height,
        }
    }
}
