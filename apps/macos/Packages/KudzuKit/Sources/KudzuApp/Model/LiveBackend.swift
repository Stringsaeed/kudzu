import Foundation
import KudzuCore
import KudzuEngine
import KudzuServices

/// The real backend: `src-tauri/src/state.rs` (`AppCore`) plus every command of
/// `commands.rs`, `clipboard/mod.rs`, `agent.rs` and `permissions/mod.rs`, wired to the services.
///
/// It owns the store, the engine and the services; windows belong to ``AppShell``, which this
/// reaches through `shell`. Events the Rust emitted (`kudzu://library`, …) go out on `bus`.
///
/// Main-actor isolated (window commands must be), but the library/settings plumbing
/// (``libraryChanged()``, ``settingsChanged()``) is `nonisolated`: it only touches thread-safe
/// services, so the engine host, the agent socket and the sync engine call it from their threads.
@MainActor
public final class LiveBackend: Backend {
    public let identity: AppIdentity
    public let store: LocalStore
    public let engine: Engine
    public let clipboard: ClipboardHistory
    public let hotkeys = HotkeyCenter()
    public let updater: Updater
    public let cloud: AutomaticCloudStatus
    public let agent: AgentServer
    let bus = EventBroadcaster()
    let log = Logging.logger(category: "app")
    private var backupTask: Task<Void, Never>?
    /// Serializes durable library publishes and engine installs. Counters are touched only on `queue`.
    private let commits = LibraryCommits()

    private final class LibraryCommits: @unchecked Sendable {
        let queue = DispatchQueue(label: "sh.saeed.kudzu.library-publish")
        var publishedRevision: UInt64 = 0
        var engineGeneration: UInt64 = 0
    }

    /// The windows. Weak: the shell owns the backend's lifetime through the delegate.
    weak var shell: AppShell?

    // MARK: Window hand-offs (state.rs)

    /// The app that was frontmost when the palette opened.
    var paletteTarget: AppInfo?
    /// The app that was frontmost when the clipboard picker opened (paste goes there).
    var clipTarget: AppInfo?
    /// What the form panel shows: the engine's request plus remembered values and the template.
    var pendingForm: FormPayload?
    /// The permission the companion guides.
    var permissionKind: PermissionKind = .accessibility

    public nonisolated var events: AsyncStream<BackendEvent> { bus.subscribe() }

    // MARK: Setup

    public static func open(identity: AppIdentity = .current) async throws -> LiveBackend {
        try await open(directory: identity.dataDirectory, identity: identity, defaultsDomain: identity.bundleID)
    }
    static func open(directory: URL, identity: AppIdentity, defaultsDomain: String? = nil) async throws -> LiveBackend {
        let result = try await Task.detached {
            let legacy = identity.bundleID == AppIdentity.productionID
                ? directory.deletingLastPathComponent().appendingPathComponent(AppIdentity.legacyID, isDirectory: true) : nil
            let migrated = try await LibraryMigration.open(directory: directory, defaultsDomain: defaultsDomain,
                role: ProcessInfo.processInfo.arguments.contains("--join-library") ? .join : .primary,
                legacyDirectory: legacy)
            let repository: LibraryRepository
            let settingsURL = directory.appendingPathComponent("settings.json")
            let defaults: Settings = identity.isDev ? .devDefault : .default
            let settings = FileManager.default.fileExists(atPath: settingsURL.path)
                ? try KudzuJSON.decoder(settingsDefaults: defaults).decode(Settings.self, from: Data(contentsOf: settingsURL)) : defaults
            if !identity.isDev && settings.icloudSync && Entitlements.hasICloudContainer(LibraryStoreConfiguration.destinationContainerIdentifier) {
                repository = try await LibraryRepository.open(.cloudKit(containerIdentifier: LibraryStoreConfiguration.destinationContainerIdentifier, url: directory.appendingPathComponent("Library.store")))
            } else { repository = migrated.repository }
            let store = try LocalStore(dir: directory, repository: repository,
                                      defaults: identity.isDev ? .devDefault : .default, firstRun: migrated.firstRun)
            try store.noteAppVersion(identity.version)
            return store
        }.value
        return LiveBackend(identity: identity, store: result)
    }
    private init(identity: AppIdentity, store: LocalStore) {
        self.identity = identity
        self.store = store
        let dir = store.dir
        let me = WeakBox<LiveBackend>()
        let bus = self.bus
        let host = LiveEngineHost(backend: me)
        engine = Engine(host: host)
        clipboard = ClipboardHistory(
            dir: dir.appendingPathComponent("clipboard", isDirectory: true),
            config: { [store] in
                let s = store.settings
                return ClipboardHistory.Config(enabled: s.clipboardEnabled, ignoredApps: s.clipboardIgnoredApps, retentionDays: s.clipboardRetentionDays)
            },
            emit: { bus.send(.clipboard($0)) }
        )
        updater = Updater(
            identity: identity,
            channel: { [store] in store.settings.updateChannel },
            autoCheck: { [store] in store.settings.autoCheckUpdates },
            publish: { state in
                bus.send(.update(state))
                Task { @MainActor in me.value?.shell?.updateChanged(state) }
            }
        )
        cloud = AutomaticCloudStatus(enabled: store.settings.icloudSync, available: store.settings.icloudSync && !identity.isDev && Entitlements.hasICloudContainer(LibraryStoreConfiguration.destinationContainerIdentifier),
                                     publish: { bus.send(.sync($0)) })
        agent = AgentServer(
            dir: dir,
            version: identity.version,
            handle: { line in
                try? await store.repository.handleAgent(message: line, version: identity.version, host: AppAgentHost(backend: me))
            },
            allowed: { [store] in store.settings.agentAccess }
        )
        me.value = self
    }

    /// Starts the engine and every background service (`lib.rs` `setup`, after the stores).
    func start() {
        observeLibrary()
        let settings = store.settings
        engine.setPaused(settings.paused)
        engine.reload(engineLibrary(library: store.library))
        engine.start()

        hotkeys.onPress = { [weak self] slot in
            switch slot {
            case .palette: self?.shell?.togglePalette()
            case .clipboard: self?.shell?.toggleClipboard()
            }
        }
        do {
            try hotkeys.register(.palette, accelerator: settings.paletteHotkey)
        } catch {
            log.warning("palette hotkey: \(error.message)")
        }
        applyClipboardEnabled()
        clipboard.start()
        agent.apply(on: settings.agentAccess)
        updater.start()
        cloud.recheck()

        // Daily backups (now, then every 6 h).
        let store = self.store
        backupTask?.cancel()
        backupTask = Task.detached(priority: .background) {
            while !Task.isCancelled {
                store.dailyBackup()
                try? await Task.sleep(for: .seconds(6 * 3600))
            }
        }
    }

    /// On quit: flush history and stores, stop the socket, install a waiting update.
    func shutDown() async {
        engine.stop()
        await usage.drain()
        backupTask?.cancel()
        await backupTask?.value
        backupTask = nil
        clipboard.stop()
        clipboard.flush()
        agent.shutDown()
        await agent.drain()
        updater.stop()
        cloud.stop()
        libraryObserver?.cancel()
        remoteObserver?.cancel()
        let store = self.store
        await Task.detached { store.flush() }.value
        // Quitting with a downloaded update installs it, so the next launch is current.
        updater.installOnQuit()
    }

    // MARK: Library & settings plumbing (state.rs)

    private var remoteObserver: Task<Void, Never>?
    private var libraryObserver: Task<Void, Never>?
    nonisolated let usage = AsyncWorkQueue()
    private func observeLibrary() {
        let changes = store.repository.changesIncludingCurrent
        remoteObserver = store.repository.observeRemoteChanges { [bus] message in bus.send(.sync(.error(message: message))) }
        libraryObserver = Task { [weak self] in
            for await library in changes {
                guard let self else { return }
                self.publish(library)
            }
        }
    }
    nonisolated func publish(_ library: Library) {
        commits.queue.async { [weak self] in
            guard let self else { return }
            self.commits.engineGeneration += 1
            self.bus.send(.library(library))
            self.engine.reload(self.engineLibrary(library: library), revision: self.commits.engineGeneration)
        }
    }
    nonisolated func editLibrary<T: Sendable>(_ body: @escaping @Sendable (inout Library) throws -> T) async throws -> T {
        try await store.repository.edit(body)
    }

    /// Persists settings, tells every window, and refreshes the engine on the same queue as
    /// library installs so an older library index cannot land after this one.
    public nonisolated func settingsChanged() {
        store.saveSettings()
        let settings = store.settings
        bus.send(.settings(settings))
        engine.setPaused(settings.paused)
        commits.queue.async { [weak self] in
            guard let self else { return }
            self.commits.engineGeneration += 1
            let generation = self.commits.engineGeneration
            let library = self.store.library
            self.engine.reload(self.engineLibrary(library: library), revision: generation)
        }
    }

    /// The engine's view of the library and settings. `library` overrides the in-memory copy
    /// so a publish installs the durable snapshot rather than edits that have not been saved.
    nonisolated func engineLibrary(library override: Library? = nil) -> EngineLibrary {
        let (library, settings) = (override ?? store.library, store.settings)
        let config = MatchingConfig(
            defaultInsertion: settings.defaultInsertion,
            deniedBundleIDs: Set(settings.deniedBundleIDs),
            appInsertions: settings.appInsertions,
            separators: MatchingConfig.defaultSeparators
        )
        return EngineLibrary(snippets: library.snippets, groups: library.groups, config: config, locale: System.locale())
    }

    func emit(_ event: BackendEvent) { bus.send(event) }

    var status: EngineStatus { engine.status }

    public func getState() async throws -> AppState {
        let (library, settings) = (store.library, store.settings)
        return AppState(library: library, settings: settings, status: status, platform: .macos)
    }
}

/// A weak reference that can be read from any thread.
final class WeakBox<T: AnyObject & Sendable>: @unchecked Sendable {
    // @unchecked: `stored` is only touched while holding `lock`.
    private let lock = NSLock()
    private weak var stored: T?
    var value: T? {
        get { lock.withLock { stored } }
        set { lock.withLock { stored = newValue } }
    }
}

/// Turns a thrown service error into the UI's error type.
func backendError(_ error: any Error) -> BackendError {
    if let e = error as? BackendError { return e }
    if let e = error as? ServiceError { return BackendError(e.message) }
    return BackendError(error.localizedDescription)
}
