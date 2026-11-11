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

/// Convert a drawn rect relative to the top-left of Rune's view bounds into its native space.
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
        // SAFETY: superview is a non-owning back-pointer that could become stale during
        // teardown. The scoped Rune view/window borrow and main-thread call prevent that
        // teardown from racing this read; the getter retains the parent before mutation.
        let Some(parent) = (unsafe { window.view.superview() }) else {
            self.remove();
            return Err("Rune view has no parent for sidebar material");
        };
        let local = material_frame(drawn, window.view.bounds().into(), window.view.isFlipped());
        let frame = window
            .view
            .convertRect_toView(local.native(), Some(&parent));
        // Repositioning an existing subview is idempotent and also repairs content replacement.
        parent.addSubview_positioned_relativeTo(
            &self.view,
            NSWindowOrderingMode::Below,
            Some(window.view),
        );
        self.view.setFrame(frame);
        self.view.setHidden(local.width == 0. || local.height == 0.);
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
