import KudzuUI
import SwiftUI

struct ListRowPage: GalleryPage {
    static let id = "list-row"
    static let title = "ListRow"
    static let section = GallerySection.components
    static let icon = KzIconName.list

    var body: some View {
        GalleryPageLayout(title: Self.title, summary: "A generic two-line row for backups and the stats ranking: a text or chip title, mono-caption meta, an aside count and a badge. Selection and hover match SnippetRow.") {
            SpecimenSection("States") {
                VStack(spacing: 1) {
                    KzListRow(meta: "Last used 4 hours ago", aside: "1", isSelected: true, accessibilityName: ":m") { KzTriggerChip(":m") }
                    KzListRow(meta: "Last used 4 hours ago", aside: "1", accessibilityName: ":an") { KzTriggerChip(":an") }
                    KzListRow("kudzu-backup-2026-11-02", meta: "kudzu-backup-2026-11-02.json", aside: "18") { KzBadge("Manual") }
                    KzListRow("Hovered", meta: "kudzu-backup-2026-11-01.json", aside: "17") { KzBadge("Automatic", tone: .vine) }
                        .kzPreviewInteraction(hovered: true)
                    KzListRow("Title only")
                }
                .padding(KzSpace.s2)
                .frame(width: KzSize.list)
                .background(KzColor.surface)
            }
        }
    }
}
