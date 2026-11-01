import KudzuUI
import SwiftUI

struct ListHeaderPage: GalleryPage {
    static let id = "list-header"
    static let title = "ListHeader"
    static let section = GallerySection.components
    static let icon = KzIconName.sort

    var body: some View {
        GalleryPageLayout(title: Self.title, summary: "Title, optional subtitle and the list's own actions, size-titlebar (52pt) tall so it lines up with the traffic lights. Actions are placeholders.") {
            SpecimenSection("Variants") {
                VStack(spacing: KzSpace.s3) {
                    ForEach(0..<3, id: \.self) { index in
                        Group {
                            switch index {
                            case 0:
                                KzListHeader("All snippets", subtitle: "18 snippets") {
                                    PlaceholderIconButton(icon: .sort)
                                    PlaceholderIconButton(icon: .compose)
                                }
                            case 1:
                                KzListHeader("Settings", subtitle: "Kudzu 1.0")
                            default:
                                KzListHeader("A group with a very long name that must truncate", subtitle: "3 snippets") {
                                    PlaceholderIconButton(icon: .compose)
                                }
                            }
                        }
                        .frame(width: KzSize.list)
                        .background(KzColor.surface)
                        .overlay(Rectangle().strokeBorder(KzColor.line))
                    }
                }
            }
        }
    }
}
