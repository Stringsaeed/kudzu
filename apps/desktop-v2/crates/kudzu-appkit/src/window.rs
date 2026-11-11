//! RUNE-GAP(#562, #574): delete the scoped handle bridge with the last native window adapter.
use crate::geometry::{Rect, Screen};
use objc2_app_kit::{NSView, NSWindow};
use objc2_foundation::MainThreadMarker;
use std::{ffi::c_void, ptr::NonNull};

/// Cannot escape the raw-handle callback or move to another thread.
#[derive(Clone, Copy)]
pub struct WindowRef<'a> {
    pub(crate) window: &'a NSWindow,
    pub(crate) view: &'a NSView,
}

impl WindowRef<'_> {
    pub fn frame(self) -> Rect {
        self.window.frame().into()
    }
    pub fn screen(self) -> Option<Screen> {
        self.window.screen().map(|s| Screen::from_native(&s))
    }
    /// The drawn element's rect is top-down in the borrowed NSView's points, even when
    /// that Rune view has been nested beneath a replacement content view.
    pub fn rect_to_screen(self, drawn: Rect) -> Option<Rect> {
        if !drawn.valid() {
            return None;
        }
        let local = drawn.in_content(self.view.bounds().into(), self.view.isFlipped());
        let base = self.view.convertRect_toView(local.native(), None);
        Some(self.window.convertRectToScreen(base).into())
    }
    pub fn set_origin(self, point: crate::geometry::Point) -> Result<(), &'static str> {
        if !point.x.is_finite() || !point.y.is_finite() {
            return Err("invalid window origin");
        }
        self.window
            .setFrameOrigin(objc2_foundation::NSPoint::new(point.x, point.y));
        Ok(())
    }
}

/// Borrow the window of a raw NSView, inside the framework's `with_raw_handle` call.
/// All public adapter methods after this boundary are safe.
///
/// # Safety
/// `view` must be a live NSView on this main thread for the duration of `f`.
pub unsafe fn with_raw_view<R>(
    _: MainThreadMarker,
    view: NonNull<c_void>,
    f: impl for<'a> FnOnce(WindowRef<'a>) -> R,
) -> Option<R> {
    // SAFETY: the caller guarantees the raw view's validity and this marker proves the thread.
    let view = unsafe { view.cast::<NSView>().as_ref() };
    view.window().map(|window| {
        f(WindowRef {
            window: &window,
            view,
        })
    })
}

/// Borrow an NSWindow and its current content view without storing either pointer.
/// Rune callers use `with_raw_view` to preserve its exact drawing view after reparenting.
///
/// # Safety
/// `window` must be a live NSWindow on this main thread for the duration of `f`.
///
/// ```compile_fail
/// use kudzu_appkit::window::{with_raw_window, WindowRef};
/// use objc2_foundation::MainThreadMarker;
/// use std::{ffi::c_void, ptr::NonNull};
/// fn escape(mtm: MainThreadMarker, pointer: NonNull<c_void>) -> Option<WindowRef<'static>> {
///     unsafe { with_raw_window(mtm, pointer, |window| window) }
/// }
/// ```
pub unsafe fn with_raw_window<R>(
    _: MainThreadMarker,
    window: NonNull<c_void>,
    f: impl for<'a> FnOnce(WindowRef<'a>) -> R,
) -> Option<R> {
    // SAFETY: the caller guarantees the lifetime, type and thread of the pointer.
    let window = unsafe { window.cast::<NSWindow>().as_ref() };
    let view = window.contentView()?;
    Some(f(WindowRef {
        window,
        view: &view,
    }))
}
