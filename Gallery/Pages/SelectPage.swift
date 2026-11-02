import KudzuUI
import SwiftUI

struct SelectPage: GalleryPage {
    static let id = "select"
    static let title = "Select"
    static let section = GallerySection.controls
    static let icon = KzIconName.chevronsV

    @State private var app = "all"
    @State private var group = "base"
    @State private var mode = "Type"
    @State private var unmatched = "missing"

    private let apps = [("all", "All apps"), ("notes", "Notes"), ("mail", "Mail"), ("safari", "Safari"), ("xcode", "Xcode")]
    private let sample = KzMenu {
        KzMenuItem("All apps", isChecked: false) {}
        KzMenuItem("Notes", isChecked: true) {}
        KzMenuItem("Mail", isChecked: false) {}
        KzMenuItem("Safari", isChecked: false) {}
    }

    var body: some View {
        GalleryPageLayout(title: Self.title, summary: "A pop-up button: the current choice in a surface-sunken well with up-down chevrons, opening the glass menu with the checked item over the field, like NSPopUpButton. It is KzMenuButton(placement: .select) with KzSelectStyle.") {
            SpecimenSection("Interactive", note: "KzSelect(\"App\", selection:, options:). Press and drag to an item, or click and use ↑ ↓, type to select, Return to choose.") {
                GalleryCard {
                    HStack(spacing: KzSpace.s6) {
                        KzSelect("App", selection: $app, options: apps)
                        Text("Selected: \(app)").kzTextStyle(.callout).foregroundStyle(KzColor.inkMuted)
                    }
                }
            }
            SpecimenSection("Open", note: "The menu aligns the checked item's label with the field's label (10pt in). Menus are at least 208pt wide. Drawn here as a static specimen.") {
                GalleryCard {
                    // The menu is placed so the checked item's label (34pt into the menu) sits over the field's label
                    // (10pt into the field), as `KzMenuPlacement.select` does in a real window.
                    ZStack(alignment: .topLeading) {
                        KzSelectLabel("Notes", isOpen: true)
                            .frame(width: 160)
                            .background(KzColor.surfaceSunken, in: RoundedRectangle(cornerRadius: KzRadius.md))
                            .offset(x: 24, y: 31)
                        KzMenuSurface(sample)
                    }
                    .padding(.bottom, KzSpace.s8)
                }
            }
            SpecimenSection("Variants and states") {
                GalleryCard {
                    VStack(alignment: .leading, spacing: 16) {
                        KzSelect("Group", selection: $group, options: [("base", "base"), ("gs", "Getting started")])
                        KzSelect("Group", selection: $group, options: [("base", "base"), ("gs", "Getting started")]).kzPreviewInteraction(hovered: true)
                        KzSelect("Group", selection: $group, options: [("base", "base"), ("gs", "Getting started")]).kzPreviewInteraction(pressed: true)
                        KzSelect("Group", selection: $group, options: [("base", "base"), ("gs", "Getting started")]).kzPreviewInteraction(focused: true)
                        KzSelect("Group", selection: $group, options: [("base", "base"), ("gs", "Getting started")]).disabled(true)
                        KzSelect("Unset", selection: $unmatched, options: [("a", "A"), ("b", "B")])
                    }
                }
            }
            SpecimenSection("Lower level", note: "KzSelect(\"Mode\", value:) { … } takes any menu: icons, headers, submenus. Mark the current item with isChecked: true so a pop-up menu puts it over the field.") {
                GalleryCard {
                    KzSelect("Mode", value: mode) {
                        for name in ["Type", "Paste"] { KzMenuItem(name, isChecked: mode == name) { mode = name } }
                        KzMenuSeparator()
                        KzSubmenu("More") { KzMenuItem("Script") { mode = "Script" } }
                    }
                }
            }
        }
    }
}
