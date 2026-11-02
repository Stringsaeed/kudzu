import KudzuUI
import SwiftUI

/// Menus the gallery pages share, so the specimens and the live buttons show the same items.
enum GalleryMenus {
    static let sortOrders = ["Name", "Last used", "Most used", "Date created"]

    /// The editor's Insert menu: tags in groups, with the form fields in a submenu.
    @KzMenuBuilder
    static func insertTag(_ insert: @escaping @MainActor (String) -> Void) -> [KzMenuElement] {
        KzMenuHeader("Dates")
        KzMenuItem("Date", icon: .clock) { insert("{date}") }
        KzMenuItem("Time", icon: .timer) { insert("{time}") }
        KzMenuHeader("Text")
        KzMenuItem("Clipboard", icon: .copy) { insert("{clipboard}") }
        KzMenuItem("Cursor position", icon: .cursor) { insert("{cursor}") }
        KzSubmenu("Form field", icon: .type) {
            KzMenuItem("Text") { insert("{form:text}") }
            KzMenuItem("Multiline") { insert("{form:multiline}") }
            KzMenuItem("Choice") { insert("{form:choice}") }
        }
    }

    /// A pull-down of checkable sort orders.
    @KzMenuBuilder
    static func sort(selection: String, choose: @escaping @MainActor (String) -> Void) -> [KzMenuElement] {
        for name in sortOrders {
            KzMenuItem(name, isChecked: selection == name) { choose(name) }
        }
    }

    /// The "more" menu for a snippet.
    @KzMenuBuilder
    static func snippetActions(_ run: @escaping @MainActor (String) -> Void) -> [KzMenuElement] {
        KzMenuItem("Duplicate", icon: .copy) { run("Duplicate") }.keyboardShortcut("d")
        KzMenuItem("Copy expanded text", icon: .copy) { run("Copy expanded text") }
        KzMenuSeparator()
        KzMenuItem("Delete", icon: .trash, role: .danger) { run("Delete") }.keyboardShortcut(.delete)
    }

    /// The Debug menu in the detail toolbar.
    @KzMenuBuilder
    static func debug(_ run: @escaping @MainActor (String) -> Void) -> [KzMenuElement] {
        KzMenuItem("Test expansion", icon: .play) { run("Test expansion") }
        KzMenuItem("Copy expanded text", icon: .copy) { run("Copy expanded text") }
        KzMenuItem("View expansion log", icon: .history) { run("View expansion log") }
    }

    /// The right-click menu for a snippet or list row.
    @KzMenuBuilder
    static func row(groups: [String] = ["base", "Getting started"], isFavorite: Bool = true, run: @escaping @MainActor (String) -> Void,
                    moveTo: @escaping @MainActor (String) -> Void) -> [KzMenuElement] {
        KzMenuItem("Duplicate", icon: .copy) { run("Duplicate") }.keyboardShortcut("d")
        KzMenuItem("Favorite", icon: .star, isChecked: isFavorite) { run("Favorite") }
        KzSubmenu("Move to group", icon: .folder) {
            for group in groups { KzMenuItem(group) { moveTo(group) } }
        }
        KzMenuSeparator()
        KzMenuItem("Delete", icon: .trash, role: .danger) { run("Delete") }.keyboardShortcut(.delete)
    }
}

/// A right-click menu on a gallery row that outlines the row while it's open and reports the chosen
/// action in `log`: Duplicate, Favorite (checked), Move to group, Delete.
private struct GalleryRowMenu: ViewModifier {
    let id: String
    @Binding var target: String?
    @Binding var log: String

    func body(content: Content) -> some View {
        content
            .kzContextMenu(isPresented: Binding(
                get: { target == id },
                set: { open in target = open ? id : (target == id ? nil : target) }
            )) {
                GalleryMenus.row(run: { log = "\($0) “\(id)”" }, moveTo: { log = "Moved “\(id)” to \($0)" })
            }
            .kzContextTarget(target == id)
    }
}

extension View {
    /// Adds the shared row context menu, keyed by `id`; `target` tracks which row is right-clicked.
    func galleryRowMenu(_ id: String, target: Binding<String?>, log: Binding<String>) -> some View {
        modifier(GalleryRowMenu(id: id, target: target, log: log))
    }
}
