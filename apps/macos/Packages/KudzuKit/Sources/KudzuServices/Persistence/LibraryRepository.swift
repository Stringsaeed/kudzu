import Foundation
import KudzuCore
import SwiftData
import CoreData

/// The one owner of the SwiftData library store.
///
/// Its fresh per-operation `ModelContext` lives on this actor's serial executor and never leaves
/// it. Callers exchange Core values only. Every mutation takes the current `Library` value, runs
/// the caller's closure (the same `LibraryEditing` calls `DataStore.editLibrary` callers pass
/// today), applies the difference to rows inside one transaction, and publishes the new value
/// only after the save succeeded. Mutation failures roll back. A subsequent refresh failure
/// is reported separately and never turns a committed command into a retryable rejection.
///
/// Synchronous callers (the agent socket's `Agent.Host.library()`, the engine's
/// `formRequested`) read ``snapshot``: a lock-protected copy of the last committed value. No
/// database work happens on their threads, and none happens inside the event-tap callback.
public actor LibraryRepository {
    public nonisolated let modelContainer: ModelContainer
    var modelContext: ModelContext { let context = ModelContext(modelContainer); context.autosaveEnabled = false; return context }
    public init(modelContainer: ModelContainer) { self.modelContainer = modelContainer }
    private nonisolated let published = LibrarySnapshotBox()
    /// Authoritative committed value, actor-isolated.
    private var current = Library.empty
    /// Test-only: throw after rows were changed and before the transaction saved.
    private var faultBeforeSave = false

    // MARK: Opening

    /// Opens (or creates) the store described by `configuration` and loads the first snapshot.
    /// Fails without touching the store when the configuration names the legacy container.
    public static func open(_ configuration: LibraryStoreConfiguration) async throws -> LibraryRepository {
        let schema = KudzuLibrarySchemaV1.schema
        let modelConfiguration = try configuration.modelConfiguration(schema: schema)
        let container = try ModelContainer(for: schema, configurations: [modelConfiguration])
        let repository = LibraryRepository(modelContainer: container)
        try await repository.reload()
        return repository
    }

    /// Re-reads every row and replaces the published snapshot with what is on disk.
    @discardableResult
    public func reload() throws -> Library {
        let library = try loadLibrary()
        let changed = try library != current || ( KudzuJSON.encoder(pretty: false).encode(library) != KudzuJSON.encoder(pretty: false).encode(current))
        current = library
        published.clearRefreshFailure()
        if changed { published.publish(library) }
        return library
    }

    // MARK: Reading

    /// The last committed library. Safe from any thread; never blocks on the store.
    public nonisolated var snapshot: Library { published.current }
    public nonisolated var lastRefreshFailure: String? { published.lastRefreshFailure }

    /// Every library committed after this call, in commit order.
    public nonisolated var changes: AsyncStream<Library> { published.stream() }
    public nonisolated var changesIncludingCurrent: AsyncStream<Library> { published.stream(includeCurrent: true) }
    public nonisolated func observeRemoteChanges(onError: @escaping @Sendable (String) -> Void) -> Task<Void, Never> {
        published.setRefreshFailureHandler(onError)
        let signals = AsyncStream<Void> { continuation in
            let observer = NotificationCenter.default.addObserver(forName: .NSPersistentStoreRemoteChange, object: nil, queue: nil) { _ in continuation.yield(()) }
            let token = NotificationToken(observer)
            continuation.onTermination = { _ in token.cancel() }
        }
        return Task { [weak self] in
            for await _ in signals {
                guard let self else { return }
                do { _ = try await self.reload() } catch { onError(error.localizedDescription) }
            }
        }
    }

    /// Rebuilds the `Library` value from rows. Order is `listOrder`, then id for rows that share a
    /// position (which only mirroring could produce). Two rows with one identity are an error.
    public func loadLibrary() throws -> Library {
        try readLibrary(in: modelContext)
    }

    private func readLibrary(in context: ModelContext) throws -> Library {
        let snippetRows = try context.fetch(FetchDescriptor<PersistedSnippet>())
            .sorted { ($0.listOrder, $0.id.uuidString) < ($1.listOrder, $1.id.uuidString) }
        let groupRows = try context.fetch(FetchDescriptor<PersistedGroup>())
            .sorted { ($0.listOrder, $0.id.uuidString) < ($1.listOrder, $1.id.uuidString) }
        let usageRows = try context.fetch(FetchDescriptor<PersistedUsage>())
            .sorted { ($0.listOrder, $0.snippetID.uuidString, $0.deviceIDUTF8.base64EncodedString()) < ($1.listOrder, $1.snippetID.uuidString, $1.deviceIDUTF8.base64EncodedString()) }

        do {
            try requireDistinct(snippetRows.map(\.id.lowercased), entity: "PersistedSnippet")
            try requireDistinct(groupRows.map(\.id.lowercased), entity: "PersistedGroup")
            try requireDistinct(usageRows.map { $0.snippetID.lowercased + "|" + $0.deviceIDUTF8.base64EncodedString() }, entity: "PersistedUsage")
        } catch {
            // Preserve every conflicting payload, including repeated identities, for explicit
            // recovery. Never silently pick a winning row or delete a cloud record.
            if let storeURL = modelContainer.configurations.first?.url {
                let recovery = Library(formatVersion: Library.currentFormatVersion,
                    groups: try groupRows.map(LibraryRowCodec.group(from:)),
                    snippets: try snippetRows.map(LibraryRowCodec.snippet(from:)),
                    usage: try usageRows.map(LibraryRowCodec.usage(from:)))
                let url = storeURL.deletingLastPathComponent().appendingPathComponent("library-conflicts-" + UUID().uuidString + ".json")
                try AtomicFile.write(try KudzuJSON.encoder(pretty: true).encode(recovery), to: url, mode: 0o600)
                throw ServiceError("Duplicate library identities; editing paused. Every payload was retained at \(url.path). Restore a verified store backup or resolve identities in this recovery export before importing it into a clean store. \(error.localizedDescription)")
            }
            throw error
        }

        return Library(
            formatVersion: Library.currentFormatVersion,
            groups: try groupRows.map(LibraryRowCodec.group(from:)),
            snippets: try snippetRows.map(LibraryRowCodec.snippet(from:)),
            usage: try usageRows.map(LibraryRowCodec.usage(from:))
        )
    }

    /// Row counts straight from the store, for tests that must not trust the value layer.
    func rowCounts() throws -> (snippets: Int, groups: Int, usage: Int) {
        (
            try modelContext.fetchCount(FetchDescriptor<PersistedSnippet>()),
            try modelContext.fetchCount(FetchDescriptor<PersistedGroup>()),
            try modelContext.fetchCount(FetchDescriptor<PersistedUsage>())
        )
    }

    // MARK: Writing

    /// Mutates the library through `body` and commits the result in one transaction. A throw from
    /// `body`, a failed save, or an injected fault leaves everything unchanged and rethrows.
    @discardableResult
    public func edit<T: Sendable>(_ body: @Sendable (inout Library) throws -> T) throws -> T {
        _ = try reload()
        var next = current
        let value = try body(&next)
        try commit(next, baseline: current)
        return value
    }

    /// Replaces the stored rows with `library`: id-keyed upsert plus deletion of absent ids, in
    /// one transaction. Importing the same library again changes nothing and adds no rows.
    public func importLibrary(_ library: Library) throws {
        _ = try reload()
        try commit(library)
    }

    /// `DataStore.recordUsage`'s rule, unchanged: one row per snippet per device.
    public func recordUsage(snippetID: UUID, deviceID: String, charsSaved: Int, now: Date = Date()) throws {
        try edit { library in
            if let i = library.usage.firstIndex(where: { $0.snippetID == snippetID && Data($0.deviceID.utf8) == Data(deviceID.utf8) }) {
                library.usage[i].count += 1
                library.usage[i].charsSaved += max(0, charsSaved)
                library.usage[i].lastUsedAt = now
            } else {
                library.usage.append(UsageRecord(snippetID: snippetID, deviceID: deviceID, count: 1, charsSaved: max(0, charsSaved), lastUsedAt: now))
            }
        }
    }

    /// Test-only. While set, the next commit throws after changing rows and before saving.
    func testingSetFaultBeforeSave(_ on: Bool) {
        faultBeforeSave = on
    }

    // MARK: Internal

    private func commit(_ next: Library, baseline: Library? = nil) throws {
        guard next.formatVersion == Library.currentFormatVersion else {
            throw LibraryRepositoryError.unsupportedFormatVersion(next.formatVersion)
        }
        try requireDistinct(next.snippets.map(\.id.lowercased), entity: "Library.snippets")
        try requireDistinct(next.groups.map(\.id.lowercased), entity: "Library.groups")
        // Value equality checks dates without JSON's whole-second rounding; encoded bytes
        // additionally distinguish canonically equivalent Unicode in every nested text field.
        let encoder = KudzuJSON.encoder(pretty: false)
        if next == current {
            if try encoder.encode(next) == encoder.encode(current) { return }
        }
        let context = modelContext
        var committed: Library?
        do {
            try context.transaction {
                try applyRows(next, baseline: baseline, in: context)
                if faultBeforeSave {
                    throw LibraryRepositoryError.injectedFault
                }
                // Capture the actual staged rows, including external fields preserved by the
                // delta write, rather than publishing the caller's pre-merge Library value.
                committed = try readLibrary(in: context)
            }
        } catch {
            // `transaction` saves on success. On any throw, discard whatever was staged so the
            // context matches the store again; nothing was committed.
            context.rollback()
            throw error
        }
        if let committed {
            current = committed
            published.publish(committed)
        }
        do { _ = try reload() }
        catch {
            published.reportRefreshFailure(error.localizedDescription)
            Logging.logger(category: "store").error("Library saved; refresh failed: \(error.localizedDescription)")
        }
    }

    private func applyRows(_ next: Library, baseline: Library?, in context: ModelContext) throws {
        let snippetRows = try context.fetch(FetchDescriptor<PersistedSnippet>())
        let groupRows = try context.fetch(FetchDescriptor<PersistedGroup>())
        let usageRows = try context.fetch(FetchDescriptor<PersistedUsage>())
        // Snippets by id.
        var snippetsByID = try index(snippetRows, entity: "PersistedSnippet") { $0.id.lowercased }
        for (position, snippet) in next.snippets.enumerated() {
            let beforeIndex = baseline?.snippets.firstIndex(where: { $0.id == snippet.id })
            let before = beforeIndex.map { baseline!.snippets[$0] }
            if snippetsByID[snippet.id.lowercased] == nil, let before {
                guard try sameValue(snippet, before) else {
                    throw LibraryRepositoryError.deletedDuringEdit(entity: "snippet", id: snippet.id.lowercased)
                }
                continue
            }
            let wasPresent = snippetsByID[snippet.id.lowercased] != nil
            let row: PersistedSnippet
            if let existing = snippetsByID.removeValue(forKey: snippet.id.lowercased) {
                row = existing
            } else {
                row = PersistedSnippet()
                context.insert(row)
            }
            try LibraryRowCodec.apply(snippet, listOrder: position, to: row, baseline: wasPresent ? before : nil, baselineOrder: beforeIndex)
        }
        for orphan in snippetsByID.values {
            if baseline == nil || baseline!.snippets.contains(where: { $0.id == orphan.id }) { context.delete(orphan) }
        }

        // Groups by id.
        var groupsByID = try index(groupRows, entity: "PersistedGroup") { $0.id.lowercased }
        for (position, group) in next.groups.enumerated() {
            let beforeIndex = baseline?.groups.firstIndex(where: { $0.id == group.id })
            let before = beforeIndex.map { baseline!.groups[$0] }
            if groupsByID[group.id.lowercased] == nil, let before {
                guard try sameValue(group, before) else {
                    throw LibraryRepositoryError.deletedDuringEdit(entity: "group", id: group.id.lowercased)
                }
                continue
            }
            let wasPresent = groupsByID[group.id.lowercased] != nil
            let row: PersistedGroup
            if let existing = groupsByID.removeValue(forKey: group.id.lowercased) {
                row = existing
            } else {
                row = PersistedGroup()
                context.insert(row)
            }
            try LibraryRowCodec.apply(group, listOrder: position, to: row, baseline: wasPresent ? before : nil, baselineOrder: beforeIndex)
        }
        for orphan in groupsByID.values {
            if baseline == nil || baseline!.groups.contains(where: { $0.id == orphan.id }) { context.delete(orphan) }
        }

        // Usage by (snippet, device).
        var usageByKey = try index(usageRows, entity: "PersistedUsage") { $0.snippetID.lowercased + "|" + $0.deviceIDUTF8.base64EncodedString() }
        var seenUsageKeys = Set<String>()
        for (position, usage) in next.usage.enumerated() {
            let key = LibraryRowCodec.usageKey(snippetID: usage.snippetID, deviceID: usage.deviceID)
            guard seenUsageKeys.insert(key).inserted else {
                throw LibraryRepositoryError.duplicateIdentity(entity: "Library.usage", keys: [key])
            }
            let beforeIndex = baseline?.usage.firstIndex(where: { LibraryRowCodec.usageKey(snippetID: $0.snippetID, deviceID: $0.deviceID) == key })
            let before = beforeIndex.map { baseline!.usage[$0] }
            if usageByKey[key] == nil, let before {
                guard try sameValue(usage, before) else {
                    throw LibraryRepositoryError.deletedDuringEdit(entity: "usage", id: key)
                }
                continue
            }
            let wasPresent = usageByKey[key] != nil
            let row: PersistedUsage
            if let existing = usageByKey.removeValue(forKey: key) {
                row = existing
            } else {
                row = PersistedUsage()
                context.insert(row)
            }
            LibraryRowCodec.apply(usage, listOrder: position, to: row, baseline: wasPresent ? before : nil, baselineOrder: beforeIndex)
        }
        for (key, orphan) in usageByKey {
            if baseline == nil || baseline!.usage.contains(where: { LibraryRowCodec.usageKey(snippetID: $0.snippetID, deviceID: $0.deviceID) == key }) { context.delete(orphan) }
        }
    }

    private func sameValue<T: Encodable & Equatable>(_ lhs: T, _ rhs: T) throws -> Bool {
        guard lhs == rhs else { return false }
        let encoder = KudzuJSON.encoder(pretty: false)
        return try encoder.encode(lhs) == encoder.encode(rhs)
    }

    private func index<Row>(_ rows: [Row], entity: String, key: (Row) -> String) throws -> [String: Row] {
        var result: [String: Row] = [:]
        var duplicates: [String] = []
        for row in rows {
            let k = key(row)
            if result.updateValue(row, forKey: k) != nil { duplicates.append(k) }
        }
        if !duplicates.isEmpty {
            throw LibraryRepositoryError.duplicateIdentity(entity: entity, keys: duplicates.sorted())
        }
        return result
    }

    private func requireDistinct(_ keys: [String], entity: String) throws {
        var seen = Set<String>()
        let duplicates = keys.filter { !seen.insert($0).inserted }
        if !duplicates.isEmpty {
            throw LibraryRepositoryError.duplicateIdentity(entity: entity, keys: Array(Set(duplicates)).sorted())
        }
    }
}

/// The committed snapshot and its observers. Lock-protected so engine and socket threads can
/// read it without touching the actor or the `ModelContext`.
final class LibrarySnapshotBox: @unchecked Sendable {
    private let lock = NSLock()
    private var value = Library.empty
    private var continuations: [UUID: AsyncStream<Library>.Continuation] = [:]
    private var refreshFailure: String?
    private var refreshFailureHandler: (@Sendable (String) -> Void)?

    var lastRefreshFailure: String? { lock.withLock { refreshFailure } }
    func clearRefreshFailure() { lock.withLock { refreshFailure = nil } }
    func setRefreshFailureHandler(_ handler: @escaping @Sendable (String) -> Void) {
        let failure = lock.withLock { refreshFailureHandler = handler; return refreshFailure }
        if let failure { handler(failure) }
    }
    func reportRefreshFailure(_ message: String) {
        let handler = lock.withLock { refreshFailure = message; return refreshFailureHandler }
        handler?(message)
    }

    var current: Library {
        lock.lock()
        defer { lock.unlock() }
        return value
    }

    func publish(_ library: Library) {
        lock.lock()
        value = library
        let targets = Array(continuations.values)
        for continuation in targets { continuation.yield(library) }
        lock.unlock()
    }

    func stream(includeCurrent: Bool = false) -> AsyncStream<Library> {
        let token = UUID()
        return AsyncStream { continuation in
            lock.lock()
            if includeCurrent { continuation.yield(value) }
            continuations[token] = continuation
            lock.unlock()
            continuation.onTermination = { [weak self] _ in
                guard let self else { return }
                self.lock.lock()
                self.continuations[token] = nil
                self.lock.unlock()
            }
        }
    }
}

extension LibraryRepository {
    /// Dynamic JSON and the synchronous request host never leave this actor invocation.
    public func handleAgent<H: Agent.Host & Sendable>(message: String, version: String, host: H) throws -> String? {
        _ = try reload()
        let request = RepositoryAgentHost(base: host, library: current)
        let response = Agent.handle(host: request, serverVersion: version, message: message)
        do { try commit(request.value, baseline: current) }
        catch {
            request.failure = error.localizedDescription
            return Agent.handle(host: request, serverVersion: version, message: message)
        }
        return response
    }
}

private final class RepositoryAgentHost<H: Agent.Host>: Agent.Host {
    let base: H
    var value: Library
    var failure: String?
    init(base: H, library: Library) { self.base = base; value = library }
    func access() throws -> Agent.Access { try base.access() }
    func library() throws -> Library { value }
    func editLibrary(_ edit: (inout Library) throws -> [String: Any]) throws -> [String: Any] {
        if let failure { throw Agent.Failure(failure) }
        return try edit(&value)
    }
    func clipboard() throws -> [ClipEntry] { try base.clipboard() }
    func clipboardText(_ id: UUID) throws -> String? { try base.clipboardText(id) }
    func renderEnv() -> Agent.RenderEnv { base.renderEnv() }
}

private final class NotificationToken: @unchecked Sendable {
    let token: NSObjectProtocol
    init(_ token: NSObjectProtocol) { self.token = token }
    func cancel() { NotificationCenter.default.removeObserver(token) }
}
