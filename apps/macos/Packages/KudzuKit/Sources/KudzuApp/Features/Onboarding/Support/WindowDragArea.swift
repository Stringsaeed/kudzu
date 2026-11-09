import AppKit
import SwiftUI

/// `data-tauri-drag-region`: empty space that moves the window. In a titled window's title row
/// AppKit drags it (`mouseDownCanMoveWindow`); in a borderless panel the view starts the drag itself.
/// Use it as a background or a strip behind content, so buttons in front keep their clicks.
struct WindowDragArea: NSViewRepresentable {
    func makeNSView(context: Context) -> DragView { DragView() }
    func updateNSView(_ view: DragView, context: Context) {}

    final class DragView: NSView {
        override var mouseDownCanMoveWindow: Bool { true }

        override func mouseDown(with event: NSEvent) {
            guard let window, !window.styleMask.contains(.titled) else { return super.mouseDown(with: event) }
            window.performDrag(with: event)
        }
    }
}
