import AppKit

/// The menu bar while a Kudzu window is active (`tray.rs` `create_app_menu`): the app menu with
/// About, Services, Hide, Hide Others, **Close Windows** on ⌘Q (Kudzu keeps running in the menu
/// bar) and **Quit Kudzu** without a shortcut; then the standard Edit and Window menus.
@MainActor
enum MainMenu {
    static func install(appName: String, version: String, shell: AppShell) {
        let target = MenuTarget.shared
        target.shell = shell
        let main = NSMenu()

        let appItem = NSMenuItem()
        let app = NSMenu(title: appName)
        app.addItem(withTitle: "About \(appName)", action: #selector(NSApplication.orderFrontStandardAboutPanel(_:)), keyEquivalent: "")
        app.addItem(.separator())
        let services = NSMenuItem(title: "Services", action: nil, keyEquivalent: "")
        let servicesMenu = NSMenu(title: "Services")
        services.submenu = servicesMenu
        NSApp.servicesMenu = servicesMenu
        app.addItem(services)
        app.addItem(.separator())
        app.addItem(withTitle: "Hide \(appName)", action: #selector(NSApplication.hide(_:)), keyEquivalent: "h")
        let others = app.addItem(withTitle: "Hide Others", action: #selector(NSApplication.hideOtherApplications(_:)), keyEquivalent: "h")
        others.keyEquivalentModifierMask = [.command, .option]
        app.addItem(.separator())
        let close = app.addItem(withTitle: "Close Windows", action: #selector(MenuTarget.closeWindows), keyEquivalent: "q")
        close.target = target
        let quit = app.addItem(withTitle: "Quit \(appName)", action: #selector(MenuTarget.quit), keyEquivalent: "")
        quit.target = target
        appItem.submenu = app
        main.addItem(appItem)

        let editItem = NSMenuItem()
        let edit = NSMenu(title: "Edit")
        edit.addItem(withTitle: "Undo", action: Selector(("undo:")), keyEquivalent: "z")
        let redo = edit.addItem(withTitle: "Redo", action: Selector(("redo:")), keyEquivalent: "z")
        redo.keyEquivalentModifierMask = [.command, .shift]
        edit.addItem(.separator())
        edit.addItem(withTitle: "Cut", action: #selector(NSText.cut(_:)), keyEquivalent: "x")
        edit.addItem(withTitle: "Copy", action: #selector(NSText.copy(_:)), keyEquivalent: "c")
        edit.addItem(withTitle: "Paste", action: #selector(NSText.paste(_:)), keyEquivalent: "v")
        edit.addItem(withTitle: "Select All", action: #selector(NSText.selectAll(_:)), keyEquivalent: "a")
        editItem.submenu = edit
        main.addItem(editItem)

        let windowItem = NSMenuItem()
        let window = NSMenu(title: "Window")
        window.addItem(withTitle: "Minimize", action: #selector(NSWindow.performMiniaturize(_:)), keyEquivalent: "m")
        window.addItem(withTitle: "Zoom", action: #selector(NSWindow.performZoom(_:)), keyEquivalent: "")
        window.addItem(.separator())
        window.addItem(withTitle: "Close Window", action: #selector(NSWindow.performClose(_:)), keyEquivalent: "w")
        windowItem.submenu = window
        NSApp.windowsMenu = window
        main.addItem(windowItem)

        NSApp.mainMenu = main
    }

    /// The target of Kudzu's own app-menu items.
    @MainActor
    final class MenuTarget: NSObject {
        static let shared = MenuTarget()
        weak var shell: AppShell?

        @objc func closeWindows() { shell?.hideAll() }
        @objc func quit() { shell?.quit() }
    }
}
