import AppKit
import ApplicationServices
import Carbon


@MainActor
final class Inserter {
    enum Method { case automatic, type, paste }
    enum Failure: Error { case permissionDenied, eventCreation, pasteboardUnavailable, busy }
    /// There's no signal that the target consumed ⌘V; Electron apps can take well over 250 ms.
    var restoreDelay: Duration = .milliseconds(400)
    private(set) var isBusy = false
    private let source = CGEventSource(stateID: .privateState)
    private var pendingRestore: (items: [NSPasteboardItem], changeCount: Int)?

    func deleteBackward(count: Int) throws {
        for _ in 0..<max(0, count) { try press(51) }
    }

    func moveCaretLeft(count: Int) throws {
        for _ in 0..<max(0, count) { try press(123) }
    }

    func moveCaretRight(count: Int) throws {
        for _ in 0..<max(0, count) { try press(124) }
    }

    func copySelection() throws { try press(8, flags: .maskCommand) }

    func type(_ text: String) throws {
        // Chunk by grapheme cluster so emoji ZWJ sequences and combining marks are never split
        // across events. A single cluster longer than the limit is sent on its own.
        var chunk: [UniChar] = []
        for character in text {
            let units = Array(String(character).utf16)
            if !chunk.isEmpty && chunk.count + units.count > 20 {
                try press(0, unicode: chunk)
                chunk.removeAll(keepingCapacity: true)
            }
            chunk += units
        }
        if !chunk.isEmpty { try press(0, unicode: chunk) }
    }

    func paste(_ text: String) async throws {
        let board = NSPasteboard.general
        var saved: [NSPasteboardItem] = []
        for item in board.pasteboardItems ?? [] {
            let copy = NSPasteboardItem()
            for type in item.types {
                guard let data = item.data(forType: type), copy.setData(data, forType: type) else {
                    throw Failure.pasteboardUnavailable
                }
            }
            saved.append(copy)
        }
        let item = NSPasteboardItem()
        guard item.setString(text, forType: .string),
              item.setData(Data(), forType: .init("org.nspasteboard.TransientType")),
              item.setData(Data(), forType: .init("org.nspasteboard.ConcealedType")) else {
            throw Failure.pasteboardUnavailable
        }
        board.clearContents()
        guard board.writeObjects([item]) else {
            board.clearContents()
            if !saved.isEmpty { board.writeObjects(saved) }
            throw Failure.pasteboardUnavailable
        }
        pendingRestore = (saved, board.changeCount)
        do { try press(9, flags: .maskCommand) } catch { restorePasteboard(); throw error }
        // Restore on our own schedule. Cancelling the expansion (app switch, pause) must not
        // pull the old clipboard back before the target app has read the paste. Awaiting an
        // unstructured task's value doesn't propagate the caller's cancellation into it.
        let delay = restoreDelay
        await Task { @MainActor [weak self] in
            try? await Task.sleep(for: delay)
            self?.restorePasteboard()
        }.value
    }

    func insert(_ text: String, method: Method = .automatic) async throws {
        guard !isBusy else { throw Failure.busy }
        guard CGPreflightPostEventAccess(), !IsSecureEventInputEnabled() else { throw Failure.permissionDenied }
        isBusy = true
        defer { isBusy = false }
        switch method {
        case .type: try type(text)
        case .paste: try await paste(text)
        case .automatic:
            if text.count < 100 && text.filter({ $0.isNewline }).count < 2 { try type(text) }
            else { try await paste(text) }
        }
    }

    func restorePasteboard() {
        guard let saved = pendingRestore else { return }
        pendingRestore = nil
        let board = NSPasteboard.general
        // A user copy during the delay takes precedence over the old clipboard.
        guard board.changeCount == saved.changeCount else { return }
        board.clearContents()
        if !saved.items.isEmpty { board.writeObjects(saved.items) }
    }

    private func press(_ key: CGKeyCode, flags: CGEventFlags = [], unicode: [UniChar] = []) throws {
        guard CGPreflightPostEventAccess(), !IsSecureEventInputEnabled() else { throw Failure.permissionDenied }
        guard let down = CGEvent(keyboardEventSource: source, virtualKey: key, keyDown: true),
              let up = CGEvent(keyboardEventSource: source, virtualKey: key, keyDown: false) else {
            throw Failure.eventCreation
        }
        for event in [down, up] {
            event.flags = flags
            event.setIntegerValueField(.eventSourceUserData, value: EventTap.syntheticMarker)
            if !unicode.isEmpty {
                unicode.withUnsafeBufferPointer { buffer in
                    event.keyboardSetUnicodeString(stringLength: buffer.count, unicodeString: buffer.baseAddress!)
                }
            }
            event.post(tap: .cgSessionEventTap)
        }
    }
}
