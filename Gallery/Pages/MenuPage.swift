import KudzuUI
import SwiftUI

struct MenuPage: GalleryPage {
    static let id = "menu"
    static let title = "Menu"
    static let section = GallerySection.components
    static let icon = KzIconName.chevronsV
    static var snapshotWidth: CGFloat { 980 }

    @State private var sort = "Last used"
    @State private var format = "Plain text"
    @State private var showsMenu = false
    @State private var lastAction = "Nothing chosen yet."
    @State private var selectedRow: String? = "Address"
    @State private var contextRow: String?
    @Environment(\.kzSnapshotRendering) private var snapshotRendering

    private let sorts = ["Name", "Last used", "Most used", "Date created"]
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
                            KzMenuButton(title: "Sort by") {
                                for name in sorts {
                                    KzMenuItem(name, isChecked: sort == name) { sort = name; lastAction = "Sorted by \(name)" }
                                }
                            } label: { isOpen in
                                FloatDemoButtonLabel(icon: .sort, text: "Sort", isOpen: isOpen)
                            }
                            .buttonStyle(FloatDemoButtonStyle())

                            KzMenuButton(placement: .select) {
                                for name in formats {
                                    KzMenuItem(name, isChecked: format == name) { format = name; lastAction = "Format: \(name)" }
                                }
                            } label: { isOpen in
                                FloatDemoSelectLabel(value: format, isOpen: isOpen)
                            }
                            .buttonStyle(FloatDemoButtonStyle(well: true))

                            KzMenuButton {
                                insertTagItems
                            } label: { isOpen in
                                FloatDemoButtonLabel(icon: .braces, text: "Insert tag", isOpen: isOpen)
                            }
                            .buttonStyle(FloatDemoButtonStyle())

                            Button("Open with .kzMenu") { showsMenu = true }
                                .buttonStyle(FloatDemoButtonStyle())
                                .kzMenu(isPresented: $showsMenu, placement: .pullDown(.trailing)) {
                                    snippetItems
                                }
                        }
                        Text(lastAction).kzTextStyle(.callout).foregroundStyle(KzColor.inkMuted)
                    }
                }
            }

            SpecimenSection("Context menus", note: "Right-click or Control-click a row. Clicking still selects; the text field keeps its native text menu.") {
                GalleryCard(padding: KzSpace.s2) {
                    VStack(spacing: 2) {
                        ForEach(rows, id: \.self) { row in
                            Button { selectedRow = row } label: {
                                FloatDemoRow(title: row, isSelected: selectedRow == row, isContextTarget: contextRow == row)
                            }
                            .buttonStyle(.plain)
                            .kzContextMenu(isPresented: contextBinding(for: row)) {
                                    KzMenuItem("Open", icon: .compose) { lastAction = "Opened \(row)" }
                                    KzMenuItem("Duplicate", icon: .copy) { lastAction = "Duplicated \(row)" }.keyboardShortcut("d")
                                    KzMenuItem("Favorite", icon: .star, isChecked: false) { lastAction = "Favorited \(row)" }
                                    KzSubmenu("Move to group", icon: .folder) {
                                        KzMenuItem("Personal") { lastAction = "Moved \(row) to Personal" }
                                        KzMenuItem("Work") { lastAction = "Moved \(row) to Work" }
                                    }
                                    KzMenuSeparator()
                                    KzMenuItem("Delete", icon: .trash, role: .danger) { lastAction = "Deleted \(row)" }.keyboardShortcut(.delete)
                                }
                        }
                        if !snapshotRendering {
                            TextField("A text field keeps its own menu", text: .constant(""))
                                .textFieldStyle(.roundedBorder)
                                .padding(KzSpace.s2)
                        }
                    }
                }
            }
        }
    }

    private func contextBinding(for row: String) -> Binding<Bool> {
        Binding(get: { contextRow == row }, set: { contextRow = $0 ? row : (contextRow == row ? nil : contextRow) })
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

    @KzMenuBuilder private var insertTagItems: [KzMenuElement] {
        KzMenuHeader("Dates")
        KzMenuItem("Date", icon: .clock) { lastAction = "Inserted {date}" }
        KzMenuItem("Time", icon: .timer) { lastAction = "Inserted {time}" }
        KzMenuHeader("Text")
        KzMenuItem("Clipboard", icon: .copy) { lastAction = "Inserted {clipboard}" }
        KzMenuItem("Cursor position", icon: .cursor) { lastAction = "Inserted {cursor}" }
        KzSubmenu("Form field", icon: .type) {
            KzMenuItem("Text") { lastAction = "Inserted {form:text}" }
            KzMenuItem("Multiline") { lastAction = "Inserted {form:multiline}" }
            KzMenuItem("Choice") { lastAction = "Inserted {form:choice}" }
        }
    }
}

// MARK: - Stand-ins for the controls (Button and Select belong to the controls work)

/// A plain bordered button, close to `.kz-btn`, so the page doesn't depend on unbuilt controls.
struct FloatDemoButtonStyle: ButtonStyle {
    var well = false

    func makeBody(configuration: Configuration) -> some View {
        let shape = RoundedRectangle(cornerRadius: KzRadius.md)
        KzInteractive(shape, isPressed: configuration.isPressed) { _ in
            configuration.label
                .kzTextStyle(.bodyStrong)
                .foregroundStyle(KzColor.ink)
                .padding(.horizontal, well ? 0 : KzSpace.s3)
                .frame(height: well ? KzSize.field : KzSize.control)
                .background(well ? KzColor.surfaceSunken : KzColor.surfaceRaised, in: shape)
                .overlay(shape.strokeBorder(well ? .clear : KzColor.line))
        }
    }
}

struct FloatDemoButtonLabel: View {
    let icon: KzIconName
    let text: String
    let isOpen: Bool

    var body: some View {
        HStack(spacing: KzSpace.s1 + 2) {
            KzIcon(icon, size: 14).foregroundStyle(isOpen ? KzColor.vineInk : KzColor.inkMuted)
            Text(text)
            KzIcon(.chevronDown, size: 12).foregroundStyle(KzColor.inkMuted)
        }
    }
}

/// `.kz-select`: a well with the value and up-down chevrons; its label starts 10pt in.
struct FloatDemoSelectLabel: View {
    let value: String
    let isOpen: Bool

    var body: some View {
        HStack(spacing: KzSpace.s2) {
            Text(value).kzTextStyle(.bodyStrong).foregroundStyle(KzColor.ink)
            Spacer(minLength: 0)
            KzIcon(.chevronsV, size: 14).foregroundStyle(KzColor.inkMuted)
        }
        .padding(.leading, 10)
        .padding(.trailing, 6)
        .frame(width: 160)
    }
}

/// A list row with a selected fill and the outline native lists draw on a right-clicked row.
struct FloatDemoRow: View {
    let title: String
    let isSelected: Bool
    let isContextTarget: Bool

    var body: some View {
        let shape = RoundedRectangle(cornerRadius: KzRadius.md)
        HStack {
            Text(title).kzTextStyle(.bodyStrong).foregroundStyle(KzColor.ink)
            Spacer()
        }
        .padding(.horizontal, KzSpace.s3)
        .frame(height: 36)
        .background(isSelected ? KzColor.vineSoft : .clear, in: shape)
        .overlay(shape.strokeBorder(KzColor.vineInk, lineWidth: isContextTarget ? 2 : 0))
        .contentShape(shape)
        .accessibilityAddTraits(isSelected ? .isSelected : [])
    }
}
