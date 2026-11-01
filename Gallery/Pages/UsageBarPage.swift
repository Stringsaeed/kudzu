import KudzuUI
import SwiftUI

struct UsageBarPage: GalleryPage {
    static let id = "usage-bar"
    static let title = "UsageBar"
    static let section = GallerySection.structure
    static let icon = KzIconName.chart

    var body: some View {
        GalleryPageLayout(title: Self.title, summary: "A ranked usage bar: a trigger chip in a 96pt column, an 8pt vine-ink bar on a surface-sunken track, and the count. VoiceOver reads the value as a fraction (\"64 of 128\").") {
            SpecimenSection("Ranking") {
                KzSettingsGroup("Most used") {
                    VStack(spacing: 4) {
                        KzUsageBar(":m", value: 128, maximum: 128)
                        KzUsageBar(":an", value: 64, maximum: 128)
                        KzUsageBar(";sig", value: 12, maximum: 128)
                        KzUsageBar(":date", value: 1, maximum: 128)
                    }
                    .padding(.horizontal, KzSpace.s4)
                    .padding(.vertical, KzSpace.s3)
                }
            }
        }
    }
}
