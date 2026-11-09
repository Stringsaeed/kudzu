import AppKit
import KudzuCore
import KudzuEngine
import KudzuServices
import SwiftUI

/// The windows, panels and status item (`windows.rs`, `tray.rs`, the window events of `lib.rs`).
/// One ``AppModel`` serves every window, given to each SwiftUI root through the environment.
@MainActor
public final class AppShell {
    let backend: LiveBackend
    public let model: AppModel

    private(set) var main: MainWindowController?
    private(set) var palette: FloatingPanel?
    private(set) var clipboardPanel: FloatingPanel?
    private(set) var formPanel: FloatingPanel?
    private(set) var permissionsPanel: FloatingPanel?
    private(set) var permissionGuide: PermissionGuide?
    private(set) var statusItem: StatusItemController?

    private var hidingPalette = false
    private var hidingClipboard = false
    private var paletteDismissal: Task<Void, Never>?

    init(backend: LiveBackend) {
        self.backend = backend
        model = AppModel(backend: backend, isDev: backend.identity.isDev)
        backend.shell = self
    }

    /// Builds the shell after the backend started (`lib.rs` `setup`, "Shell").
    func launch(showMainWindow: Bool) {
        let settings = backend.store.settings
        MainMenu.install(appName: backend.identity.productName, version: backend.identity.version, shell: self)
        statusItem = StatusItemController(isDev: backend.identity.isDev, settings: settings, actions: statusActions()) { [weak self] in
            let s = self?.backend.store.settings ?? settings
            return (s.paletteHotkey, s.clipboardHotkey)
        }
        statusItem?.update(backend.status)
        Task { await model.start() }
        prewarm()
        applyTheme(settings.theme)
        if showMainWindow { showMain() }
        if PermissionGuide.consumeResume(dataDir: backend.store.dir) {
            Task { try? await backend.showPermissions(kind: .accessibility) }
        }
    }

    // MARK: Prewarm

    /// Creates the panels up front so they open instantly.
    func prewarm() {
        if palette == nil {
            let p = FloatingPanel(size: PanelPlacement.paletteSize, radius: PanelPlacement.paletteRadius)
            p.host(PaletteRootView().environment(model))
            p.onResignKey = { [weak self] in self?.hidePalette(restore: false) }
            p.onCancel = { [weak self] in self?.hidePalette(restore: true) }
            palette = p
        }
        if clipboardPanel == nil {
            let p = FloatingPanel(size: PanelPlacement.clipboardSize, radius: PanelPlacement.paletteRadius)
            p.host(ClipboardPickerRootView().environment(model))
            p.onResignKey = { [weak self] in self?.hideClipboard(restore: false) }
            p.onCancel = { [weak self] in self?.hideClipboard(restore: true) }
            clipboardPanel = p
        }
        if formPanel == nil {
            let p = FloatingPanel(size: NSSize(width: PanelPlacement.formWidth, height: PanelPlacement.formHeight), radius: PanelPlacement.formRadius)
            p.host(FormRootView().environment(model))
            p.onCancel = { [weak self] in
                guard let self, let token = self.backend.pendingForm?.token else { return }
                Task { try? await self.backend.cancelForm(token: token) }
            }
            formPanel = p
        }
    }

    // MARK: Main window

    var mainVisible: Bool { main?.isVisible ?? false }

    /// Shows (and focuses) the main window, adding the Dock icon if the user wants it.
    func showMain() {
        NSApp.setActivationPolicy(backend.store.settings.showInDock ? .regular : .accessory)
        let controller = main ?? makeMain()
        main = controller
        controller.show()
    }

    private func makeMain() -> MainWindowController {
        let c = MainWindowController(root: RootView().environment(model), title: backend.identity.productName)
        c.onClose = { [weak self] in self?.hideMain() }
        return c
    }

    /// Hides the main window and drops back to a menu-bar-only app.
    func hideMain() {
        main?.hide()
        NSApp.setActivationPolicy(.accessory)
    }

    /// "Close Windows" (⌘Q): returns to the menu bar without destroying windows or
    /// interrupting background services.
    func hideAll() {
        if let pending = backend.pendingForm {
            backend.pendingForm = nil
            backend.engine.cancelForm(token: pending.token)
        }
        backend.engine.setSuppressed(false)
        hidePalette(restore: false)
        hideClipboard(restore: false)
        hideForm()
        hidePermissions()
        hideMain()
    }

    // MARK: Palette

    private func frontmostOther() -> AppInfo? {
        let me = ProcessInfo.processInfo.processIdentifier
        return System.frontmostApp().flatMap { $0.pid == me ? nil : $0 }
    }

    func togglePalette() {
        paletteDismissal?.cancel()
        paletteDismissal = nil
        guard let palette else { return prewarm() }
        if palette.isVisible { return hidePalette(restore: true) }
        // Remember where the user was, unless it's Kudzu itself.
        let front = frontmostOther()
        backend.paletteTarget = front
        backend.engine.setSuppressed(true)
        let origin: NSPoint
        if let m = PanelPlacement.activeMonitor() {
            let p = PanelPlacement.centered(width: PanelPlacement.paletteSize.width, on: m, topFraction: 0.2)
            origin = PanelPlacement.appKitOrigin(x: p.x, y: p.y, height: PanelPlacement.paletteSize.height)
        } else {
            origin = palette.frame.origin
        }
        backend.emit(.paletteShow(target: front))
        palette.present(at: origin)
    }

    /// Hides the palette; with `restore`, hands focus back to the app the user came from.
    func hidePalette(restore: Bool) {
        paletteDismissal?.cancel()
        paletteDismissal = nil
        guard let palette, palette.isVisible, !hidingPalette else { return }
        hidingPalette = true
        palette.orderOut(nil)
        hidingPalette = false
        if backend.pendingForm == nil { backend.engine.setSuppressed(false) }
        if restore, !(main?.isKey ?? false), let target = backend.paletteTarget {
            System.activateApp(target)
        }
    }

    /// A copy flash belongs to this presentation. Closing/reopening cancels its delayed hide.
    func hidePaletteAfterCopy() {
        paletteDismissal?.cancel()
        paletteDismissal = Task { [weak self] in
            do { try await Task.sleep(for: .milliseconds(260)) }
            catch { return }
            guard !Task.isCancelled else { return }
            self?.hidePalette(restore: true)
        }
    }

    // MARK: Clipboard picker

    /// Shows the clipboard history picker over the current app (or hides it if open).
    func toggleClipboard() {
        guard let panel = clipboardPanel else { return prewarm() }
        if panel.isVisible { return hideClipboard(restore: true) }
        guard backend.store.settings.clipboardEnabled else { return }
        hidePalette(restore: false)
        let front = frontmostOther()
        backend.clipTarget = front
        backend.engine.setSuppressed(true)
        let origin: NSPoint
        if let m = PanelPlacement.activeMonitor() {
            let p = PanelPlacement.centered(width: PanelPlacement.clipboardSize.width, on: m, topFraction: 0.18)
            origin = PanelPlacement.appKitOrigin(x: p.x, y: p.y, height: PanelPlacement.clipboardSize.height)
        } else {
            origin = panel.frame.origin
        }
        backend.emit(.clipboardShow(target: front, canPaste: EngineClipboard.canPaste()))
        panel.present(at: origin)
    }

    /// Hides the clipboard picker; with `restore`, hands focus back to the app the user came from.
    func hideClipboard(restore: Bool) {
        guard let panel = clipboardPanel, panel.isVisible, !hidingClipboard else { return }
        hidingClipboard = true
        panel.orderOut(nil)
        hidingClipboard = false
        if backend.pendingForm == nil { backend.engine.setSuppressed(false) }
        if restore, !(main?.isKey ?? false), let target = backend.clipTarget {
            System.activateApp(target)
        }
    }

    // MARK: Form

    func showForm(caret: ScreenRect?) {
        guard let panel = formPanel else { return prewarm() }
        let h = panel.frame.height
        let p = PanelPlacement.form(caret: caret, monitor: PanelPlacement.activeMonitor(), height: h)
        panel.present(at: PanelPlacement.appKitOrigin(x: p.x, y: p.y, height: h))
    }

    func hideForm() {
        formPanel?.orderOut(nil)
    }

    /// Resizes the form to its content (160…680 pt), keeping its top edge in place.
    func resizeForm(height: Double) {
        guard let panel = formPanel else { return }
        let h = min(max(CGFloat(height), PanelPlacement.formHeightRange.lowerBound), PanelPlacement.formHeightRange.upperBound)
        var frame = panel.frame
        frame.origin.y += frame.height - h
        frame.size = NSSize(width: PanelPlacement.formWidth, height: h)
        panel.setFrame(frame, display: true)
        // The shadow is cached per shape; a new height needs a new one.
        panel.refreshShadow()
    }

    // MARK: Permission companion

    func showPermissions(kind: PermissionKind) {
        let panel = permissionsPanel ?? makePermissions()
        permissionGuide?.stop()
        if !panel.isVisible { panel.center() }
        panel.makeKeyAndOrderFront(nil)
        panel.refreshShadow()
    }

    private func makePermissions() -> FloatingPanel {
        let p = FloatingPanel(size: PanelPlacement.permissionsSize, radius: PanelPlacement.paletteRadius, title: "Kudzu permissions")
        p.host(PermissionsRootView().environment(model))
        p.onCancel = { [weak self] in self?.hidePermissions() }
        p.center()
        permissionsPanel = p
        permissionGuide = PermissionGuide(window: p)
        return p
    }

    func hidePermissions() {
        permissionGuide?.stop()
        permissionsPanel?.orderOut(nil)
    }

    // MARK: Appearance, status, updates

    /// Points every window's native appearance at the user's theme, so translucent materials
    /// (sidebar, palette, form) match the app. `system` follows macOS.
    func applyTheme(_ theme: Theme) {
        NSApp.appearance = switch theme {
        case .dark: NSAppearance(named: .darkAqua)
        case .light: NSAppearance(named: .aqua)
        case .system: nil
        }
    }

    func statusChanged(_ status: EngineStatus) {
        statusItem?.update(status)
    }

    func updateChanged(_ state: UpdateState) {
        statusItem?.setUpdate(state)
    }

    // MARK: Quit and restart

    /// The explicit Quit: stops the engine and exits.
    func quit() {
        backend.engine.stop()
        NSApp.terminate(nil)
    }

    /// Relaunches the app (the permission companion's Restart).
    func restart() {
        if let bundle = Installer.bundleURL {
            try? Installer.relaunch(bundle)
        }
        quit()
    }

    private func statusActions() -> StatusItemController.Actions {
        StatusItemController.Actions(
            togglePause: { [weak self] in self?.backend.togglePaused() },
            search: { [weak self] in self?.togglePalette() },
            clipboard: { [weak self] in self?.toggleClipboard() },
            fromSelection: { [weak self] in self?.backend.newSnippetFromSelection() },
            open: { [weak self] in self?.showMain() },
            settings: { [weak self] in
                self?.showMain()
                self?.backend.emit(.navigate(.init(view: .settings())))
            },
            updates: { [weak self] in self?.updatesAction() },
            quit: { [weak self] in self?.quit() }
        )
    }

    private func updatesAction() {
        let state = backend.updater.state
        if case .ready = state {
            // Downloaded and verified: this item reads "Restart to Update to x.y.z".
            Task { try? await backend.restartToUpdate() }
            return
        }
        showMain()
        backend.emit(.navigate(.init(view: .settings(section: .about))))
        switch state {
        case .downloading, .checking: break
        default: Task { _ = await backend.updater.check(quiet: false) }
        }
    }
}
