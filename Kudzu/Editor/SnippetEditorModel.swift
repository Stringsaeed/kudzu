import AppKit
import KudzuCore
import Observation
import SwiftUI

/// A retained draft: Save or leaving the editor commits it; Discard loads the latest stored version.
@MainActor @Observable
final class SnippetEditorModel {
    private(set) var draft: SnippetRecord
    private(set) var saveError: String?
    private var persisted: SnippetRecord
    @ObservationIgnored private unowned let library: LibraryModel

    init(snippet: SnippetRecord, library: LibraryModel) {
        self.draft = snippet
        self.persisted = snippet
        self.library = library
        library.retainEditor(self)
    }

    /// Whether there are edits since the last successful save or adoption of a stored version.
    var isDirty: Bool { draft != persisted }

    /// Changes the draft without writing, so Discard remains available until Save or navigation.
    func edit(_ body: (inout SnippetRecord) -> Void) {
        body(&draft)
    }

    /// A binding to one field of the draft.
    func binding<Value>(_ keyPath: WritableKeyPath<SnippetRecord, Value>) -> Binding<Value> {
        Binding(
            get: { self.draft[keyPath: keyPath] },
            set: { newValue in self.edit { $0[keyPath: keyPath] = newValue } }
        )
    }

    /// Adopts unrelated external edits while retaining local edits and refusing overlapping changes.
    func adoptExternal(_ record: SnippetRecord) {
        guard record != persisted else { return }
        do {
            let merged = try SnippetDraft.merge(draft, baseline: persisted, latest: record)
            persisted = record
            draft = merged
            if !isDirty { saveError = nil }
        } catch {
            saveError = error.localizedDescription
        }
    }

    /// Saves against the actual store, never a potentially stale library snapshot.
    @discardableResult
    func flush() -> Bool {
        guard isDirty else { return true }
        do {
            let saved = try library.saveEdited(draft, baseline: persisted)
            persisted = saved
            draft = saved
            saveError = nil
            return true
        } catch {
            saveError = error.localizedDescription
            library.errorMessage = "Couldn't save “\(draft.name)”: \(error.localizedDescription)"
            return false
        }
    }

    /// Drops only this local draft, reading the latest baseline without writing any stored fields.
    @discardableResult
    func discardChanges() -> Bool {
        do {
            if let latest = try library.storedSnippet(draft.id) {
                persisted = latest
                draft = latest
            } else {
                draft = persisted
            }
            saveError = nil
            library.reload()
            return true
        } catch {
            saveError = error.localizedDescription
            return false
        }
    }

    /// A confirmed, successful local deletion also discards the deleted snippet's draft.
    func didDelete() {
        draft = persisted
        saveError = nil
    }

    // MARK: - Lifecycle

    /// Retains the active editor; application-level termination saves all retained drafts.
    func start() {
        library.retainEditor(self)
    }

    /// Attempts a save on navigation; the library retains failed drafts for later recovery.
    func stop() {
        flush()
        library.releaseEditor(self)
    }
}
