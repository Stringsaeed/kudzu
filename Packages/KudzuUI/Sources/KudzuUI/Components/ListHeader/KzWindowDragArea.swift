import AppKit
import SwiftUI

extension View {
    /// Lets the window be dragged from this view's background, like a titlebar.
    ///
    /// With a hidden title and full-size content, pane headers are the visual titlebar but AppKit only
    /// treats the system titlebar strip as draggable. Put this on header backgrounds; controls inside
    /// still receive their own clicks.
    public func kzWindowDragArea() -> some View {
        modifier(KzWindowDragAreaModifier())
    }
}

private struct KzWindowDragAreaModifier: ViewModifier {
    @Environment(\.kzSnapshotRendering) private var snapshotRendering

    func body(content: Content) -> some View {
        // Offscreen renderers draw AppKit-backed views as a placeholder, so skip it there.
        if snapshotRendering { content } else { content.background { KzWindowDragView() } }
    }
}

private struct KzWindowDragView: NSViewRepresentable {
    func makeNSView(context: Context) -> NSView { DragView() }
    func updateNSView(_ view: NSView, context: Context) {}

    final class DragView: NSView {
        override var mouseDownCanMoveWindow: Bool { true }
        override func accessibilityRole() -> NSAccessibility.Role? { nil }
        override func isAccessibilityElement() -> Bool { false }
    }
}
