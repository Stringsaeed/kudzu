//! RUNE-GAP(#574): delete this default guide panel when Rune provides a proven click-through,
//! non-activating all-Spaces overlay. The borrowed companion window is never stored here.
mod lifecycle;
use crate::geometry::{Point, Rect};
pub use lifecycle::{Generation, Lifecycle, Observation, Step};
use objc2::{define_class, msg_send, rc::Retained, DefinedClass, MainThreadOnly};
use objc2_app_kit::{
    NSBackingStoreType, NSBezierPath, NSColor, NSCompositingOperation, NSImage, NSPanel, NSView,
    NSWindowCollectionBehavior, NSWindowStyleMask, NSWorkspace,
};
use objc2_foundation::{MainThreadMarker, NSPoint, NSRect, NSString};
use std::{cell::RefCell, path::Path};

struct Drawing {
    icon: Retained<NSImage>,
    source: Point,
    destination: Rect,
    phase: f64,
}

define_class!(
    #[unsafe(super = NSView)]
    #[thread_kind = MainThreadOnly]
    #[name = "KudzuPermissionGuideDrawing"]
    #[ivars = RefCell<Drawing>]
    struct DrawingView;
    impl DrawingView {
        #[unsafe(method(hitTest:))]
        fn hit_test(&self, _: NSPoint) -> *mut NSView { std::ptr::null_mut() }

        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _: NSRect) {
            let drawing = self.ivars().borrow();
            let fade = (drawing.phase * 10.).min((1. - drawing.phase) * 10.).clamp(0., 1.);
            NSColor::controlAccentColor().colorWithAlphaComponent(0.65 * fade).setStroke();
            let destination = drawing.destination;
            let outline = NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(Rect {
                x: destination.x - 3., y: destination.y - 3., width: destination.width + 6., height: destination.height + 6.
            }.native(), 14., 14.);
            outline.setLineWidth(3.);
            // SAFETY: the dash slice remains alive for this synchronous call.
            unsafe { outline.setLineDash_count_phase([4., 6.].as_ptr(), 2, 0.); }
            outline.stroke();
            let (end, bend) = curve(drawing.source, destination);
            let path = NSBezierPath::bezierPath();
            path.moveToPoint(NSPoint::new(drawing.source.x, drawing.source.y));
            path.curveToPoint_controlPoint1_controlPoint2(NSPoint::new(end.x, end.y), NSPoint::new(bend.x, bend.y), NSPoint::new(bend.x, bend.y));
            unsafe { path.setLineDash_count_phase([4., 6.].as_ptr(), 2, 0.); }
            path.setLineWidth(1.5);
            path.stroke();
            let point = ghost(drawing.source, destination, drawing.phase);
            drawing.icon.drawInRect_fromRect_operation_fraction(Rect { x: point.x - 26., y: point.y - 26., width: 52., height: 52. }.native(), NSRect::ZERO, NSCompositingOperation::SourceOver, fade * 0.85);
        }
    }
);

fn curve(source: Point, destination: Rect) -> (Point, Point) {
    let midpoint = destination.midpoint();
    let end = Point {
        x: midpoint.x + destination.width * 0.15,
        y: midpoint.y,
    };
    (
        end,
        Point {
            x: (source.x + end.x) / 2.,
            y: source.y.max(end.y) + 65.,
        },
    )
}

fn ghost(source: Point, destination: Rect, phase: f64) -> Point {
    let (end, bend) = curve(source, destination);
    let t = ((phase - 0.12) / 0.65).clamp(0., 1.);
    let t = t * t * (3. - 2. * t);
    let u = 1. - t;
    Point {
        x: u * u * u * source.x + 3. * u * t * bend.x + t * t * t * end.x,
        y: u * u * u * source.y + 3. * u * t * bend.y + t * t * t * end.y,
    }
}

/// Owns only its own native panel and drawing view, never the caller's window.
/// The UI drives Lifecycle, supplying current observations rather than retaining a window.
pub struct GuidePanel {
    panel: Retained<NSPanel>,
    drawing: Retained<DrawingView>,
}

impl GuidePanel {
    pub fn new(mtm: MainThreadMarker, app_bundle: &Path) -> Result<Self, &'static str> {
        if app_bundle.extension().is_none_or(|e| e != "app") {
            return Err("guide icon requires an app bundle");
        }
        let path = app_bundle.to_str().ok_or("bundle path is not UTF-8")?;
        let icon = NSWorkspace::sharedWorkspace().iconForFile(&NSString::from_str(path));
        // SAFETY: our main-thread NSView subclass initializes its ivars before super init.
        let drawing: Retained<DrawingView> = unsafe {
            msg_send![super(DrawingView::alloc(mtm).set_ivars(RefCell::new(Drawing {
                icon, source: Point::default(), destination: Rect::default(), phase: 0.
            }))), initWithFrame: NSRect::ZERO]
        };
        let panel = NSPanel::initWithContentRect_styleMask_backing_defer(
            mtm.alloc(),
            NSRect::ZERO,
            NSWindowStyleMask::Borderless | NSWindowStyleMask::NonactivatingPanel,
            NSBackingStoreType::Buffered,
            false,
        );
        panel.setIgnoresMouseEvents(true);
        panel.setLevel(3);
        panel.setCollectionBehavior(
            NSWindowCollectionBehavior::CanJoinAllSpaces
                | NSWindowCollectionBehavior::FullScreenAuxiliary,
        );
        panel.setOpaque(false);
        panel.setBackgroundColor(Some(&NSColor::clearColor()));
        panel.setHasShadow(false);
        panel.setHidesOnDeactivate(false);
        // SAFETY: Retained owns lifetime; close must not release AppKit's extra reference.
        unsafe {
            panel.setReleasedWhenClosed(false);
        }
        panel.setContentView(Some(&drawing));
        Ok(Self { panel, drawing })
    }

    /// Rects are in global AppKit points. Source MUST come from the real icon element via
    /// `WindowRef::rect_to_screen`. Supply fresh frames for ALL displays: this module computes
    /// their union, independently of whichever display API the host uses.
    pub fn show(
        &self,
        display_frames: &[Rect],
        source: Rect,
        destination: Rect,
        phase: f64,
    ) -> Result<(), &'static str> {
        if !source.valid()
            || !destination.valid()
            || source.width == 0.
            || source.height == 0.
            || !phase.is_finite()
        {
            return Err("invalid guide drawing geometry");
        }
        if display_frames.iter().any(|frame| !frame.valid()) {
            return Err("invalid display frame");
        }
        let canvas = display_frames
            .iter()
            .copied()
            .reduce(Rect::union)
            .ok_or("no displays for guide panel")?;
        self.panel.setFrame_display(canvas.native(), false);
        self.drawing.setFrame(
            Rect {
                width: canvas.width,
                height: canvas.height,
                ..Rect::default()
            }
            .native(),
        );
        let mut drawing = self.drawing.ivars().borrow_mut();
        let source = source.midpoint();
        drawing.source = Point {
            x: source.x - canvas.x,
            y: source.y - canvas.y,
        };
        drawing.destination = Rect {
            x: destination.x - canvas.x,
            y: destination.y - canvas.y,
            ..destination
        };
        drawing.phase = phase.clamp(0., 1.);
        drop(drawing);
        self.drawing.setNeedsDisplay(true);
        self.panel.orderFrontRegardless();
        Ok(())
    }

    pub fn hide(&self) {
        self.panel.orderOut(None);
    }
}

impl Drop for GuidePanel {
    fn drop(&mut self) {
        self.hide();
        self.panel.close();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ghost_uses_the_real_source_and_ends_inside_settings() {
        let source = Point { x: 602., y: 900. };
        let destination = Rect {
            x: 830.,
            y: 162.,
            width: 740.,
            height: 852.,
        };
        assert_eq!(ghost(source, destination, 0.), source);
        assert_eq!(ghost(source, destination, 1.), Point { x: 1311., y: 588. });
    }
}
