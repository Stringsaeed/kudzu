import KudzuUI
import SwiftUI

struct ListHeaderPage: GalleryPage {
    static let id = "list-header"
    static let title = "ListHeader"
    static let section = GallerySection.structure
    static let icon = KzIconName.sort

    @State private var sort = "Name"

    var body: some View {
        GalleryPageLayout(title: Self.title, summary: "Title, optional subtitle and the list's own actions, size-titlebar (52pt) tall so it lines up with the traffic lights. Sort is a pull-down KzIconButton with a caret; try it.") {
            SpecimenSection("Variants") {
                VStack(spacing: KzSpace.s3) {
                    ForEach(0..<3, id: \.self) { index in
                        Group {
                            switch index {
                            case 0:
                                KzListHeader("All snippets", subtitle: "18 snippets") {
                                    KzIconButton(.sort, label: "Sort", caret: true, menuTitle: "Sort by") {
                                        GalleryMenus.sort(selection: sort) { sort = $0 }
                                    }
                                    KzIconButton(.compose, label: "New snippet (⌘N)") {}
                                }
                            case 1:
                                KzListHeader("Settings", subtitle: "Kudzu 1.0")
                            default:
                                KzListHeader("A group with a very long name that must truncate", subtitle: "3 snippets") {
                                    KzIconButton(.compose, label: "New snippet (⌘N)") {}
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
