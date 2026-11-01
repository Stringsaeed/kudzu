import KudzuUI
import SwiftUI

struct KbdPage: GalleryPage {
    static let id = "kbd"
    static let title = "Kbd"
    static let section = GallerySection.controls
    static let icon = KzIconName.keyboard

    var body: some View {
        GalleryPageLayout(title: Self.title, summary: "A keycap for shortcuts in hints, menus and empty states. Mac glyphs, never Cmd.") {
            SpecimenSection("Keys") {
                GalleryCard {
                    HStack(spacing: KzSpace.s2) {
                        KzKbd("⌘N"); KzKbd("⌘F"); KzKbd("⌘,"); KzKbd("/"); KzKbd("⌃⌥Space"); KzKbd("↩")
                    }
                }
            }
            SpecimenSection("In a hint") {
                GalleryCard {
                    HStack(spacing: KzSpace.s1) {
                        Text("Type").kzTextStyle(.callout).foregroundStyle(KzColor.inkMuted)
                        KzKbd("/")
                        Text("to insert a tag").kzTextStyle(.callout).foregroundStyle(KzColor.inkMuted)
                    }
                }
            }
        }
    }
}
