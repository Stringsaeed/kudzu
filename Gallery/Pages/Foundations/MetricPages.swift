import KudzuUI
import SwiftUI

struct SpacingPage: GalleryPage {
    static let id = "spacing"
    static let title = "Spacing"
    static let section = GallerySection.foundations
    static let icon = KzIconName.sliders

    var body: some View {
        GalleryPageLayout(title: "Spacing", summary: "A 4pt base. Separate with spacing first, hairlines second.") {
            MetricTable(prefix: "KzSpace", metrics: KzSpace.all) { metric in
                Rectangle().fill(KzColor.vine).frame(width: metric.value, height: 16)
            }
            SpecimenSection("Sizes", note: "Fixed dimensions of controls and panes.") {
                MetricTable(prefix: "KzSize", metrics: KzSize.all) { metric in
                    Rectangle().fill(KzColor.vineSoft).frame(width: min(metric.value, 200), height: 16)
                        .overlay(alignment: .trailing) {
                            if metric.value > 200 { Text("…").kzTextStyle(.caption).foregroundStyle(KzColor.inkMuted).padding(.trailing, 4) }
                        }
                }
            }
            SpecimenSection("Blur") {
                MetricTable(prefix: "KzBlur", metrics: KzBlur.all) { _ in EmptyView() }
            }
        }
    }
}

struct RadiiPage: GalleryPage {
    static let id = "radii"
    static let title = "Radii"
    static let section = GallerySection.foundations
    static let icon = KzIconName.app

    var body: some View {
        GalleryPageLayout(title: "Radii", summary: "From inline tags (xs) to the window (xl); full for toggles, badges and bars.") {
            MetricTable(prefix: "KzRadius", metrics: KzRadius.all) { metric in
                RoundedRectangle(cornerRadius: min(metric.value, 28))
                    .fill(KzColor.surfaceSunken)
                    .overlay(RoundedRectangle(cornerRadius: min(metric.value, 28)).strokeBorder(KzColor.edge))
                    .frame(width: 56, height: 56)
            }
        }
    }
}

/// A table of metric tokens: Swift name, value, usage and a visual.
private struct MetricTable<Visual: View>: View {
    let prefix: String
    let metrics: [KzMetric]
    @ViewBuilder let visual: (KzMetric) -> Visual

    var body: some View {
        GalleryCard(padding: 0) {
            VStack(spacing: 0) {
                ForEach(metrics) { metric in
                    HStack(alignment: .center, spacing: KzSpace.s4) {
                        VStack(alignment: .leading, spacing: 2) {
                            TokenName("\(prefix).\(metric.name)")
                            Text("\(metric.token) · \(format(metric.value))pt").kzTextStyle(.monoCaption).foregroundStyle(KzColor.inkMuted)
                        }
                        .frame(width: 210, alignment: .leading)
                        visual(metric).frame(width: 210, alignment: .leading)
                        Text(metric.usage).kzTextStyle(.callout).foregroundStyle(KzColor.inkMuted)
                            .fixedSize(horizontal: false, vertical: true)
                            .frame(maxWidth: .infinity, alignment: .leading)
                    }
                    .padding(.horizontal, KzSpace.s4)
                    .padding(.vertical, KzSpace.s3)
                    if metric != metrics.last {
                        Rectangle().fill(KzColor.line).frame(height: 1)
                    }
                }
            }
        }
    }

    private func format(_ value: CGFloat) -> String {
        value.rounded() == value ? String(Int(value)) : String(format: "%.1f", value)
    }
}
