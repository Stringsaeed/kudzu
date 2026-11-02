import KudzuUI
import SwiftUI

struct IconButtonPage: GalleryPage {
    static let id = "icon-button"
    static let title = "IconButton"
    static let section = GallerySection.controls
    static let icon = KzIconName.more

    @State private var on = true
    @State private var sort = "Last used"
    @State private var lastAction = "Open a menu."

    var body: some View {
        GalleryPageLayout(title: Self.title, summary: "A square, borderless button holding one icon. A button that runs an action, or one that opens a glass menu (KzMenuButton underneath): it opens on mouse-down and keeps the active look while the menu is open. Inside a ToolbarGroup it is 28pt tall.") {
            SpecimenSection("Interactive") {
                GalleryCard {
                    HStack(spacing: KzSpace.s6) {
                        KzIconButton(on ? .starFill : .star, label: "Favorite", active: on, tone: .bloom) { on.toggle() }
                        KzIconButton(.sort, label: "Sort", caret: true, menuTitle: "Sort by") {
                            GalleryMenus.sort(selection: sort) { sort = $0; lastAction = "Sorted by \($0)" }
                        }
                        KzIconButton(.more, label: "More") {
                            GalleryMenus.snippetActions { lastAction = $0 }
                        }
                        Text(lastAction).kzTextStyle(.callout).foregroundStyle(KzColor.inkMuted)
                    }
                }
            }
            SpecimenSection("Variants and states") {
                GalleryCard {
                    VStack(alignment: .leading, spacing: 16) {
                        ForEach(KzButton.Size.allCases, id: \.self) { size in
                            HStack(spacing: 16) {
                                KzIconButton(.compose, label: "New snippet", size: size) {}
                                KzIconButton(.compose, label: "New snippet", size: size) {}.kzPreviewInteraction(hovered: true)
                                KzIconButton(.compose, label: "New snippet", size: size) {}.kzPreviewInteraction(pressed: true)
                                KzIconButton(.compose, label: "New snippet", size: size) {}.kzPreviewInteraction(focused: true)
                                KzIconButton(.compose, label: "New snippet", size: size) {}.disabled(true)
                            }
                            HStack(spacing: 16) {
                                KzIconButton(.sidebar, label: "Sidebar", active: true, size: size) {}
                                KzIconButton(.starFill, label: "Favorite", size: size, tone: .bloom) {}
                                KzIconButton(.sort, label: "Sort", caret: true, size: size) {}
                                KzIconButton(.sort, label: "Sort", caret: true, size: size) {}.kzPreviewInteraction(hovered: true)
                                KzIconButton(.sort, label: "Sort", caret: true, active: true, size: size) {}
                                KzIconButton(.sidebar, label: "Sidebar", active: true, size: size) {}.disabled(true)
                            }
                        }
                    }
                }
            }
            SpecimenSection("In a ToolbarGroup", note: "Buttons are 28pt tall (.kz-toolbar .kz-iconbtn), 30pt wide. The More button shows the active look while its menu is open.") {
                GalleryCard {
                    HStack(spacing: KzSpace.s6) {
                        KzToolbarGroup {
                            KzIconButton(.starFill, label: "Favorite", tone: .bloom) {}
                            KzIconButton(.bug, label: "Debug", caret: true) {}
                            KzToolbarSeparator()
                            KzIconButton(.more, label: "More", active: true) {}
                        }
                        KzIconButton(.more, label: "More") {}
                    }
                }
            }
        }
    }
}
