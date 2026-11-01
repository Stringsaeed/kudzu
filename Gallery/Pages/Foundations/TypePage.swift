import KudzuUI
import SwiftUI

struct TypePage: GalleryPage {
    static let id = "type"
    static let title = "Type"
    static let section = GallerySection.foundations
    static let icon = KzIconName.type

    var body: some View {
        GalleryPageLayout(
            title: "Type",
            summary: "Geist for UI text, Geist Mono for triggers and literal text. Apply a style with .kzTextStyle(_:); a single line is exactly the CSS line height."
        ) {
            if case .failed(let missing) = KzFonts.status {
                Text("Font registration failed for \(missing.joined(separator: ", ")); showing the system fallback.")
                    .kzTextStyle(.callout).foregroundStyle(KzColor.danger)
            }
            ForEach(["Display", "Text", "Mono"], id: \.self) { group in
                SpecimenSection(group) {
                    GalleryCard(padding: 0) {
                        VStack(spacing: 0) {
                            let styles = KzTextStyle.allCases.filter { $0.spec.group == group }
                            ForEach(styles, id: \.self) { style in
                                TypeRow(style: style)
                                if style != styles.last {
                                    Rectangle().fill(KzColor.line).frame(height: 1)
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

private struct TypeRow: View {
    let style: KzTextStyle

    var body: some View {
        let spec = style.spec
        HStack(alignment: .top, spacing: KzSpace.s4) {
            VStack(alignment: .leading, spacing: 2) {
                TokenName(".kzTextStyle(.\(swiftName))")
                Text(details(spec)).kzTextStyle(.monoCaption).foregroundStyle(KzColor.inkMuted)
                    .fixedSize(horizontal: false, vertical: true)
            }
            .frame(width: 250, alignment: .leading)
            VStack(alignment: .leading, spacing: KzSpace.s1) {
                Text(spec.sample)
                    .kzTextStyle(style)
                    .foregroundStyle(KzColor.ink)
                    // The tinted box is the line box, to check line heights at a glance.
                    .background(KzColor.vineSoft)
                Text(style.usage).kzTextStyle(.callout).foregroundStyle(KzColor.inkMuted)
                    .fixedSize(horizontal: false, vertical: true)
            }
            .frame(maxWidth: .infinity, alignment: .leading)
        }
        .padding(KzSpace.s4)
    }

    private var swiftName: String {
        let parts = style.rawValue.split(separator: "-")
        return parts.enumerated().map { $0.offset == 0 ? String($0.element) : $0.element.capitalized }.joined()
    }

    private func details(_ spec: KzTextSpec) -> String {
        var parts = ["\(style.postScriptName)", "\(Int(spec.size))/\(Int(spec.lineHeight))"]
        if spec.trackingEm != 0 { parts.append("\(spec.trackingEm)em") }
        if spec.tabularFigures { parts.append("tabular") }
        return parts.joined(separator: " · ")
    }
}
