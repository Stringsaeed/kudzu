import KudzuUI
import SwiftUI

struct StatTilePage: GalleryPage {
    static let id = "stat-tile"
    static let title = "StatTile"
    static let section = GallerySection.structure
    static let icon = KzIconName.chart

    var body: some View {
        GalleryPageLayout(title: Self.title, summary: "A headline number on the Stats view: label, stat value (tabular), optional icon and note. Three across with space-3 gaps; tiles stretch to the tallest.") {
            SpecimenSection("Row") {
                KzStatTileRow {
                    KzStatTile("Expansions", value: "1,284", icon: .bolt, note: "+12% on last week")
                    KzStatTile("Characters saved", value: "30", icon: .type)
                    KzStatTile("Time saved", value: "2h 14m", icon: .timer)
                }
            }
        }
    }
}
