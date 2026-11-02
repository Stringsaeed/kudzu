import AppKit
import KudzuCore
import KudzuUI
import SwiftUI

/// The `/` insert menu, floating under the caret on the Kudzu menu surface.
///
/// It never becomes key, so typing keeps going to the editor, which filters the list and forwards ↑/↓, Return,
/// Tab and Esc here (see `TemplateEditorCoordinator.textView(_:doCommandBy:)`). Hover highlights a row and a click
/// picks it, like a menu.
@MainActor
final class SlashMenuController {
    private let model = SlashMenuModel()
    private let onPick: (TemplateInsertItem) -> Void
    private var panel: SlashMenuPanel?
    private var hostingView: SlashMenuHostingView?
    /// The glass in screen coordinates (the panel is larger by the shadow margins).
    private var glassFrame: CGRect = .zero
    private var pressedRow: Int?

    init(onPick: @escaping (TemplateInsertItem) -> Void) {
        self.onPick = onPick
    }

    var isVisible: Bool { panel?.isVisible == true }
    var selectedItem: TemplateInsertItem? { model.selection }

    /// Shows `items` under `caret` (screen coordinates), keeping the highlighted item when it still matches.
    func show(items: [TemplateInsertItem], query: String, below caret: NSRect, in window: NSWindow) {
        // A new filter result highlights its best match; an unchanged one keeps the user's arrow-key choice.
        if items != model.items || model.selection.map(items.contains) != true { model.selection = items.first }
        model.items = items
        model.query = query
        let panel = panel ?? makePanel()
        guard let hostingView else { return }
        panel.appearance = window.effectiveAppearance
        // Reassigning the root view applies the model change now, so `fittingSize` is current.
        hostingView.rootView = SlashMenuView(model: model)
        let margins = SlashMenuMetrics.margins
        let fitting = hostingView.fittingSize
        let glassSize = CGSize(width: fitting.width - margins.left - margins.right,
                               height: fitting.height - margins.top - margins.bottom)
        let visible = window.screen?.visibleFrame ?? NSScreen.main?.visibleFrame ?? .infinite
        // The row icons sit under the slash: pull the glass left by the menu padding plus the item inset.
        let anchor = caret.offsetBy(dx: -10, dy: 0)
        glassFrame = KzFloatingPlacement.edge(.bottom, anchor: anchor, gap: 4, alignment: .leading).frame(for: glassSize, in: visible)
        panel.setFrame(NSRect(x: glassFrame.minX - margins.left, y: glassFrame.minY - margins.bottom,
                              width: fitting.width, height: fitting.height), display: true)
        if panel.parent !== window {
            panel.parent?.removeChildWindow(panel)
            window.addChildWindow(panel, ordered: .above)
        }
        panel.orderFront(nil)
    }

    func hide() {
        model.items = []
        pressedRow = nil
        guard let panel, panel.isVisible else { return }
        panel.parent?.removeChildWindow(panel)
        panel.orderOut(nil)
    }

    func moveSelection(by delta: Int) {
        guard !model.items.isEmpty else { return }
        let current = model.selection.flatMap(model.items.firstIndex(of:)) ?? 0
        let next = (current + delta + model.items.count) % model.items.count
        model.selection = model.items[next]
    }

    // MARK: Pointer

    private func row(for event: NSEvent) -> Int? {
        guard let window = event.window else { return nil }
        let point = window.convertPoint(toScreen: event.locationInWindow)
        guard glassFrame.contains(point) else { return nil }
        // Screen coordinates grow upwards; the metrics are measured from the glass's top edge.
        return SlashMenuMetrics.row(atY: glassFrame.maxY - point.y, itemCount: model.items.count)
    }

    private func pointerMoved(_ event: NSEvent) {
        if let row = row(for: event) { model.selection = model.items[row] }
    }

    private func pointerDown(_ event: NSEvent) {
        pressedRow = row(for: event)
        pointerMoved(event)
    }

    private func pointerUp(_ event: NSEvent) {
        defer { pressedRow = nil }
        guard let pressed = pressedRow, row(for: event) == pressed, model.items.indices.contains(pressed) else { return }
        onPick(model.items[pressed])
    }

    private func makePanel() -> SlashMenuPanel {
        let panel = SlashMenuPanel(contentRect: .zero, styleMask: [.borderless, .nonactivatingPanel], backing: .buffered, defer: true)
        panel.isFloatingPanel = true
        // The shadow is drawn by the menu surface like the CSS; the window itself has none.
        panel.hasShadow = false
        panel.backgroundColor = .clear
        panel.isOpaque = false
        panel.hidesOnDeactivate = true
        panel.animationBehavior = .none
        panel.acceptsMouseMovedEvents = true

        let host = SlashMenuHostingView(rootView: SlashMenuView(model: model))
        host.onMove = { [weak self] in self?.pointerMoved($0) }
        host.onDown = { [weak self] in self?.pointerDown($0) }
        host.onUp = { [weak self] in self?.pointerUp($0) }
        panel.contentView = host
        self.panel = panel
        hostingView = host
        return panel
    }
}

/// A panel that never takes key status away from the editor.
private final class SlashMenuPanel: NSPanel {
    override var canBecomeKey: Bool { false }
    override var canBecomeMain: Bool { false }
}

/// Hosts the menu and reports the pointer. A click acts at once even though the panel is not key.
final class SlashMenuHostingView: NSHostingView<SlashMenuView> {
    var onMove: ((NSEvent) -> Void)?
    var onDown: ((NSEvent) -> Void)?
    var onUp: ((NSEvent) -> Void)?
    private var trackingArea: NSTrackingArea?

    override func acceptsFirstMouse(for event: NSEvent?) -> Bool { true }

    override func updateTrackingAreas() {
        super.updateTrackingAreas()
        if let trackingArea { removeTrackingArea(trackingArea) }
        let area = NSTrackingArea(rect: .zero, options: [.mouseMoved, .activeAlways, .inVisibleRect], owner: self, userInfo: nil)
        addTrackingArea(area)
        trackingArea = area
    }

    override func mouseMoved(with event: NSEvent) { onMove?(event) }
    override func mouseDown(with event: NSEvent) { onDown?(event) }
    override func mouseDragged(with event: NSEvent) { onMove?(event) }
    override func mouseUp(with event: NSEvent) { onUp?(event) }
}
