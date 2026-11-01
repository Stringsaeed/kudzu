import KudzuUI
import SwiftUI

/// The standard page layout: a title, a one-line summary, then sections. Not scrollable itself,
/// so the snapshot renderer can draw the whole page; the shell wraps it in a scroll view.
struct GalleryPageLayout<Content: View>: View {
    let title: String
    let summary: String
    @ViewBuilder let content: Content

    var body: some View {
        VStack(alignment: .leading, spacing: KzSpace.s8) {
            VStack(alignment: .leading, spacing: KzSpace.s1) {
                Text(title).kzTextStyle(.titleLg).foregroundStyle(KzColor.ink)
                Text(summary).kzTextStyle(.callout).foregroundStyle(KzColor.inkMuted)
                    .fixedSize(horizontal: false, vertical: true)
            }
            content
        }
        .padding(.horizontal, KzSpace.s8)
        .padding(.vertical, KzSpace.s8)
        .frame(maxWidth: .infinity, alignment: .leading)
    }
}

/// A titled group of specimens on a page.
struct SpecimenSection<Content: View>: View {
    let title: String
    var note: String?
    @ViewBuilder let content: Content

    init(_ title: String, note: String? = nil, @ViewBuilder content: () -> Content) {
        self.title = title
        self.note = note
        self.content = content()
    }

    var body: some View {
        VStack(alignment: .leading, spacing: KzSpace.s3) {
            VStack(alignment: .leading, spacing: 2) {
                Text(title).kzTextStyle(.headline).foregroundStyle(KzColor.ink)
                if let note {
                    Text(note).kzTextStyle(.callout).foregroundStyle(KzColor.inkMuted)
                        .fixedSize(horizontal: false, vertical: true)
                }
            }
            content
        }
    }
}

/// A card that holds specimens: `surface-raised`, `line` border, `radius-lg`.
struct GalleryCard<Content: View>: View {
    var padding: CGFloat = KzSpace.s4
    @ViewBuilder let content: Content

    var body: some View {
        content
            .padding(padding)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(KzColor.surfaceRaised, in: RoundedRectangle(cornerRadius: KzRadius.lg))
            .overlay(RoundedRectangle(cornerRadius: KzRadius.lg).strokeBorder(KzColor.line))
    }
}

/// A token's Swift name in mono, the way it's written at a call site.
struct TokenName: View {
    let text: String

    init(_ text: String) { self.text = text }

    var body: some View {
        Text(text).kzTextStyle(.code).foregroundStyle(KzColor.ink)
            .textSelection(.enabled)
    }
}

/// A colourful backdrop to show glass against.
struct GlassBackdrop: View {
    var body: some View {
        ZStack {
            LinearGradient(colors: [KzColorToken.vine.light.color, KzColorToken.bloom.light.color, KzColorToken.vineInk.light.color], startPoint: .topLeading, endPoint: .bottomTrailing)
            HStack(spacing: 18) {
                ForEach(0..<12) { i in
                    Capsule().fill(.white.opacity(i.isMultiple(of: 2) ? 0.55 : 0.15)).frame(width: 10)
                        .rotationEffect(.degrees(18))
                }
            }
            Text("The quick brown fox").kzTextStyle(.stat).foregroundStyle(.black.opacity(0.6))
                .offset(x: -120, y: 40)
        }
        .clipped()
    }
}
