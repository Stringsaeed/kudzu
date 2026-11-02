import AppKit
import KudzuCore
import KudzuUI
import SwiftUI

/// Shows a chip's inspector in a Kudzu popover anchored to the chip and applies edits live.
///
/// It uses the same floating panel and chrome as `.kzPopover` (glass-strong, `radius-lg`, `shadow-float`, no arrow, 12pt
/// padding, 6pt from the anchor). `.kzPopover` itself needs a SwiftUI view to anchor to, and the chip is a rectangle
/// inside an `NSTextView`, so the panel is driven directly. Like the modifier, it is transient: it closes on Esc, on a
/// click outside and when another window takes key, and gives focus back to the text view.
@MainActor
final class ChipPopoverController {
    private unowned let coordinator: TemplateEditorCoordinator
    private var panel: KzFloatingPanel?
    private var chipIndex: Int?
    /// Whether this popover session already registered its undo step.
    private var registeredUndo = false
    /// Identifies the open popover, so a late callback from a closed inspector cannot edit another chip.
    private var session = UUID()
    private var observers: [any NSObjectProtocol] = []

    /// Distance from the chip, as for `.kzPopover`.
    private static let gap: CGFloat = 6
    /// Inset of the content from the glass edge (`space-3`), as for `.kzPopover`.
    private static let contentPadding = KzSpace.s3

    init(coordinator: TemplateEditorCoordinator) {
        self.coordinator = coordinator
    }

    var isOpen: Bool { panel != nil }

    /// Opens the inspector for the chip at `index`, replacing any open one.
    func show(chipAt index: Int) {
        guard let textView = coordinator.textView, let window = textView.window, let storage = coordinator.storage,
              let chip = ChipFactory.chip(in: storage, at: index) else { return }
        dismiss()
        chipIndex = index
        registeredUndo = false
        let session = UUID()
        self.session = session
        let inspector = ChipInspector(tag: chip.tag, context: coordinator.inspectorContext(forChipAt: index)) { [weak self] tag in
            self?.apply(tag, session: session)
        }
        let panel = KzFloatingPanel(sizing: .fitting, chrome: .glass(cornerRadius: KzRadius.lg, shadow: .float),
                                    behavior: .transient, level: window.level) {
            inspector.padding(Self.contentPadding)
        }
        panel.appearance = window.effectiveAppearance
        panel.onClose = { [weak self] _ in
            MainActor.assumeIsolated { self?.panelDidClose(panel) }
        }
        self.panel = panel
        panel.present(placement(forChipAt: index), attachedTo: window, makeKey: true)
        if panel.firstResponder === panel, let content = panel.contentView { panel.makeFirstResponder(content) }
        observe(window)
    }

    /// Closes the popover the way the user would, returning focus to the chip.
    func close() {
        panel?.dismiss(.programmatic)
    }

    /// Closes the popover without touching focus or selection, for reloads and teardown.
    func dismiss() {
        guard let open = panel else { return }
        panel = nil
        chipIndex = nil
        stopObserving()
        open.onClose = nil
        open.behavior.remove(.restoresFocus)
        open.dismiss(.programmatic)
    }

    private func placement(forChipAt index: Int) -> KzFloatingPlacement {
        guard let textView = coordinator.textView, let window = textView.window else { return .centered() }
        let rect = window.convertToScreen(textView.convert(coordinator.rect(forChipAt: index), to: nil))
        return .edge(.bottom, anchor: rect, gap: Self.gap, alignment: .center)
    }

    private func apply(_ tag: KudzuCore.Tag, session: UUID) {
        guard session == self.session, let chipIndex else { return }
        coordinator.updateChip(at: chipIndex, to: tag, coalescing: registeredUndo)
        registeredUndo = true
        // The chip's width can change with its label; keep the popover centred on it.
        panel?.reposition(placement(forChipAt: chipIndex))
    }

    private func panelDidClose(_ closed: KzFloatingPanel) {
        guard closed === panel else { return }
        let index = chipIndex
        panel = nil
        chipIndex = nil
        stopObserving()
        coordinator.popoverDidClose(chipAt: index)
    }

    // MARK: Parent window

    private func observe(_ window: NSWindow) {
        let center = NotificationCenter.default
        for name in [NSWindow.willCloseNotification, NSWindow.didMiniaturizeNotification] {
            observers.append(center.addObserver(forName: name, object: window, queue: .main) { [weak self] _ in
                MainActor.assumeIsolated { self?.dismiss() }
            })
        }
        observers.append(center.addObserver(forName: NSWindow.didResizeNotification, object: window, queue: .main) { [weak self] _ in
            MainActor.assumeIsolated {
                guard let self, let index = self.chipIndex else { return }
                self.panel?.reposition(self.placement(forChipAt: index))
            }
        })
    }

    private func stopObserving() {
        observers.forEach(NotificationCenter.default.removeObserver)
        observers = []
    }
}
