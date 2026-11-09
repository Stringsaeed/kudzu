import AppKit
import KudzuCore
import KudzuServices

/// The application delegate (`lib.rs` `run`): an accessory (menu-bar) app, one instance per
/// bundle id, the main window on launch unless started hidden by the login item, the main
/// window again on reopen. ⌘Q closes windows through MainMenu; native Quit requests exit.
@MainActor
public final class AppDelegate: NSObject, NSApplicationDelegate {
    /// Started with `--hidden` (the Tauri login agent) or by the login item.
    public private(set) var launchedHidden: Bool
    public let identity: AppIdentity

    private var backend: LiveBackend?
    private var shell: AppShell?
    private var instance: SingleInstance?

    public init(launchedHidden: Bool, identity: AppIdentity = .current) {
        self.launchedHidden = launchedHidden
        self.identity = identity
        super.init()
    }

    public func applicationWillFinishLaunching(_ notification: Notification) {
        Logging.configure(identity: identity)
        // A second launch shows the running copy's main window and exits.
        let instance = SingleInstance(bundleID: identity.bundleID)
        if instance.handOffToRunningCopy() {
            exit(0)
        }
        self.instance = instance
        if LoginItem.launchedAsLoginItem() { launchedHidden = true }
    }

    public func applicationDidFinishLaunching(_ notification: Notification) {
        NSApp.setActivationPolicy(.accessory)
        bootstrap()
    }
    private func bootstrap() {
        Task {
            do {
                let backend = try await LiveBackend.open(identity: identity)
                let shell = AppShell(backend: backend)
                self.backend = backend; self.shell = shell
                if !identity.isDev, backend.store.settings.icloudSync,
                   Entitlements.hasICloudContainer(LibraryStoreConfiguration.destinationContainerIdentifier) {
                    NSApp.registerForRemoteNotifications()
                }
                backend.start()
                shell.launch(showMainWindow: !launchedHidden || backend.store.firstRun)
                instance?.onShowMain = { [weak shell] in shell?.showMain() }
            } catch {
                let alert = NSAlert()
                alert.messageText = "Kudzu could not open the library"
                alert.informativeText = error.localizedDescription + " Original data has been retained."
                alert.addButton(withTitle: "Retry"); alert.addButton(withTitle: "Quit")
                if alert.runModal() == .alertFirstButtonReturn { bootstrap() }
                else { NSApp.terminate(nil) }
            }
        }
    }

    public func applicationShouldHandleReopen(_ sender: NSApplication, hasVisibleWindows flag: Bool) -> Bool {
        shell?.showMain()
        return true
    }

    /// Dock Quit, Apple-event Quit and logout/shutdown are real termination requests, as in
    /// Tauri/tao. ⌘Q is already wired to Close Windows, so it never reaches this delegate.
    public func applicationShouldTerminate(_ sender: NSApplication) -> NSApplication.TerminateReply {
        guard let backend else { return .terminateNow }
        Task { await backend.shutDown(); self.backend = nil; sender.reply(toApplicationShouldTerminate: true) }
        return .terminateLater
    }

    public func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool { false }

    public func applicationWillTerminate(_ notification: Notification) {
        // Drained in applicationShouldTerminate before termination is accepted.
    }

    public func applicationSupportsSecureRestorableState(_ app: NSApplication) -> Bool { true }
}
