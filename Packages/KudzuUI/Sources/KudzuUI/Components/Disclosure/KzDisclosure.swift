import SwiftUI

/// A collapsible section in the snippet editor (Rules, Advanced): a chevron and a `headline` title,
/// with an optional one-line summary shown while closed. Content is indented 18px.
///
/// Built on `DisclosureGroup` with `KzDisclosureStyle`, so the header is a real button: Space and
/// Return toggle it, and VoiceOver announces expanded or collapsed.
public struct KzDisclosure<Content: View>: View {
    private let title: String
    private let summary: String?
    private let content: Content
    private let external: Binding<Bool>?
    @State private var internalExpanded: Bool

    /// Creates a disclosure that keeps its own state.
    /// - Parameters:
    ///   - title: The `headline` title.
    ///   - summary: One line shown at the right while closed ("Word boundary, case, undo").
    ///   - isInitiallyExpanded: The starting state.
    ///   - content: Shown indented 18px, stacked `space-3` apart, when open.
    public init(_ title: String, summary: String? = nil, isInitiallyExpanded: Bool = false, @ViewBuilder content: () -> Content) {
        self.title = title
        self.summary = summary
        self.content = content()
        self.external = nil
        _internalExpanded = State(initialValue: isInitiallyExpanded)
    }

    /// Creates a disclosure whose state the caller owns.
    public init(_ title: String, summary: String? = nil, isExpanded: Binding<Bool>, @ViewBuilder content: () -> Content) {
        self.title = title
        self.summary = summary
        self.content = content()
        self.external = isExpanded
        _internalExpanded = State(initialValue: isExpanded.wrappedValue)
    }

    public var body: some View {
        DisclosureGroup(isExpanded: external ?? $internalExpanded) {
            VStack(alignment: .leading, spacing: KzSpace.s3) { content }
        } label: {
            Text(title)
        }
        .disclosureGroupStyle(KzDisclosureStyle(summary: summary))
    }
}

/// The Kudzu look for a `DisclosureGroup`: chevron, `headline` title, optional summary while closed.
public struct KzDisclosureStyle: DisclosureGroupStyle {
    private let summary: String?

    /// Creates the style; `summary` is shown only while the group is closed.
    public init(summary: String? = nil) { self.summary = summary }

    public func makeBody(configuration: Configuration) -> some View {
        VStack(alignment: .leading, spacing: 0) {
            // Only the chevron animates; the content appears at once, because layout never animates.
            Button {
                configuration.isExpanded.toggle()
            } label: {
                HStack(spacing: 6) {
                    KzIcon(.chevronRight, size: 12)
                        .foregroundStyle(KzColor.inkMuted)
                        .rotationEffect(.degrees(configuration.isExpanded ? 90 : 0))
                        .kzAnimation(.color, value: configuration.isExpanded)
                    configuration.label
                        .kzTextStyle(.headline, matchLineBox: false)
                        .foregroundStyle(KzColor.ink)
                    Spacer(minLength: KzSpace.s2)
                    if let summary, !configuration.isExpanded {
                        Text(summary)
                            .kzTextStyle(.callout, matchLineBox: false)
                            .foregroundStyle(KzColor.inkMuted)
                            .lineLimit(1)
                    }
                }
                .padding(.trailing, 4)
                .frame(height: 28)
                .contentShape(Rectangle())
            }
            .buttonStyle(HeaderStyle())
            .accessibilityValue(configuration.isExpanded ? "Expanded" : "Collapsed")
            if configuration.isExpanded {
                configuration.content
                    .padding(.leading, 18)
                    .padding(.top, KzSpace.s2)
                    .frame(maxWidth: .infinity, alignment: .leading)
            }
        }
    }

    private struct HeaderStyle: ButtonStyle {
        func makeBody(configuration: Configuration) -> some View {
            // The header has no fill of its own; it only needs the focus ring for keyboard users.
            KzInteractive(RoundedRectangle(cornerRadius: KzRadius.sm), isPressed: configuration.isPressed, stateLayer: false) { _ in
                configuration.label
            }
        }
    }
}

extension DisclosureGroupStyle where Self == KzDisclosureStyle {
    /// Kudzu's disclosure look.
    public static func kz(summary: String? = nil) -> KzDisclosureStyle { KzDisclosureStyle(summary: summary) }
}
