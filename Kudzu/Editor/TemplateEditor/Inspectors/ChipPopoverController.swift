import AppKit
import KudzuCore
import SwiftUI

/// Shows a chip's inspector in a popover anchored to the chip and applies edits live.
@MainActor
final class ChipPopoverController: NSObject, NSPopoverDelegate {
    private unowned let coordinator: TemplateEditorCoordinator
    private var popover: NSPopover?
    private var chipIndex: Int?
    /// Whether this popover session already registered its undo step.
    private var registeredUndo = false
    /// Identifies the open popover, so a late callback from a closed inspector cannot edit another chip.
    private var session = UUID()

    init(coordinator: TemplateEditorCoordinator) {
        self.coordinator = coordinator
    }

    /// Opens the inspector for the chip at `index`, replacing any open one.
    func show(chipAt index: Int) {
        guard let textView = coordinator.textView, let storage = coordinator.storage,
              let chip = ChipFactory.chip(in: storage, at: index) else { return }
        dismiss()
        chipIndex = index
        registeredUndo = false
        let session = UUID()
        self.session = session
        let inspector = ChipInspector(tag: chip.tag, context: coordinator.inspectorContext(forChipAt: index)) { [weak self] tag in
            self?.apply(tag, session: session)
        }
        let host = NSHostingController(rootView: inspector)
        host.sizingOptions = .preferredContentSize
        let popover = NSPopover()
        popover.behavior = .transient
        popover.contentViewController = host
        popover.delegate = self
        self.popover = popover
        popover.show(relativeTo: coordinator.rect(forChipAt: index), of: textView, preferredEdge: .maxY)
    }

    /// Closes the popover the way the user would, returning focus to the chip.
    func close() {
        popover?.close()
    }

    /// Closes the popover without touching focus or selection, for reloads and teardown.
    func dismiss() {
        let open = popover
        popover = nil
        chipIndex = nil
        open?.delegate = nil
        open?.close()
    }

    private func apply(_ tag: KudzuCore.Tag, session: UUID) {
        guard session == self.session, let chipIndex else { return }
        coordinator.updateChip(at: chipIndex, to: tag, coalescing: registeredUndo)
        registeredUndo = true
        // The chip's width can change with its label; keep the arrow on it.
        popover?.positioningRect = coordinator.rect(forChipAt: chipIndex)
    }

    func popoverDidClose(_ notification: Notification) {
        guard let closed = notification.object as? NSPopover, closed === popover else { return }
        let index = chipIndex
        popover = nil
        chipIndex = nil
        coordinator.popoverDidClose(chipAt: index)
    }
}
