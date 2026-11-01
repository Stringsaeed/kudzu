import AppKit
import KudzuCore
import Observation
import SwiftUI

/// Holds the draft of the snippet being edited and autosaves it shortly after the last change.
///
/// Edits go to `draft` only; the library snapshot (and so the list) updates when the debounce
/// fires. External changes (sync, import) are adopted only while there are no unsaved edits, so
/// a refresh never clobbers what the user is typing.
@MainActor @Observable
final class SnippetEditorModel {
    /// How long to wait after the last edit before saving.
    static let debounce: Duration = .milliseconds(400)

    private(set) var draft: SnippetRecord
    @ObservationIgnored private var persisted: SnippetRecord
    @ObservationIgnored private let library: LibraryModel
    @ObservationIgnored private var saveTask: Task<Void, Never>?
    @ObservationIgnored private var observers: [NSObjectProtocol] = []

    init(snippet: SnippetRecord, library: LibraryModel) {
        self.draft = snippet
        self.persisted = snippet
        self.library = library
    }

    /// Whether there are edits that haven't been saved yet.
    var isDirty: Bool { draft != persisted }

    /// Applies a change to the draft and schedules a save.
    func edit(_ body: (inout SnippetRecord) -> Void) {
        var copy = draft
        body(&copy)
        guard copy != draft else { return }
        draft = copy
        scheduleSave()
    }

    /// A binding to one field of the draft.
    func binding<Value>(_ keyPath: WritableKeyPath<SnippetRecord, Value>) -> Binding<Value> {
        Binding(
            get: { self.draft[keyPath: keyPath] },
            set: { newValue in self.edit { $0[keyPath: keyPath] = newValue } }
        )
    }

    /// Takes a newer stored version of the snippet unless the user has unsaved edits.
    func adoptExternal(_ record: SnippetRecord) {
        guard record != persisted, !isDirty else { return }
        persisted = record
        draft = record
    }

    /// Saves immediately if there is anything to save.
    func flush() {
        saveTask?.cancel()
        saveTask = nil
        guard isDirty else { return }
        var toSave = draft
        toSave.updatedAt = .now
        // On failure the draft stays dirty, the library shows the banner, and the next edit retries.
        if library.saveEdited(toSave) {
            draft.updatedAt = toSave.updatedAt
            persisted = draft
        }
    }

    private func scheduleSave() {
        saveTask?.cancel()
        saveTask = Task { [weak self] in
            try? await Task.sleep(for: Self.debounce)
            guard !Task.isCancelled else { return }
            self?.flush()
        }
    }

    // MARK: - Lifecycle

    /// Starts flushing on app deactivation and quit. Call from `onAppear`.
    func start() {
        guard observers.isEmpty else { return }
        for name in [NSApplication.willResignActiveNotification, NSApplication.willTerminateNotification] {
            observers.append(NotificationCenter.default.addObserver(forName: name, object: nil, queue: .main) { [weak self] _ in
                MainActor.assumeIsolated { self?.flush() }
            })
        }
    }

    /// Saves pending edits and stops observing. Call from `onDisappear`.
    func stop() {
        flush()
        for observer in observers { NotificationCenter.default.removeObserver(observer) }
        observers = []
    }
}
