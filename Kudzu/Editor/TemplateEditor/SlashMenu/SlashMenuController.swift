import AppKit
import KudzuCore
import SwiftUI

/// Observable state behind the `/` menu.
@MainActor @Observable
final class SlashMenuModel {
    var items: [TemplateInsertItem] = []
    var query = ""
    var selection: TemplateInsertItem?
}

/// A compact insert menu floating at the caret. It never becomes key, so typing keeps going to the editor,
/// which forwards arrow keys, Return and Escape here.
@MainActor
final class SlashMenuController {
    private let model = SlashMenuModel()
    private let onPick: (TemplateInsertItem) -> Void
    private var panel: NSPanel?
    private var hostingView: FirstMouseHostingView<SlashMenuView>?

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
        // Reassigning the root view applies the model change now, so `fittingSize` is current.
        hostingView.rootView = makeRootView()
        let size = hostingView.fittingSize
        let screen = window.screen?.visibleFrame ?? NSScreen.main?.visibleFrame ?? .infinite
        var origin = NSPoint(x: caret.minX - 10, y: caret.minY - size.height - 4)
        if origin.y < screen.minY { origin.y = caret.maxY + 4 }
        origin.x = min(max(origin.x, screen.minX + 4), screen.maxX - size.width - 4)
        panel.setFrame(NSRect(origin: origin, size: size), display: true)
        if panel.parent !== window {
            panel.parent?.removeChildWindow(panel)
            window.addChildWindow(panel, ordered: .above)
        }
        panel.orderFront(nil)
    }

    func hide() {
        model.items = []
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

    private func makeRootView() -> SlashMenuView {
        SlashMenuView(model: model) { [weak self] item in self?.onPick(item) }
    }

    private func makePanel() -> NSPanel {
        let panel = NonKeyPanel(contentRect: .zero, styleMask: [.borderless, .nonactivatingPanel], backing: .buffered, defer: true)
        panel.isFloatingPanel = true
        panel.hasShadow = true
        panel.backgroundColor = .clear
        panel.isOpaque = false
        panel.hidesOnDeactivate = true
        panel.animationBehavior = .utilityWindow

        let effect = NSVisualEffectView()
        effect.material = .menu
        effect.state = .active
        effect.blendingMode = .behindWindow
        effect.wantsLayer = true
        effect.layer?.cornerRadius = 8
        effect.layer?.masksToBounds = true

        let host = FirstMouseHostingView(rootView: makeRootView())
        host.translatesAutoresizingMaskIntoConstraints = false
        effect.addSubview(host)
        NSLayoutConstraint.activate([
            host.leadingAnchor.constraint(equalTo: effect.leadingAnchor),
            host.trailingAnchor.constraint(equalTo: effect.trailingAnchor),
            host.topAnchor.constraint(equalTo: effect.topAnchor),
            host.bottomAnchor.constraint(equalTo: effect.bottomAnchor),
        ])
        panel.contentView = effect
        self.panel = panel
        hostingView = host
        return panel
    }
}

/// A panel that never takes key status away from the editor.
private final class NonKeyPanel: NSPanel {
    override var canBecomeKey: Bool { false }
    override var canBecomeMain: Bool { false }
}

/// Lets a click on a menu row act immediately even though the panel is not key.
final class FirstMouseHostingView<Content: View>: NSHostingView<Content> {
    override func acceptsFirstMouse(for event: NSEvent?) -> Bool { true }
}
