import SwiftUI

/// A single headline number on the Stats view.
///
/// Lay tiles out three across with `space-3` gaps, for example in a `KzStatTileRow`.
public struct KzStatTile: View {
    private let label: String
    private let value: String
    private let icon: KzIconName?
    private let note: String?

    /// Creates a tile.
    /// - Parameters:
    ///   - label: What is counted ("Expansions"), in `ink-muted`.
    ///   - value: Pre-formatted: "1,284", "9s", "2h 14m". Set in `stat` with tabular figures.
    ///   - icon: A 14pt glyph before the label.
    ///   - note: A line of context below the value.
    public init(_ label: String, value: String, icon: KzIconName? = nil, note: String? = nil) {
        self.label = label
        self.value = value
        self.icon = icon
        self.note = note
    }

    public var body: some View {
        let shape = RoundedRectangle(cornerRadius: KzRadius.lg)
        VStack(alignment: .leading, spacing: KzSpace.s2) {
            HStack(spacing: 6) {
                if let icon { KzIcon(icon, size: 14) }
                Text(label).kzText(.adhoc(size: 12, lineHeight: 16, weight: 500))
            }
            .foregroundStyle(KzColor.inkMuted)
            Text(value)
                .kzTextStyle(.stat)
                .foregroundStyle(KzColor.ink)
                .lineLimit(1)
                .minimumScaleFactor(0.6)
            if let note {
                Text(note)
                    .kzTextStyle(.callout)
                    .foregroundStyle(KzColor.inkMuted)
            }
        }
        .padding(.top, 14)
        .padding(.horizontal, KzSpace.s4)
        .padding(.bottom, KzSpace.s4)
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        .background(KzColor.surfaceRaised, in: shape)
        .overlay(shape.strokeBorder(KzColor.line))
        .accessibilityElement(children: .ignore)
        .accessibilityLabel(label)
        .accessibilityValue(note.map { "\(value). \($0)" } ?? value)
    }
}

/// Three stat tiles across, `space-3` apart.
public struct KzStatTileRow<Content: View>: View {
    private let content: Content

    /// Creates the row; give it up to three `KzStatTile`s.
    public init(@ViewBuilder content: () -> Content) { self.content = content() }

    public var body: some View {
        // Tiles stretch to the tallest, as grid items do in the CSS.
        HStack(alignment: .top, spacing: KzSpace.s3) { content }
            .fixedSize(horizontal: false, vertical: true)
    }
}
