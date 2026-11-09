import AppKit
import KudzuDesign
import SwiftUI

/// The palette, clipboard picker, form and permission companion windows (`windows.rs`
/// `floating`): borderless, translucent (`.popover` material, always active), rounded, with a
/// shadow computed from the rounded shape; floating above other apps on every Space; hidden,
/// never closed.
///
/// A non-activating panel: it takes keyboard focus without making Kudzu the active app, so the
/// app the user was in stays frontmost underneath (where the palette inserts and the picker
/// pastes).
@MainActor
final class FloatingPanel: NSPanel {
    /// Called when the panel stops being key (palette and picker hide on focus loss).
    var onResignKey: (() -> Void)?
    /// Called for Esc when nothing in the panel handled it.
    var onCancel: (() -> Void)?

    let effect = NSVisualEffectView()
    private(set) var radius: CGFloat

    init(size: NSSize, radius: CGFloat, title: String = "Kudzu", activating: Bool = false) {
        self.radius = radius
        var style: NSWindow.StyleMask = [.borderless, .fullSizeContentView]
        if !activating { style.insert(.nonactivatingPanel) }
        super.init(contentRect: NSRect(origin: .zero, size: size), styleMask: style, backing: .buffered, defer: false)
        self.title = title
        isFloatingPanel = true
        level = .floating
        collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary, .transient, .ignoresCycle]
        hidesOnDeactivate = false
        isReleasedWhenClosed = false
        isOpaque = false
        backgroundColor = .clear
        hasShadow = true
        isMovableByWindowBackground = false
        animationBehavior = .utilityWindow

        effect.material = .popover
        effect.blendingMode = .behindWindow
        effect.state = .active
        effect.maskImage = VisualEffect.mask(radius: radius)
        effect.autoresizingMask = [.width, .height]
        contentView = effect
    }

    override var canBecomeKey: Bool { true }
    override var canBecomeMain: Bool { false }

    /// Puts a SwiftUI root inside the material.
    func host(_ view: some View) {
        let hosting = NSHostingView(rootView: AnyView(view))
        hosting.sizingOptions = []
        hosting.frame = effect.bounds
        hosting.autoresizingMask = [.width, .height]
        effect.subviews.forEach { $0.removeFromSuperview() }
        effect.addSubview(hosting)
    }

    override func resignKey() {
        super.resignKey()
        onResignKey?()
    }

    override func cancelOperation(_ sender: Any?) {
        if let onCancel { onCancel() } else { super.cancelOperation(sender) }
    }

    /// Recomputes the shadow from the rounded shape (it's cached per shape and size).
    func refreshShadow() {
        effect.maskImage = VisualEffect.mask(radius: radius)
        invalidateShadow()
    }

    /// Shows the panel at `origin` and makes it key.
    func present(at origin: NSPoint) {
        setFrameOrigin(origin)
        makeKeyAndOrderFront(nil)
        refreshShadow()
    }
}
