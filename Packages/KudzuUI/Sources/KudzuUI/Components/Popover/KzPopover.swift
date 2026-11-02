import AppKit
import Combine
import SwiftUI

/// When a popover closes on its own.
public enum KzPopoverBehavior: Sendable {
    /// Closes on a click outside it, when another window takes focus, when Kudzu is deactivated, and on Esc.
    case transient
    /// Closes only on Esc or when `isPresented` becomes false.
    case applicationDefined
}

/// The popover's glass surface as drawn in its floating panel, for static specimens.
public struct KzPopoverSurface<Content: View>: View {
    private let content: Content

    /// Draws `content` on popover glass.
    public init(@ViewBuilder content: () -> Content) {
        self.content = content()
    }

    public var body: some View {
        let shape = RoundedRectangle(cornerRadius: KzRadius.lg)
        content
            .padding(KzPopoverController.contentPadding)
            .clipShape(shape)
            .kzGlass(.strong, in: shape, blending: .withinWindow, shadow: .float)
    }
}

/// Owns a popover's anchor and panel.
@MainActor
final class KzPopoverController: NSObject, ObservableObject {
    /// Inset of the content from the glass edge.
    static let contentPadding: CGFloat = KzSpace.s3
    /// Distance from the anchor.
    static let gap: CGFloat = 6

    let anchor = KzAnchorNSView()
    private var panel: KzFloatingPanel?
    private var anchorMonitor: Any?
    private var edge: Edge = .bottom
    private var alignment: KzFloatingPlacement.Alignment = .center
    private var lastAnchorFrame: CGRect?

    override init() {
        super.init()
        anchor.windowDidChange = { [weak self] window in
            MainActor.assumeIsolated { if window == nil { self?.dismiss() } }
        }
    }

    var isPresented: Bool { panel?.isPresented ?? false }

    func present<Content: View>(_ content: Content, edge: Edge, alignment: KzFloatingPlacement.Alignment,
                                behavior: KzPopoverBehavior, onClose: @escaping @MainActor () -> Void) {
        guard let window = anchor.window, let frame = anchor.screenFrame else {
            onClose()
            return
        }
        self.edge = edge
        self.alignment = alignment
        lastAnchorFrame = frame
        let panel = KzFloatingPanel(sizing: .fitting, chrome: .glass(cornerRadius: KzRadius.lg, shadow: .float),
                                    behavior: behavior == .transient ? .transient : .modeless, level: window.level) {
            Self.wrap(content)
        }
        panel.appearance = window.effectiveAppearance
        panel.onClose = { [weak self] _ in
            MainActor.assumeIsolated {
                self?.stopObserving()
                self?.panel = nil
                onClose()
            }
        }
        self.panel = panel
        panel.present(.edge(edge, anchor: frame, gap: Self.gap, alignment: alignment), attachedTo: window, makeKey: true)
        if panel.firstResponder === panel, let content = panel.contentView { panel.makeFirstResponder(content) }
        startObserving(window)
    }

    func update<Content: View>(_ content: Content) {
        guard let panel, panel.isPresented else { return }
        panel.setContent { Self.wrap(content) }
        repositionIfNeeded()
    }

    func dismiss() {
        panel?.dismiss(.programmatic)
    }

    private static func wrap<Content: View>(_ content: Content) -> some View {
        content.padding(contentPadding)
    }

    private func repositionIfNeeded() {
        guard let panel, let frame = anchor.screenFrame, frame != lastAnchorFrame else { return }
        lastAnchorFrame = frame
        panel.reposition(.edge(edge, anchor: frame, gap: Self.gap, alignment: alignment))
    }

    private func startObserving(_ window: NSWindow) {
        // Clicking the view that opened the popover closes it, rather than closing and reopening it.
        anchorMonitor = NSEvent.addLocalMonitorForEvents(matching: [.leftMouseDown]) { [weak self] event in
            guard let self, self.isPresented, self.anchor.contains(event) else { return event }
            self.dismiss()
            return nil
        }
        let center = NotificationCenter.default
        center.addObserver(self, selector: #selector(parentResized), name: NSWindow.didResizeNotification, object: window)
        center.addObserver(self, selector: #selector(parentClosing), name: NSWindow.willCloseNotification, object: window)
        center.addObserver(self, selector: #selector(parentClosing), name: NSWindow.didMiniaturizeNotification, object: window)
    }

    private func stopObserving() {
        if let anchorMonitor { NSEvent.removeMonitor(anchorMonitor) }
        anchorMonitor = nil
        NotificationCenter.default.removeObserver(self)
    }

    @objc private func parentResized() { repositionIfNeeded() }
    @objc private func parentClosing() { dismiss() }
}

private struct KzPopoverModifier<PopoverContent: View>: ViewModifier {
    @Binding var isPresented: Bool
    let edge: Edge
    let alignment: KzFloatingPlacement.Alignment
    let behavior: KzPopoverBehavior
    let content: () -> PopoverContent
    @StateObject private var controller = KzPopoverController()
    @Environment(\.isEnabled) private var isEnabled

    func body(content view: Content) -> some View {
        view
            .background(KzAnchorBackground(view: controller.anchor, isEnabled: isEnabled) {
                // Keep the popover's content in step with the state it's built from.
                if controller.isPresented { controller.update(content()) }
            })
            .onChange(of: isPresented, initial: true) { _, open in
                if open {
                    guard !controller.isPresented else { return }
                    let binding = $isPresented
                    controller.present(content(), edge: edge, alignment: alignment, behavior: behavior) {
                        if binding.wrappedValue { binding.wrappedValue = false }
                    }
                } else {
                    controller.dismiss()
                }
            }
            .onDisappear { controller.dismiss() }
    }
}

extension View {
    /// Shows a Kudzu popover, a glass floating layer with no arrow, beside this view while
    /// `isPresented` is true.
    ///
    /// - `edge`: the side of this view it opens on; it flips to the opposite side when there isn't room.
    /// - The popover takes keyboard focus (without activating Kudzu if it's in the background);
    ///   Tab moves within it, Esc closes it, and focus goes back to the view that had it.
    /// - `.transient` popovers also close on a click outside them or when another window takes focus.
    /// - The content follows its source state while open, and the popover resizes to fit it.
    public func kzPopover<Content: View>(isPresented: Binding<Bool>, edge: Edge = .bottom,
                                         alignment: KzFloatingPlacement.Alignment = .center,
                                         behavior: KzPopoverBehavior = .transient,
                                         @ViewBuilder content: @escaping () -> Content) -> some View {
        modifier(KzPopoverModifier(isPresented: isPresented, edge: edge, alignment: alignment, behavior: behavior, content: content))
    }
}
