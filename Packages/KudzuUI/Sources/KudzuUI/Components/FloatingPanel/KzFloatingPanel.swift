import AppKit
import SwiftUI

/// A floating, non-activating, key-capable panel that hosts SwiftUI on Kudzu glass.
///
/// It's the base for the search palette, the form at the caret and `kzPopover`: it can take
/// keyboard focus without activating Kudzu, so the app the user was typing in stays frontmost.
///
/// ```swift
/// let panel = KzFloatingPanel(sizing: .fixed(CGSize(width: 760, height: 460))) { SearchPaletteView(…) }
/// panel.keyHandler = { event in controller.handle(event) }   // ↑/↓/↩ before the search field
/// panel.onClose = { reason in model.isActive = false }
/// panel.present(.centered(topFraction: 1.0 / 6))
///
/// formPanel.present(.edge(.bottom, anchor: caretRect, gap: 6))  // flips above at the screen's bottom
/// ```
///
/// - Esc closes it (unless an input method is composing), before the focused view sees the key.
/// - With `.closesOnResignKey` it closes when another window takes key, except its own child
///   windows (a menu or popover opened from it).
/// - With `.closesOnOutsideClick` a click on the transparent margin around the glass (the window is
///   larger than the glass to fit its shadow) closes it.
/// - With `.restoresFocus` the window that was key before goes back to being key, with the same
///   first responder.
@MainActor
open class KzFloatingPanel: NSPanel {
    /// What closes the panel and what happens when it closes.
    public struct Behavior: OptionSet, Sendable {
        public let rawValue: Int
        public init(rawValue: Int) { self.rawValue = rawValue }

        /// Esc closes the panel.
        public static let closesOnEscape = Behavior(rawValue: 1 << 0)
        /// Losing key status to a window other than a child closes the panel.
        public static let closesOnResignKey = Behavior(rawValue: 1 << 1)
        /// A click on the margin around the glass closes the panel.
        public static let closesOnOutsideClick = Behavior(rawValue: 1 << 2)
        /// On close, the previously key window (in this app) becomes key again with its first responder.
        public static let restoresFocus = Behavior(rawValue: 1 << 3)

        /// Palettes and transient popovers.
        public static let transient: Behavior = [.closesOnEscape, .closesOnResignKey, .closesOnOutsideClick, .restoresFocus]
        /// Forms that stay up until they're submitted or cancelled.
        public static let modeless: Behavior = [.closesOnEscape, .restoresFocus]
    }

    /// How the panel is drawn.
    public enum Chrome: Sendable {
        /// `glass-strong` behind the content, clipped to rounded corners, with an optional shadow token.
        case glass(cornerRadius: CGFloat = KzRadius.xl, shadow: KzShadow? = .float)
        /// No chrome: the content draws everything (the window is still transparent).
        case none
    }

    /// How the panel gets its size.
    public enum Sizing: Sendable {
        /// A fixed size for the glass.
        case fixed(CGSize)
        /// The content's ideal size, following it as it changes.
        case fitting
    }

    /// Why the panel closed.
    public enum CloseReason: Sendable, Equatable {
        /// Esc.
        case escape
        /// Another window became key.
        case resignedKey
        /// A click outside the glass.
        case outsideClick
        /// `dismiss()` was called.
        case programmatic
    }

    /// Gets every key-down before the first responder (a search field's editor would otherwise eat
    /// ↑/↓/↩). Return true when it handled the event.
    public var keyHandler: ((NSEvent) -> Bool)?
    /// Called once each time the panel closes, after it has ordered out and focus has been restored.
    public var onClose: ((CloseReason) -> Void)?
    /// What closes the panel.
    public var behavior: Behavior
    /// The glass's frame in screen coordinates (the window is larger by the shadow margins).
    public private(set) var chromeFrame: CGRect = .zero
    /// True between `present` and close.
    public private(set) var isPresented = false

    private let chrome: Chrome
    private let sizing: Sizing
    private let margins: NSEdgeInsets
    private let hostingView: NSHostingView<AnyView>
    private var placement: KzFloatingPlacement?
    private var screenOverride: NSScreen?
    private var focusMemory = KzFocusMemory()
    private var fittingSize: CGSize = .zero
    private var isClosing = false

    /// Creates a panel hosting `content`. Nothing shows until `present(_:)`.
    public init<Content: View>(
        sizing: Sizing = .fitting,
        chrome: Chrome = .glass(),
        behavior: Behavior = .transient,
        level: NSWindow.Level = .floating,
        @ViewBuilder content: () -> Content
    ) {
        self.chrome = chrome
        self.sizing = sizing
        self.behavior = behavior
        if case .glass(_, let shadow) = chrome {
            margins = KzFloatingChrome.margins(for: shadow)
        } else {
            margins = NSEdgeInsets()
        }
        hostingView = NSHostingView(rootView: AnyView(EmptyView()))
        super.init(contentRect: .zero, styleMask: [.borderless, .nonactivatingPanel], backing: .buffered, defer: true)
        self.level = level
        isFloatingPanel = true
        hidesOnDeactivate = false
        isReleasedWhenClosed = false
        isMovable = false
        isOpaque = false
        backgroundColor = .clear
        hasShadow = false
        animationBehavior = .none
        collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary, .transient, .ignoresCycle]

        hostingView.sizingOptions = []
        contentView = hostingView
        setContent(content)
    }

    open override var canBecomeKey: Bool { true }
    open override var canBecomeMain: Bool { false }

    /// Replaces the hosted SwiftUI content (popovers call this as their source state changes).
    public func setContent<Content: View>(@ViewBuilder _ content: () -> Content) {
        hostingView.rootView = AnyView(KzFloatingPanelRoot(chrome: chrome, sizing: sizing, margins: margins, content: content()) { [weak self] size in
            // Resize after SwiftUI's layout pass, not during it.
            DispatchQueue.main.async { MainActor.assumeIsolated { self?.contentSizeChanged(size) } }
        })
        if case .fitting = sizing, isPresented {
            contentSizeChanged(measureFittingSize())
        }
    }

    // MARK: Presenting

    /// Shows the panel at `placement`, optionally attached to `parent` (it then moves with it).
    /// `screen` defaults to the screen of the anchor, point or mouse.
    public func present(_ placement: KzFloatingPlacement, on screen: NSScreen? = nil, attachedTo parent: NSWindow? = nil, makeKey: Bool = true) {
        self.placement = placement
        screenOverride = screen
        isClosing = false
        if case .fitting = sizing { fittingSize = measureFittingSize() }
        if !isPresented {
            focusMemory = NSApp.keyWindow === self ? KzFocusMemory() : KzFocusMemory.capture()
        }
        layoutForPlacement()
        if let parent, parent !== self.parent {
            self.parent?.removeChildWindow(self)
            parent.addChildWindow(self, ordered: .above)
        }
        let wasVisible = isVisible
        if makeKey { makeKeyAndOrderFront(nil) } else { orderFront(nil) }
        if !wasVisible { KzFloatingChrome.fadeIn(self) }
        isPresented = true
    }

    /// Moves the panel to a new placement without closing it.
    public func reposition(_ placement: KzFloatingPlacement) {
        self.placement = placement
        layoutForPlacement()
    }

    /// Closes the panel.
    public func dismiss(_ reason: CloseReason = .programmatic) {
        guard isPresented, !isClosing else { return }
        isClosing = true
        isPresented = false
        parent?.removeChildWindow(self)
        KzFloatingChrome.fadeOut(self)
        // When focus already moved to another window, don't pull it back.
        if behavior.contains(.restoresFocus), reason != .resignedKey { restoreFocus() }
        isClosing = false
        onClose?(reason)
    }

    private func restoreFocus() {
        focusMemory.restore()
        focusMemory = KzFocusMemory()
    }

    // MARK: Layout

    private func measureFittingSize() -> CGSize {
        hostingView.layoutSubtreeIfNeeded()
        let size = hostingView.fittingSize
        return CGSize(width: max(0, size.width - margins.left - margins.right),
                      height: max(0, size.height - margins.top - margins.bottom))
    }

    private var glassSize: CGSize {
        switch sizing {
        case .fixed(let size): size
        case .fitting: fittingSize
        }
    }

    private func layoutForPlacement() {
        guard let placement else { return }
        let size = glassSize
        let visible = screenOverride?.visibleFrame ?? KzFloatingChrome.visibleFrame(containing: Self.referencePoint(of: placement))
        chromeFrame = placement.frame(for: size, in: visible)
        setFrame(KzFloatingChrome.windowFrame(for: chromeFrame, margins: margins), display: true)
    }

    private func contentSizeChanged(_ size: CGSize) {
        guard case .fitting = sizing, size.width > 0, size.height > 0 else { return }
        let rounded = CGSize(width: ceil(size.width), height: ceil(size.height))
        guard rounded != fittingSize else { return }
        fittingSize = rounded
        if isPresented { layoutForPlacement() }
    }

    static func referencePoint(of placement: KzFloatingPlacement) -> CGPoint {
        switch placement {
        case .edge(_, let anchor, _, _): CGPoint(x: anchor.midX, y: anchor.midY)
        case .point(let point): point
        case .aligning(_, let point): point
        case .submenu(let parent, _, _): CGPoint(x: parent.midX, y: parent.midY)
        case .centered: NSEvent.mouseLocation
        }
    }

    // MARK: Events

    open override func sendEvent(_ event: NSEvent) {
        switch event.type {
        case .keyDown:
            if let keyHandler, keyHandler(event) { return }
            if event.keyCode == 53, behavior.contains(.closesOnEscape), !isComposingText {
                dismiss(.escape)
                return
            }
        case .leftMouseDown, .rightMouseDown, .otherMouseDown:
            if behavior.contains(.closesOnOutsideClick), !chromeFrame.isEmpty {
                let point = convertPoint(toScreen: event.locationInWindow)
                if !chromeFrame.contains(point) {
                    dismiss(.outsideClick)
                    return
                }
            }
        default:
            break
        }
        super.sendEvent(event)
    }

    /// True while an input method has marked text (Esc and Return belong to it then).
    public var isComposingText: Bool {
        (firstResponder as? NSTextView)?.hasMarkedText() ?? false
    }

    open override func cancelOperation(_ sender: Any?) {
        if behavior.contains(.closesOnEscape) { dismiss(.escape) } else { super.cancelOperation(sender) }
    }

    open override func resignKey() {
        super.resignKey()
        guard behavior.contains(.closesOnResignKey), isPresented else { return }
        // The new key window is only known once this returns.
        DispatchQueue.main.async { [weak self] in
            MainActor.assumeIsolated {
                guard let self, self.isPresented, !self.isKeyWindow else { return }
                if let key = NSApp.keyWindow, self.isAncestor(of: key) { return }
                self.dismiss(.resignedKey)
            }
        }
    }

    private func isAncestor(of window: NSWindow) -> Bool {
        var current = window.parent
        while let candidate = current {
            if candidate === self { return true }
            current = candidate.parent
        }
        return false
    }
}

/// The SwiftUI root of a floating panel: the content on its glass, padded by the shadow margins.
private struct KzFloatingPanelRoot<Content: View>: View {
    let chrome: KzFloatingPanel.Chrome
    let sizing: KzFloatingPanel.Sizing
    let margins: NSEdgeInsets
    let content: Content
    let sizeChanged: (CGSize) -> Void

    var body: some View {
        sized
            .modifier(KzFloatingGlass(chrome: chrome))
            .padding(EdgeInsets(top: margins.top, leading: margins.left, bottom: margins.bottom, trailing: margins.right))
            .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
    }

    @ViewBuilder private var sized: some View {
        switch sizing {
        case .fixed(let size):
            content.frame(width: size.width, height: size.height)
        case .fitting:
            content.fixedSize()
                .onGeometryChange(for: CGSize.self, of: { $0.size }, action: sizeChanged)
        }
    }
}

/// Glass chrome for a view in its own window: the blur samples what's behind the window.
struct KzFloatingGlass: ViewModifier {
    let chrome: KzFloatingPanel.Chrome

    func body(content: Content) -> some View {
        switch chrome {
        case .glass(let radius, let shadow):
            let shape = RoundedRectangle(cornerRadius: radius)
            content
                .clipShape(shape)
                .kzGlass(.strong, in: shape, blending: .behindWindow, shadow: shadow)
        case .none:
            content
        }
    }
}

/// A floating panel's glass as drawn in its window (`glass-strong`, `radius-xl`, `shadow-float`),
/// for static specimens in the gallery and snapshots.
public struct KzFloatingPanelSurface<Content: View>: View {
    private let cornerRadius: CGFloat
    private let shadow: KzShadow?
    private let content: Content

    /// Draws `content` on floating-panel glass.
    public init(cornerRadius: CGFloat = KzRadius.xl, shadow: KzShadow? = .float, @ViewBuilder content: () -> Content) {
        self.cornerRadius = cornerRadius
        self.shadow = shadow
        self.content = content()
    }

    public var body: some View {
        let shape = RoundedRectangle(cornerRadius: cornerRadius)
        content
            .clipShape(shape)
            .kzGlass(.strong, in: shape, blending: .withinWindow, shadow: shadow)
    }
}
