@testable import KudzuUI
import SwiftUI
import Testing

@MainActor
@Suite struct SelectSnapshotTests {
    private static let options = [("all", "All apps"), ("notes", "Notes"), ("mail", "Mail")]

    @Test func states() throws {
        try snapshot("select-states", width: 400) {
            VStack(alignment: .leading, spacing: 16) {
                KzSelect("App", selection: .constant("notes"), options: Self.options)
                KzSelect("App", selection: .constant("notes"), options: Self.options).kzPreviewInteraction(hovered: true)
                KzSelect("App", selection: .constant("notes"), options: Self.options).kzPreviewInteraction(pressed: true)
                KzSelect("App", selection: .constant("notes"), options: Self.options).kzPreviewInteraction(focused: true)
                KzSelect("App", selection: .constant("notes"), options: Self.options).disabled(true)
                KzSelect("App", selection: .constant("missing"), options: Self.options)
            }.frame(width: 220)
        }
    }

    /// The open select as it's drawn in a real window: the menu over the field, checked label over label.
    @Test func openSpecimen() throws {
        try snapshot("select-open", width: 320) {
            ZStack(alignment: .topLeading) {
                KzSelectLabel("Notes", isOpen: true)
                    .frame(width: 160)
                    .background(KzColor.surfaceSunken, in: RoundedRectangle(cornerRadius: KzRadius.md))
                    .offset(x: 24, y: 31)
                KzMenuSurface(KzMenu {
                    KzMenuItem("All apps", isChecked: false) {}
                    KzMenuItem("Notes", isChecked: true) {}
                    KzMenuItem("Mail", isChecked: false) {}
                })
            }
        }
    }
}

@MainActor
@Suite struct SelectBehaviorTests {
    @Test func menuChecksTheSelectedOptionAndChoosingWritesTheBinding() {
        var value = "notes"
        let select = KzSelect("App", selection: Binding(get: { value }, set: { value = $0 }),
                              options: [("all", "All apps"), ("notes", "Notes"), ("mail", "Mail")])
        #expect(select.displayedValue == "Notes")
        let items = select.menuElements.compactMap(\.item)
        #expect(items.map(\.label) == ["All apps", "Notes", "Mail"])
        #expect(items.map(\.isChecked) == [false, true, false])
        items[2].action()
        #expect(value == "mail")
    }

    @Test func showsTheLabelWhenNothingMatches() {
        let select = KzSelect("App", selection: .constant("x"), options: [("a", "A")])
        #expect(select.displayedValue == "App")
        #expect(select.menuElements.compactMap(\.item).allSatisfy { $0.isChecked == false })
    }

    @Test func lowerLevelFormTakesAnyMenu() {
        let select = KzSelect("Mode", value: "Type") {
            KzMenuItem("Type", isChecked: true) {}
            KzMenuSeparator()
            KzSubmenu("More") { KzMenuItem("Script") {} }
        }
        #expect(select.displayedValue == "Type")
        #expect(select.menuElements.count == 3)
    }
}
