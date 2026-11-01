import KudzuUI
import SwiftUI

struct IconButtonPage: GalleryPage {
    static let id = "icon-button"
    static let title = "IconButton"
    static let section = GallerySection.components
    static let icon = KzIconName.more

    @State private var on = true

    var body: some View {
        GalleryPageLayout(title: Self.title, summary: "Native controls with Kudzu metrics. States below: rest, hover, press, focus, disabled.") {
            SpecimenSection("Interactive") {
                GalleryCard {
                    KzIconButton(on ? .starFill : .star, label: "Favorite", active: on, tone: .bloom) { on.toggle() }
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
                                KzIconButton(.more, label: "More", menu: true, size: size) {}
                                KzIconButton(.more, label: "More", menu: true, size: size) {}.kzPreviewInteraction(hovered: true)
                                KzIconButton(.sidebar, label: "Sidebar", active: true, size: size) {}.disabled(true)
                            }
                        }
                    }
                }
            }
        }
    }
}
