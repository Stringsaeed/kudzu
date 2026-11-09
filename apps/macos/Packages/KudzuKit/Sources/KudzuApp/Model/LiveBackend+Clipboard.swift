import AppKit
import Foundation
import KudzuCore
import KudzuEngine
import KudzuServices

// Clipboard history commands (`clipboard/mod.rs` "Commands").
extension LiveBackend {
    public func clipboardList() async throws -> ClipboardInfo {
        let clipboard = self.clipboard
        return await Task.detached(priority: .userInitiated) { clipboard.list() }.value
    }

    /// The full text of an entry (list payloads carry an excerpt).
    public func clipboardText(id: UUID) async throws -> String? {
        let clipboard = self.clipboard
        return await Task.detached(priority: .userInitiated) { clipboard.text(id) }.value
    }

    /// Puts an entry back on the clipboard.
    public func clipboardCopy(id: UUID, plain: Bool) async throws {
        do {
            let clipboard = self.clipboard
            try await Task.detached(priority: .userInitiated) { try clipboard.copy(id, plain: plain) }.value
        } catch {
            throw backendError(error)
        }
    }

    /// From the picker: puts an entry on the clipboard and pastes it into the app the user came
    /// from. `.copied` when pasting isn't allowed (the picker stays open to say so).
    public func clipboardPaste(id: UUID, plain: Bool) async throws -> PasteOutcome {
        try await clipboardCopy(id: id, plain: plain)
        guard clipboard.canPaste() else { return .copied }
        let target = clipTarget
        shell?.hideClipboard(restore: false)
        let clipboard = self.clipboard
        let failure = await Task.detached(priority: .userInitiated) { () -> EngineClipboard.PasteError? in
            do throws(EngineClipboard.PasteError) {
                try clipboard.paste(into: target)
                return nil
            } catch {
                return error
            }
        }.value
        switch failure {
        case nil: return .pasted
        case .targetGone?: throw BackendError("The app you came from isn't available any more. The item is on your clipboard.")
        case .notAllowed?: return .copied
        }
    }

    public func clipboardDelete(ids: [UUID]) async throws -> [ClipEntry] {
        let clipboard = self.clipboard
        return await Task.detached(priority: .userInitiated) { clipboard.delete(ids) }.value
    }

    /// Undo for delete (while the files are still in the trash).
    public func clipboardRestore(_ entries: [ClipEntry]) async throws {
        let clipboard = self.clipboard
        return await Task.detached(priority: .userInitiated) { clipboard.restore(entries) }.value
    }

    public func clipboardPin(id: UUID, pinned: Bool) async throws -> ClipEntry? {
        let clipboard = self.clipboard
        return await Task.detached(priority: .userInitiated) { clipboard.pin(id, pinned: pinned) }.value
    }

    public func clipboardSetSecret(id: UUID, secret: Bool?) async throws -> ClipEntry? {
        let clipboard = self.clipboard
        return await Task.detached(priority: .userInitiated) { clipboard.setSecret(id, secret: secret) }.value
    }

    /// Deletes the whole history (pinned items too when asked). Not undoable.
    public func clipboardClear(includePinned: Bool) async throws -> Int {
        let clipboard = self.clipboard
        return await Task.detached(priority: .userInitiated) { clipboard.clear(includePinned: includePinned) }.value
    }

    /// Opens a link entry in the browser, or a file entry (by index) in its default app.
    public func clipboardOpen(id: UUID, index: Int?) async throws {
        guard let entry = clipboard.entry(id) else { throw BackendError("Not found") }
        switch entry.kind {
        case .link:
            let text = (entry.text ?? "").trimmingCharacters(in: .whitespacesAndNewlines)
            let string = text.lowercased().hasPrefix("www.") ? "https://\(text)" : text
            guard let url = URL(string: string), NSWorkspace.shared.open(url) else {
                throw BackendError("Couldn't open \(string).")
            }
        case .file:
            let i = index ?? 0
            guard entry.files.indices.contains(i) else { throw BackendError("Not found") }
            guard NSWorkspace.shared.open(URL(fileURLWithPath: entry.files[i])) else {
                throw BackendError("Couldn't open \(entry.files[i]).")
            }
        default:
            throw BackendError("Nothing to open")
        }
    }

    /// Shows a copied file in Finder.
    public func clipboardReveal(id: UUID, index: Int?) async throws {
        guard let entry = clipboard.entry(id) else { throw BackendError("Not found") }
        let i = index ?? 0
        guard entry.files.indices.contains(i) else { throw BackendError("Not found") }
        NSWorkspace.shared.activateFileViewerSelecting([URL(fileURLWithPath: entry.files[i])])
    }

    /// Opens the main window with a new snippet made from an entry's text.
    public func clipboardToSnippet(id: UUID) async throws {
        let text: String
        do {
            let clipboard = self.clipboard
            text = try await Task.detached(priority: .userInitiated) { try clipboard.snippetText(id) }.value
        } catch {
            throw backendError(error)
        }
        shell?.hideClipboard(restore: false)
        shell?.showMain()
        emit(.newSnippet(.init(content: text)))
    }

    public func showClipboard() async throws { shell?.toggleClipboard() }

    public func hideClipboard() async throws { shell?.hideClipboard(restore: true) }

    /// Clipboard media (the `kzclip://` protocol): image bytes, or `nil`.
    public func clipboardMedia(kind: ClipMediaKind, id: UUID, index: Int) async -> Data? {
        let clipboard = self.clipboard
        return await Task.detached(priority: .userInitiated) { clipboard.media(kind: kind, id: id, index: index)?.data }.value
    }
}
