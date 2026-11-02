import KudzuUI
import SwiftUI

struct MenuPage: GalleryPage {
    static let id = "menu"
    static let title = "Menu"
    static let section = GallerySection.floating
    static let icon = KzIconName.chevronsV
    static var snapshotWidth: CGFloat { 980 }

    @State private var sort = "Last used"
    @State private var draft = ""
    @State private var format = "Plain text"
    @State private var showsMenu = false
    @State private var lastAction = "Nothing chosen yet."
    @State private var selectedRow: String? = "Address"
    @State private var contextRow: String?

    private let sorts = GalleryMenus.sortOrders
    private let formats = ["Plain text", "Rich text", "Markdown", "Script"]
    private let rows = ["Address", "Signature", "Meeting link", "Shrug"]

    var body: some View {
        GalleryPageLayout(title: Self.title, summary: "A floating glass menu for sort, insert tag, select options and right-click context menus. It opens in its own panel, so it can leave the window, and behaves like a Mac menu: arrows, type-ahead, press-drag-release, Esc.") {
            SpecimenSection("Specimens", note: "glass-strong, shadow-float, radius-lg. Items are 28pt with radius-sm; the highlight (hover or keyboard) is vine-soft; the check is vine-ink.") {
                HStack(alignment: .top, spacing: KzSpace.s6) {
                    KzMenuSurface(sortMenu)
                    KzMenuSurface(snippetMenu, highlightedID: "Favorite")
                    KzMenuSurface(insertTagMenu, highlightedID: "Clipboard")
                }
                .padding(.bottom, KzSpace.s8)
            }

            SpecimenSection("Try it", note: "Press on a button and drag to an item, or click and use ↑ ↓, Home/End, letters to type-select, Return or Space to choose, → ← for submenus and Esc to close.") {
                GalleryCard {
                    VStack(alignment: .leading, spacing: KzSpace.s4) {
                        HStack(spacing: KzSpace.s3) {
                            KzButton("Sort", icon: .sort, iconEnd: .chevronDown, menuTitle: "Sort by") {
                                GalleryMenus.sort(selection: sort) { sort = $0; lastAction = "Sorted by \($0)" }
                            }

                            KzSelect("Format", selection: $format, options: formats.map { ($0, $0) })
                                .onChange(of: format) { _, new in lastAction = "Format: \(new)" }

                            KzButton("Insert tag", icon: .braces, iconEnd: .chevronDown, menuTitle: "Insert tag") {
                                insertTagItems
                            }

                            KzIconButton(.sort, label: "Sort", caret: true, menuTitle: "Sort by") {
                                GalleryMenus.sort(selection: sort) { sort = $0; lastAction = "Sorted by \($0)" }
                            }

                            KzIconButton(.more, label: "More", placement: .pullDown(.trailing)) {
                                snippetItems
                            }

                            KzButton("Open with .kzMenu") { showsMenu = true }
                                .kzMenu(isPresented: $showsMenu, placement: .pullDown(.trailing)) {
                                    snippetItems
                                }
                        }
                        Text(lastAction).kzTextStyle(.callout).foregroundStyle(KzColor.inkMuted)
                    }
                }
            }

            SpecimenSection("Context menus", note: "Right-click or Control-click a row. Clicking still selects; the text field keeps its native text menu. The right-clicked row is outlined while its menu is open.") {
                GalleryCard(padding: KzSpace.s2) {
                    VStack(spacing: 2) {
                        ForEach(rows, id: \.self) { row in
                            Button { selectedRow = row } label: {
                                KzListRow(row, meta: "\(row.lowercased()).json", aside: "1", isSelected: selectedRow == row)
                            }
                            .buttonStyle(.plain)
                            .galleryRowMenu(row, target: $contextRow, log: $lastAction)
                        }
                        KzTextInput("A text field keeps its own menu", text: $draft)
                            .padding(KzSpace.s2)
                    }
                }
            }
        }
    }

    // MARK: Menus

    private var sortMenu: KzMenu {
        KzMenu(title: "Sort by") {
            for name in sorts { KzMenuItem(name, isChecked: name == "Last used") {} }
        }
    }

    private var snippetMenu: KzMenu { KzMenu { snippetItems } }

    private var insertTagMenu: KzMenu { KzMenu(title: "Insert tag") { insertTagItems } }

    @KzMenuBuilder private var snippetItems: [KzMenuElement] {
        KzMenuItem("Duplicate", icon: .copy) { lastAction = "Duplicate" }.keyboardShortcut("d")
        KzMenuItem("Favorite", icon: .star) { lastAction = "Favorite" }.keyboardShortcut("f", modifiers: [.command, .shift])
        KzMenuItem("Debug expansion", icon: .bug) {}.disabled()
        KzMenuSeparator()
        KzMenuItem("Delete", icon: .trash, role: .danger) { lastAction = "Delete" }.keyboardShortcut(.delete)
    }

    private var insertTagItems: [KzMenuElement] {
        GalleryMenus.insertTag { lastAction = "Inserted \($0)" }
    }
}
