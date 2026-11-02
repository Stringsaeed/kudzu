import CoreData
import Foundation
import Observation
import os
import SwiftData

/// Errors thrown by `KudzuStore` repository operations.
public enum KudzuStoreError: Error, Equatable, Sendable {
    /// The change would make a group its own ancestor.
    case groupCycle
    /// A referenced group doesn't exist.
    case groupNotFound(UUID)
}

/// What a dedupe pass did.
public struct DedupeReport: Hashable, Sendable {
    public var mergedGroups: Int
    public var mergedSnippets: Int
    /// Number of (snippet, device) usage rows rewritten.
    public var mergedUsageRows: Int
    /// Same trigger, different content; for the UI to surface.
    public var conflicts: [DedupeConflict]

    /// True when nothing was merged (conflicts may still exist).
    public var isEmpty: Bool { mergedGroups == 0 && mergedSnippets == 0 && mergedUsageRows == 0 }
}

/// The persistence layer: owns the SwiftData container and exposes a record-based repository.
///
/// Main-actor bound because it works on the container's `mainContext`
/// (`ModelContext` isn't `Sendable`). Callers on other actors `await` it and
/// exchange only `Sendable` records. `@Observable` so SwiftUI can watch
/// `changeToken` and `conflicts`.
@MainActor
@Observable
public final class KudzuStore {
    /// The underlying container, e.g. for `.modelContainer(_:)` in SwiftUI.
    @ObservationIgnored public let container: ModelContainer
    /// The main context all repository calls use.
    @ObservationIgnored public let context: ModelContext
    /// Whether a dedupe pass runs automatically after remote (CloudKit) changes.
    @ObservationIgnored public var dedupesAfterRemoteChanges = true
    /// Called after remote changes are coalesced (and deduped), before `changes` fires.
    @ObservationIgnored public var onRemoteChange: (@MainActor () -> Void)?
    /// Increments once per coalesced batch of changes; observable by polling or comparison.
    public private(set) var changeToken = 0
    /// Conflicts found by the latest dedupe pass.
    public private(set) var conflicts: [DedupeConflict] = []

    private static let log = Logger(subsystem: "sh.saeed.kudzu", category: "store")
    @ObservationIgnored private let debounce: Duration
    @ObservationIgnored private var subscribers: [UUID: AsyncStream<Void>.Continuation] = [:]
    @ObservationIgnored private var pendingFlush: Task<Void, Never>?
    @ObservationIgnored private var pendingRemote = false
    @ObservationIgnored private var cachedDeviceID: String?
    @ObservationIgnored private var observers: NotificationObservers?

    /// Wraps an existing container and starts observing its changes.
    public init(container: ModelContainer, debounce: Duration = KudzuStoreConstants.changeDebounce) {
        self.container = container
        self.context = container.mainContext
        self.debounce = debounce
        observeChanges()
    }

    /// Creates the container and the store in one step.
    public convenience init(syncEnabled: Bool, inMemory: Bool = false, storeDirectory: URL? = nil) throws {
        self.init(container: try Self.makeContainer(syncEnabled: syncEnabled, inMemory: inMemory, storeDirectory: storeDirectory))
    }

    /// An in-memory, never-synced store for tests and previews.
    public static func inMemory() throws -> KudzuStore {
        try KudzuStore(syncEnabled: false, inMemory: true)
    }

    // MARK: - Container

    /// Builds the container with a "Synced" configuration (CloudKit when `syncEnabled`) and a local-only one.
    ///
    /// - Parameter storeDirectory: where the `.store` files live; nil means
    ///   `Application Support/Kudzu`. The iOS app passes its App Group container.
    ///   Ignored when `inMemory`, which also never syncs.
    public nonisolated static func makeContainer(syncEnabled: Bool, inMemory: Bool, storeDirectory: URL?) throws -> ModelContainer {
        let syncedSchema = Schema(KudzuSchemaV1.syncedModels)
        let localSchema = Schema(KudzuSchemaV1.localModels)
        let cloudKit: ModelConfiguration.CloudKitDatabase =
            syncEnabled && !inMemory ? .private(KudzuStoreConstants.cloudKitContainerID) : .none

        let synced: ModelConfiguration
        let local: ModelConfiguration
        if inMemory {
            synced = ModelConfiguration(KudzuStoreConstants.syncedConfigurationName, schema: syncedSchema,
                                        isStoredInMemoryOnly: true, groupContainer: .none, cloudKitDatabase: .none)
            local = ModelConfiguration(KudzuStoreConstants.localConfigurationName, schema: localSchema,
                                       isStoredInMemoryOnly: true, groupContainer: .none, cloudKitDatabase: .none)
        } else {
            let directory = storeDirectory
                ?? URL.applicationSupportDirectory.appending(path: KudzuStoreConstants.defaultDirectoryName, directoryHint: .isDirectory)
            try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
            synced = ModelConfiguration(KudzuStoreConstants.syncedConfigurationName, schema: syncedSchema,
                                        url: directory.appending(path: KudzuStoreConstants.syncedStoreFileName),
                                        cloudKitDatabase: cloudKit)
            local = ModelConfiguration(KudzuStoreConstants.localConfigurationName, schema: localSchema,
                                       url: directory.appending(path: KudzuStoreConstants.localStoreFileName),
                                       cloudKitDatabase: .none)
        }
        return try ModelContainer(for: Schema(versionedSchema: KudzuSchemaV1.self), configurations: [synced, local])
    }

    // MARK: - Reading

    /// The whole synced library as records, in a stable order.
    public func library() throws -> KudzuLibrary {
        let groups = try context.fetch(FetchDescriptor<GroupModel>()).map(\.record).sorted { lhs, rhs in
            if lhs.sortIndex != rhs.sortIndex { return lhs.sortIndex < rhs.sortIndex }
            if lhs.name != rhs.name { return lhs.name < rhs.name }
            return lhs.id.uuidString < rhs.id.uuidString
        }
        let snippets = try context.fetch(FetchDescriptor<SnippetModel>()).map(\.record).sorted { lhs, rhs in
            lhs.createdAt != rhs.createdAt ? lhs.createdAt < rhs.createdAt : lhs.id.uuidString < rhs.id.uuidString
        }
        let usage = try context.fetch(FetchDescriptor<UsageModel>()).map(\.record).sorted { lhs, rhs in
            lhs.snippetID != rhs.snippetID ? lhs.snippetID.uuidString < rhs.snippetID.uuidString : lhs.deviceID < rhs.deviceID
        }
        return KudzuLibrary(groups: groups, snippets: snippets, usage: usage)
    }

    /// Usage stats summed across devices, using the synced typing speed.
    public func statsSummary() throws -> StatsSummary {
        let library = try library()
        return StatsCalculator.summary(usage: library.usage, snippets: library.snippets,
                                       typingSpeedWPM: try syncedSettings().typingSpeedWPM)
    }

    // MARK: - Writing

    /// Inserts the snippet or updates the one with the same ID. An unknown `groupID` files it at the root.
    public func upsert(_ snippet: SnippetRecord) throws {
        let model: SnippetModel
        if let existing = try snippetModel(id: snippet.id) {
            model = existing
            model.update(from: snippet)
        } else {
            model = SnippetModel(snippet)
            context.insert(model)
        }
        let group = try snippet.groupID.flatMap(groupModel(id:))
        if model.group !== group { model.group = group }
        try save()
    }

    /// Inserts the group or updates the one with the same ID. Throws `groupCycle` if it would nest inside itself.
    public func upsert(_ group: GroupRecord) throws {
        let parent = try group.parentID.flatMap(groupModel(id:))
        if group.parentID == group.id { throw KudzuStoreError.groupCycle }
        let model: GroupModel
        if let existing = try groupModel(id: group.id) {
            if let parent, Self.isDescendantOrSelf(parent, of: existing) { throw KudzuStoreError.groupCycle }
            model = existing
            model.update(from: group)
        } else {
            model = GroupModel(group)
            context.insert(model)
        }
        if model.parent !== parent { model.parent = parent }
        try save()
    }

    /// Deletes a snippet and every device's usage rows for it.
    public func deleteSnippet(id: UUID) throws {
        for model in try context.fetch(FetchDescriptor<SnippetModel>(predicate: #Predicate { $0.id == id })) {
            context.delete(model)
        }
        for row in try context.fetch(FetchDescriptor<UsageModel>(predicate: #Predicate { $0.snippetID == id })) {
            context.delete(row)
        }
        try save()
    }

    /// Deletes a group after moving its child groups and snippets to `newParentID` (nil = top level).
    public func deleteGroup(id: UUID, moveChildrenTo newParentID: UUID?) throws {
        guard let group = try groupModel(id: id) else { return }
        var target: GroupModel?
        if let newParentID {
            guard let found = try groupModel(id: newParentID) else { throw KudzuStoreError.groupNotFound(newParentID) }
            if Self.isDescendantOrSelf(found, of: group) { throw KudzuStoreError.groupCycle }
            target = found
        }
        for child in group.children ?? [] { child.parent = target }
        for snippet in group.snippets ?? [] { snippet.group = target }
        context.delete(group)
        try save()
    }

    /// Replaces the whole synced library (snippets, groups, usage) with `library`, keeping its IDs. Used for restore.
    public func replaceAll(with library: KudzuLibrary) throws {
        try deleteAll(SnippetModel.self)
        try deleteAll(GroupModel.self)
        try deleteAll(UsageModel.self)
        try insertGroups(library.groups, existing: [:])
        let groupsByID = try groupsByID()
        for record in library.snippets {
            let model = SnippetModel(record)
            context.insert(model)
            model.group = record.groupID.flatMap { groupsByID[$0] }
        }
        for record in library.usage { context.insert(UsageModel(record)) }
        try save()
    }

    /// Adds `library` to the store (import), then dedupes.
    ///
    /// Records whose IDs already exist are kept unless the incoming snippet has
    /// a newer `updatedAt`. Usage rows for an existing (snippet, device) take the
    /// max of each counter: they're two snapshots of the same counter (e.g. this
    /// device's own backup), so summing would double count.
    @discardableResult
    public func merge(_ library: KudzuLibrary) throws -> DedupeReport {
        let existingGroups = try groupsByID()
        try insertGroups(library.groups.filter { existingGroups[$0.id] == nil }, existing: existingGroups)
        let groups = try groupsByID()

        var snippets = Dictionary(try context.fetch(FetchDescriptor<SnippetModel>()).map { ($0.id, $0) }, uniquingKeysWith: { a, _ in a })
        for record in library.snippets {
            if let existing = snippets[record.id] {
                guard record.updatedAt > existing.updatedAt else { continue }
                existing.update(from: record)
                let group = record.groupID.flatMap { groups[$0] }
                if existing.group !== group { existing.group = group }
            } else {
                let model = SnippetModel(record)
                context.insert(model)
                model.group = record.groupID.flatMap { groups[$0] }
                snippets[record.id] = model
            }
        }

        struct UsageKey: Hashable { var snippet: UUID; var device: String }
        var rows = Dictionary(try context.fetch(FetchDescriptor<UsageModel>()).map { (UsageKey(snippet: $0.snippetID, device: $0.deviceID), $0) },
                              uniquingKeysWith: { a, _ in a })
        for record in library.usage {
            let key = UsageKey(snippet: record.snippetID, device: record.deviceID)
            if let row = rows[key] {
                row.update(from: UsageRecord(
                    snippetID: row.snippetID, deviceID: row.deviceID,
                    count: max(row.count, record.count), charsSaved: max(row.charsSaved, record.charsSaved),
                    lastUsedAt: StatsCalculator.latest(row.lastUsedAt, record.lastUsedAt)
                ))
            } else {
                let row = UsageModel(record)
                context.insert(row)
                rows[key] = row
            }
        }
        try save()
        return try dedupe()
    }

    // MARK: - Usage

    /// This device's identifier for usage rows, generated once and kept in local settings.
    public func deviceID() throws -> String {
        if let cachedDeviceID { return cachedDeviceID }
        let id = try localSettings().deviceID
        cachedDeviceID = id
        return id
    }

    /// Stamps `hardwareID` on this Mac, or mints a new `deviceID` when the library was restored onto different hardware.
    ///
    /// An empty `hardwareID` (the probe failed) leaves the existing device id alone.
    /// The first successful probe records the UUID without rotating the id, so a normal install keeps its stats.
    @discardableResult
    public func reconcileDevice(hardwareID: String) throws -> String {
        let settings = try localSettings()
        let hardwareID = hardwareID.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !hardwareID.isEmpty else {
            cachedDeviceID = settings.deviceID
            return settings.deviceID
        }
        if settings.hardwareID.isEmpty {
            settings.hardwareID = hardwareID
            try save()
        } else if settings.hardwareID != hardwareID {
            settings.hardwareID = hardwareID
            settings.deviceID = UUID().uuidString
            try save()
            Self.log.info("Hardware changed; minted a new device ID so usage rows stay per Mac")
        }
        cachedDeviceID = settings.deviceID
        return settings.deviceID
    }

    /// Counts one expansion on this device only, creating its usage row if needed.
    public func recordUsage(snippetID: UUID, charsSaved: Int, at date: Date = .now) throws {
        let device = try deviceID()
        let rows = try context.fetch(FetchDescriptor<UsageModel>(
            predicate: #Predicate { $0.snippetID == snippetID && $0.deviceID == device }
        )).sorted { $0.id.uuidString < $1.id.uuidString }
        let row: UsageModel
        if let first = rows.first {
            row = first
        } else {
            row = UsageModel(UsageRecord(snippetID: snippetID, deviceID: device))
            context.insert(row)
        }
        row.count += 1
        row.charsSaved += max(0, charsSaved)
        row.lastUsedAt = StatsCalculator.latest(row.lastUsedAt, date)
        try save()
    }

    // MARK: - Settings

    /// The synced settings singleton, created with defaults if missing.
    public func syncedSettings() throws -> SyncedSettingsModel {
        if let existing = try sortedSyncedSettings().first { return existing }
        let settings = SyncedSettingsModel()
        context.insert(settings)
        try save()
        return settings
    }

    /// Edits synced settings and stamps `updatedAt` so the edit wins when duplicates are collapsed.
    public func updateSyncedSettings(_ body: (SyncedSettingsModel) throws -> Void) throws {
        let settings = try syncedSettings()
        try body(settings)
        settings.updatedAt = .now
        try save()
    }

    /// Edits this Mac's local settings and refreshes the cached device id.
    public func updateLocalSettings(_ body: (LocalSettingsModel) throws -> Void) throws {
        let settings = try localSettings()
        try body(settings)
        try save()
        cachedDeviceID = settings.deviceID
    }

    /// This device's local settings, created (with a fresh device ID) if missing.
    public func localSettings() throws -> LocalSettingsModel {
        let all = try context.fetch(FetchDescriptor<LocalSettingsModel>()).sorted { $0.id.uuidString < $1.id.uuidString }
        if let existing = all.first(where: { !$0.deviceID.isEmpty }) ?? all.first {
            if existing.deviceID.isEmpty {
                existing.deviceID = UUID().uuidString
                try save()
            }
            return existing
        }
        let settings = LocalSettingsModel(deviceID: UUID().uuidString)
        context.insert(settings)
        try save()
        return settings
    }

    // MARK: - Dedupe

    /// Merges duplicate snippets, groups, usage rows and synced settings. Safe to run any time; idempotent.
    @discardableResult
    public func dedupe() throws -> DedupeReport {
        let settings = try sortedSyncedSettings()
        for extra in settings.dropFirst() { context.delete(extra) }

        let snippetModels = try context.fetch(FetchDescriptor<SnippetModel>())
        var aliases: [UUID: UUID] = [:]
        for model in snippetModels.sorted(by: { $0.id.uuidString > $1.id.uuidString }) {
            // Descending, so the smallest survivor ID wins if two claim the same alias.
            for old in model.mergedIDs { aliases[old] = model.id }
        }

        let plan = Deduplicator.plan(for: try library(), aliases: aliases)
        conflicts = plan.conflicts
        if !plan.isEmpty { try apply(plan, snippetModels: snippetModels) }
        try save()
        return DedupeReport(
            mergedGroups: plan.groupMerges.count,
            mergedSnippets: plan.snippetMerges.count,
            mergedUsageRows: plan.usageMerges.count,
            conflicts: plan.conflicts
        )
    }

    private func apply(_ plan: DedupePlan, snippetModels: [SnippetModel]) throws {
        let groups = try groupsByID()
        for record in plan.updatedGroups {
            guard let model = groups[record.id] else { continue }
            model.update(from: record)
            let parent = record.parentID.flatMap { groups[$0] }
            if model.parent !== parent { model.parent = parent }
        }

        let snippets = Dictionary(snippetModels.map { ($0.id, $0) }, uniquingKeysWith: { a, _ in a })
        for record in plan.updatedSnippets {
            guard let model = snippets[record.id] else { continue }
            model.update(from: record)
            let group = record.groupID.flatMap { groups[$0] }
            if model.group !== group { model.group = group }
        }
        for (loserID, survivorID) in plan.snippetMerges {
            guard let loser = snippets[loserID] else { continue }
            if let survivor = snippets[survivorID] {
                let ids = Set(survivor.mergedIDs + [loserID] + loser.mergedIDs)
                survivor.mergedIDs = ids.sorted { $0.uuidString < $1.uuidString }
            }
            context.delete(loser)
        }
        for loserID in plan.groupMerges.keys {
            if let loser = groups[loserID] { context.delete(loser) }
        }

        let rows = try context.fetch(FetchDescriptor<UsageModel>())
        for merge in plan.usageMerges {
            // Keep the row with the smallest ID so devices applying the same plan
            // rewrite the same CloudKit record instead of each creating a new one.
            let matching = rows
                .filter { $0.deviceID == merge.deviceID && merge.sourceSnippetIDs.contains($0.snippetID) }
                .sorted { $0.id.uuidString < $1.id.uuidString }
            guard let keep = matching.first else { continue }
            keep.update(from: merge.record)
            for extra in matching.dropFirst() { context.delete(extra) }
        }
    }

    // MARK: - Change observation

    /// A new stream that yields once per coalesced batch of local saves or remote imports.
    ///
    /// Each access creates an independent subscription; cancel the consuming task to end it.
    public var changes: AsyncStream<Void> {
        let (stream, continuation) = AsyncStream<Void>.makeStream(bufferingPolicy: .bufferingNewest(1))
        let key = UUID()
        subscribers[key] = continuation
        continuation.onTermination = { [weak self] _ in
            Task { @MainActor in self?.subscribers[key] = nil }
        }
        return stream
    }

    /// Reports a remote change (normally observed automatically); runs dedupe and `onRemoteChange` after the debounce.
    public func noteRemoteChange() {
        scheduleChange(remote: true)
    }

    private func observeChanges() {
        let observers = NotificationObservers()
        let containerID = ObjectIdentifier(container)
        observers.add(ModelContext.didSave) { [weak self] note in
            guard let context = note.object as? ModelContext,
                  ObjectIdentifier(context.container) == containerID else { return }
            MainActor.assumeIsolated { self?.scheduleChange(remote: false) }
        }
        // CloudKit imports land via the persistent store, not a ModelContext, so
        // they only show up as remote-change notifications for the synced store.
        if let syncedURL = container.configurations
            .first(where: { $0.name == KudzuStoreConstants.syncedConfigurationName && !$0.isStoredInMemoryOnly })?
            .url.standardizedFileURL {
            observers.add(.NSPersistentStoreRemoteChange) { [weak self] note in
                guard let url = note.userInfo?[NSPersistentStoreURLKey] as? URL,
                      url.standardizedFileURL == syncedURL else { return }
                MainActor.assumeIsolated { self?.scheduleChange(remote: true) }
            }
        }
        self.observers = observers
    }

    private func scheduleChange(remote: Bool) {
        pendingRemote = pendingRemote || remote
        pendingFlush?.cancel()
        pendingFlush = Task { [weak self, debounce] in
            try? await Task.sleep(for: debounce)
            guard !Task.isCancelled else { return }
            self?.flushChange()
        }
    }

    private func flushChange() {
        pendingFlush = nil
        let remote = pendingRemote
        pendingRemote = false
        if remote {
            if dedupesAfterRemoteChanges {
                do { try dedupe() } catch { Self.log.error("Dedupe after remote change failed: \(error)") }
            }
            onRemoteChange?()
        }
        changeToken &+= 1
        for continuation in subscribers.values { continuation.yield() }
    }

    // MARK: - Helpers

    private func save() throws {
        if context.hasChanges { try context.save() }
    }

    private func snippetModel(id: UUID) throws -> SnippetModel? {
        try context.fetch(FetchDescriptor<SnippetModel>(predicate: #Predicate { $0.id == id })).first
    }

    private func groupModel(id: UUID) throws -> GroupModel? {
        try context.fetch(FetchDescriptor<GroupModel>(predicate: #Predicate { $0.id == id })).first
    }

    private func groupsByID() throws -> [UUID: GroupModel] {
        Dictionary(try context.fetch(FetchDescriptor<GroupModel>()).map { ($0.id, $0) }, uniquingKeysWith: { a, _ in a })
    }

    private func deleteAll<T: PersistentModel>(_: T.Type) throws {
        for model in try context.fetch(FetchDescriptor<T>()) { context.delete(model) }
    }

    /// Most recently edited first (never-edited last), then smallest ID: the same order on every device.
    private func sortedSyncedSettings() throws -> [SyncedSettingsModel] {
        try context.fetch(FetchDescriptor<SyncedSettingsModel>()).sorted { lhs, rhs in
            let l = lhs.updatedAt ?? .distantPast, r = rhs.updatedAt ?? .distantPast
            return l != r ? l > r : lhs.id.uuidString < rhs.id.uuidString
        }
    }

    /// Inserts groups in two passes so children may precede parents; skips parent links that would form a cycle.
    private func insertGroups(_ records: [GroupRecord], existing: [UUID: GroupModel]) throws {
        var all = existing
        var inserted: [(GroupRecord, GroupModel)] = []
        for record in records where all[record.id] == nil {
            let model = GroupModel(record)
            context.insert(model)
            all[record.id] = model
            inserted.append((record, model))
        }
        for (record, model) in inserted {
            guard let parentID = record.parentID, let parent = all[parentID],
                  !Self.isDescendantOrSelf(parent, of: model) else { continue }
            model.parent = parent
        }
    }

    /// True if `candidate` is `group` or sits anywhere below it.
    private static func isDescendantOrSelf(_ candidate: GroupModel, of group: GroupModel) -> Bool {
        var current: GroupModel? = candidate
        var seen: Set<PersistentIdentifier> = []
        while let node = current, seen.insert(node.persistentModelID).inserted {
            if node === group { return true }
            current = node.parent
        }
        return false
    }
}

/// Removes its NotificationCenter observers when the owning store goes away.
private final class NotificationObservers: @unchecked Sendable {
    // Only mutated during `KudzuStore.init`, before the instance is shared.
    private var tokens: [any NSObjectProtocol] = []

    func add(_ name: Notification.Name, _ handler: @escaping @Sendable (Notification) -> Void) {
        tokens.append(NotificationCenter.default.addObserver(forName: name, object: nil, queue: .main, using: handler))
    }

    deinit {
        for token in tokens { NotificationCenter.default.removeObserver(token) }
    }
}
