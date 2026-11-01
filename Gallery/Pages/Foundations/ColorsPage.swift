import KudzuUI
import SwiftUI

struct ColorsPage: GalleryPage {
    static let id = "colors"
    static let title = "Colours"
    static let section = GallerySection.foundations
    static let icon = KzIconName.stack

    var body: some View {
        GalleryPageLayout(
            title: "Colours",
            summary: "Every token in both themes, side by side. Use them as KzColor.<name>; they follow Light, Dark and Increase Contrast."
        ) {
            VStack(spacing: 0) {
                header
                ForEach(KzColorToken.allCases, id: \.self) { token in
                    ColorRow(token: token)
                    if token != KzColorToken.allCases.last {
                        Rectangle().fill(KzColor.line).frame(height: 1)
                    }
                }
            }
            .background(KzColor.surfaceRaised, in: RoundedRectangle(cornerRadius: KzRadius.lg))
            .overlay(RoundedRectangle(cornerRadius: KzRadius.lg).strokeBorder(KzColor.line))
        }
    }

    private var header: some View {
        HStack(spacing: KzSpace.s4) {
            Text("Token").frame(width: 190, alignment: .leading)
            Text("Light").frame(width: 120, alignment: .leading)
            Text("Dark").frame(width: 120, alignment: .leading)
            Text("Usage").frame(maxWidth: .infinity, alignment: .leading)
        }
        .kzTextStyle(.caption)
        .foregroundStyle(KzColor.inkMuted)
        .padding(.horizontal, KzSpace.s4)
        .padding(.vertical, KzSpace.s2)
    }
}

private struct ColorRow: View {
    let token: KzColorToken

    var body: some View {
        HStack(alignment: .top, spacing: KzSpace.s4) {
            VStack(alignment: .leading, spacing: 2) {
                TokenName("KzColor.\(swiftName)")
                Text(aliasNote).kzTextStyle(.monoCaption).foregroundStyle(KzColor.inkMuted)
            }
            .frame(width: 190, alignment: .leading)
            Swatch(value: token.light, ground: KzColorToken.surfaceRaised.light, ink: KzColorToken.ink.light, muted: KzColorToken.inkMuted.light)
            Swatch(value: token.dark, ground: KzColorToken.surfaceRaised.dark, ink: KzColorToken.ink.dark, muted: KzColorToken.inkMuted.dark)
            Text(token.usage).kzTextStyle(.callout).foregroundStyle(KzColor.inkMuted)
                .fixedSize(horizontal: false, vertical: true)
                .frame(maxWidth: .infinity, alignment: .leading)
        }
        .padding(.horizontal, KzSpace.s4)
        .padding(.vertical, KzSpace.s3)
    }

    private var swiftName: String {
        let parts = token.rawValue.split(separator: "-")
        return parts.enumerated().map { $0.offset == 0 ? String($0.element) : $0.element.capitalized }.joined()
    }

    private var aliasNote: String {
        if let alias = token.aliasOf { return "\(token.rawValue) = {\(alias.rawValue)}" }
        return token.rawValue
    }
}

/// A swatch drawn from explicit theme values, so both themes show regardless of the app's appearance.
private struct Swatch: View {
    let value: KzRGBA
    let ground: KzRGBA
    let ink: KzRGBA
    let muted: KzRGBA

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            RoundedRectangle(cornerRadius: KzRadius.sm)
                .fill(value.color)
                .frame(height: 28)
                .overlay(RoundedRectangle(cornerRadius: KzRadius.sm).strokeBorder(.black.opacity(0.08)))
            Text(value.hex).kzTextStyle(.monoCaption).foregroundStyle(muted.color)
        }
        .padding(6)
        .frame(width: 120)
        .background(ground.color, in: RoundedRectangle(cornerRadius: KzRadius.md))
    }
}
