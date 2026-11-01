import KudzuUI
import SwiftUI

struct TriggerChipPage: GalleryPage {
    static let id = "trigger-chip"
    static let title = "TriggerChip"
    static let section = GallerySection.controls
    static let icon = KzIconName.braces

    var body: some View {
        GalleryPageLayout(title: Self.title, summary: "Kudzu's signature element. Triggers appear exactly as typed; the prefix symbol is set in vine-ink.") {
            SpecimenSection("Triggers") {
                GalleryCard {
                    HStack(spacing: KzSpace.s2) {
                        KzTriggerChip(":an"); KzTriggerChip(";sig"); KzTriggerChip("//date"); KzTriggerChip("addr")
                    }
                }
            }
            SpecimenSection("Removable", note: "Only inside TriggerInput.") {
                GalleryCard {
                    HStack(spacing: KzSpace.s2) {
                        KzTriggerChip(":an") {}; KzTriggerChip(";signature") {}
                    }
                }
            }
        }
    }
}
