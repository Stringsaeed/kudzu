import AppKit
import Foundation
import KudzuCore
import KudzuEngine
import KudzuServices

// Settings, engine and permission commands (`commands.rs` "Settings & engine",
// `permissions/mod.rs`, `set_*_hotkey`).
extension LiveBackend {
    public func updateSettings(_ patch: SettingsPatch) async throws -> Settings {
        let (old, new) = store.withLock { c -> (Settings, Settings) in
            let old = c.settings
            c.settings = patch.applied(to: old)
            return (old, c.settings)
        }
        if old.launchAtLogin != new.launchAtLogin {
            do {
                try LoginItem.set(enabled: new.launchAtLogin)
            } catch {
                log.warning("autostart: \(error.message)")
            }
        }
        if old.showInDock != new.showInDock, shell?.mainVisible == true {
            NSApp.setActivationPolicy(new.showInDock ? .regular : .accessory)
        }
        if old.icloudSync != new.icloudSync {
            cloud.configurationChanged()
        }
        if old.theme != new.theme { shell?.applyTheme(new.theme) }
        settingsChanged()
        if old.clipboardEnabled != new.clipboardEnabled {
            applyClipboardEnabled()
            shell?.statusItem?.setClipboardEnabled(new.clipboardEnabled)
        }
        if old.agentAccess != new.agentAccess { agent.apply(on: new.agentAccess) }
        if old.clipboardRetentionDays != new.clipboardRetentionDays { clipboard.pruneSoon() }
        return new
    }

    public func setPaused(_ paused: Bool) async throws -> EngineStatus {
        store.withLock { $0.settings.paused = paused }
        settingsChanged()
        let status = self.status
        emit(.status(status))
        shell?.statusChanged(status)
        return status
    }

    /// The status item's pause toggle.
    func togglePaused() {
        let paused = store.withLock { c -> Bool in
            c.settings.paused.toggle()
            return c.settings.paused
        }
        settingsChanged()
        engine.setPaused(paused)
        shell?.statusChanged(status)
    }

    public func engineStatus() async throws -> EngineStatus { status }

    public func requestPermission(kind: PermissionKind) async throws {
        switch kind {
        case .accessibility: System.requestAccessibility()
        case .inputMonitoring: System.requestInputMonitoring()
        }
    }

    public func openSystemSettings(pane: SystemSettingsPane) async throws {
        System.openSettings(pane)
    }

    // MARK: Hotkeys

    public func setPaletteHotkey(accelerator: String) async throws -> Settings {
        do {
            try hotkeys.register(.palette, accelerator: accelerator)
        } catch {
            throw BackendError(error.message)
        }
        let settings = store.withLock { c -> Settings in
            c.settings.paletteHotkey = accelerator
            return c.settings
        }
        settingsChanged()
        shell?.statusItem?.refreshHints()
        return settings
    }

    public func setClipboardHotkey(accelerator: String) async throws -> Settings {
        do {
            try hotkeys.register(.clipboard, accelerator: accelerator)
        } catch {
            throw BackendError(error.message)
        }
        let settings = store.withLock { c -> Settings in
            c.settings.clipboardHotkey = accelerator
            return c.settings
        }
        settingsChanged()
        shell?.statusItem?.refreshHints()
        return settings
    }

    /// Registers the picker shortcut when history is on (and removes it when off).
    func applyClipboardEnabled() {
        let s = store.settings
        if s.clipboardEnabled {
            do {
                try hotkeys.register(.clipboard, accelerator: s.clipboardHotkey)
            } catch {
                log.warning("clipboard hotkey: \(error.message)")
            }
        } else {
            hotkeys.unregister(.clipboard)
            shell?.hideClipboard(restore: false)
        }
    }

    // MARK: Permission companion

    public func showPermissions(kind: PermissionKind) async throws {
        permissionKind = kind
        shell?.showPermissions(kind: kind)
        emit(.permissionShow(kind))
    }

    public func permissionContext() async throws -> PermissionContext {
        PermissionContext(kind: permissionKind, appName: identity.productName, accessibilityName: PermissionGuide.accessibilityName)
    }

    public func installPermissionIcon(bounds: ScreenRect) async throws -> Bool {
        guard let guide = shell?.permissionGuide else { return false }
        do {
            return try guide.installDragIcon(bounds: bounds)
        } catch {
            throw BackendError(error.message)
        }
    }

    public func beginPermission(kind: PermissionKind) async throws {
        switch kind {
        case .accessibility: System.requestAccessibility()
        case .inputMonitoring: System.requestInputMonitoring()
        }
        guard let guide = shell?.permissionGuide, guide.openSettings(accessibility: kind == .accessibility) else {
            throw BackendError("Could not open System Settings. Open Privacy & Security from the Apple menu.")
        }
    }

    public func stopPermissionGuide() async throws {
        shell?.permissionGuide?.stop()
    }

    public func closePermissions() async throws {
        shell?.hidePermissions()
        shell?.showMain()
    }

    public func restartPermissionApp() async throws {
        shell?.permissionGuide?.stop()
        do {
            try PermissionGuide.markResume(dataDir: store.dir)
        } catch {
            throw backendError(error)
        }
        shell?.restart()
    }

    public func revealPermissionApp() async throws {
        shell?.permissionGuide?.stop()
        guard PermissionGuide.revealApp() else {
            throw BackendError("Run the installed Kudzu application to reveal it in Finder.")
        }
    }
}
