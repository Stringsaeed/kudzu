//! RUNE-GAP(#562): delete when Rune has a Sidebar material that follows window active state.
//! Feed `sync` the sidebar's DRAWN (animated) top-down rect on every layout. Use it for
//! resize/full-screen/layout transitions and after content replacement. No window is stored.
use crate::{geometry::Rect, window::WindowRef};
use objc2::{define_class, msg_send, rc::Retained, MainThreadOnly};
use objc2_app_kit::{
    NSAutoresizingMaskOptions, NSView, NSVisualEffectBlendingMode, NSVisualEffectMaterial,
    NSVisualEffectState, NSVisualEffectView, NSWindowOrderingMode,
};
use objc2_foundation::{MainThreadMarker, NSPoint};

/// Convert a drawn rect relative to the top-left of content bounds into native view space.
pub fn material_frame(drawn: Rect, bounds: Rect, flipped: bool) -> Rect {
    drawn.in_content(bounds, flipped)
}

define_class!(
    #[unsafe(super = NSVisualEffectView)]
    #[thread_kind = MainThreadOnly]
    #[name = "KudzuSidebarMaterial"]
    struct EffectView;
    impl EffectView {
        #[unsafe(method(hitTest:))]
        fn hit_test(&self, _: NSPoint) -> *mut NSView { std::ptr::null_mut() }
    }
);

pub struct SidebarMaterial {
    view: Retained<EffectView>,
}

impl SidebarMaterial {
    pub fn new(mtm: MainThreadMarker) -> Self {
        // SAFETY: inherited initializer, our main-thread subclass has no additional ivars.
        let view: Retained<EffectView> =
            unsafe { msg_send![EffectView::alloc(mtm), initWithFrame: Rect::default().native()] };
        view.setMaterial(NSVisualEffectMaterial::Sidebar);
        view.setBlendingMode(NSVisualEffectBlendingMode::BehindWindow);
        view.setState(NSVisualEffectState::FollowsWindowActiveState);
        // Height tracks AppKit live resize even before Rune's next drawn-layout callback.
        view.setAutoresizingMask(NSAutoresizingMaskOptions::ViewHeightSizable);
        Self { view }
    }

    /// Idempotently install/reparent one view under the Metal content, then update its frame.
    /// Zero width hides it during collapse; reopening uses the current animated width.
    pub fn sync(&self, window: WindowRef<'_>, drawn: Rect) -> Result<(), &'static str> {
        if !drawn.valid() {
            return Err("invalid sidebar frame");
        }
        let Some(content) = window.0.contentView() else {
            self.remove();
            return Err("window has no content view");
        };
        let frame = material_frame(drawn, content.bounds().into(), content.isFlipped());
        // SAFETY: both hierarchy references are read on the main thread and retained.
        if unsafe { self.view.superview() }.as_deref() != Some(&content) {
            self.view.removeFromSuperview();
            content.addSubview_positioned_relativeTo(&self.view, NSWindowOrderingMode::Below, None);
        }
        self.view.setFrame(frame.native());
        self.view.setHidden(frame.width == 0. || frame.height == 0.);
        Ok(())
    }

    pub fn remove(&self) {
        self.view.removeFromSuperview();
    }
}

impl Drop for SidebarMaterial {
    fn drop(&mut self) {
        self.remove();
    }
}
