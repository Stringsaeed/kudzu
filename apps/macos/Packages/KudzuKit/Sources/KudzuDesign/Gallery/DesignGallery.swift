// A living reference of the design system: every token, depth level, motion, style and custom
// view, in the current appearance. Public so the app can show it behind a hidden debug menu.

import SwiftUI

/// A browsable reference for Kudzu’s native design system.
public struct DesignGallery: View {
    /// A browsable section of the design gallery.
    public enum Section: String, CaseIterable, Identifiable, Sendable {
        case tokens = "Tokens", depth = "Depth", motion = "Motion", controls = "Controls", views = "Views", icons = "Icons"
        /// Stable identity for this value.
        public var id: String { rawValue }
    }

    @State private var section: Section = .tokens
    @Environment(\.colorScheme) private var scheme

    /// Opens the gallery at a chosen section for previews and review exports.
    public init(section: Section = .tokens) { _section = State(initialValue: section) }

    /// The composed native view.
    public var body: some View {
        let t = KZTheme(scheme)
        VStack(spacing: 0) {
            HStack {
                Wordmark(.small).frame(width: 76).foregroundStyle(t[.foreground])
                Spacer()
                Segmented("Section", selection: $section, options: Section.allCases.map { SegmentOption($0.rawValue, value: $0) })
            }
            .padding(16)
            Divider()
            ScrollView {
                VStack(alignment: .leading, spacing: 24) {
                    switch section {
                    case .tokens: GalleryTokens()
                    case .depth: GalleryDepth()
                    case .motion: GalleryMotion()
                    case .controls: GalleryControls()
                    case .views: GalleryViews()
                    case .icons: GalleryIcons()
                    }
                }
                .padding(24)
                .frame(maxWidth: .infinity, alignment: .leading)
            }
        }
        .font(.system(size: 13))
        .foregroundStyle(t[.foreground])
        .background(t[.background])
        .kzToaster()
    }
}

/// A titled gallery block.
struct GalleryBlock<Content: View>: View {
    var title: String
    var content: Content
    init(_ title: String, @ViewBuilder content: () -> Content) {
        self.title = title
        self.content = content()
    }
    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text(title).font(.system(size: 11, weight: .medium)).tracking(Tracking.wide * 11).textCase(.uppercase).faint()
            content
        }
    }
}

// MARK: Tokens

struct GalleryTokens: View {
    @Environment(\.colorScheme) private var scheme
    var body: some View {
        let t = KZTheme(scheme)
        GalleryBlock("Colour tokens (\(scheme == .dark ? "dark" : "light"))") {
            LazyVGrid(columns: [GridItem(.adaptive(minimum: 240), spacing: 12)], alignment: .leading, spacing: 12) {
                ForEach(KZToken.allCases, id: \.self) { token in
                    HStack(spacing: 8) {
                        RoundedRectangle(cornerRadius: Radius.md)
                            .fill(t[token])
                            .frame(width: 28, height: 28)
                            .depth(.hairline, in: RoundedRectangle(cornerRadius: Radius.md))
                        VStack(alignment: .leading, spacing: 0) {
                            Text(token.cssName).font(KZFonts.mono(size: 11)).lineLimit(1)
                            Text(t.value(token).srgb.hex + (t.value(token).alpha < 1 ? " / \(Int(t.value(token).alpha * 100))%" : ""))
                                .font(.system(size: 11).monospacedDigit()).mutedForeground()
                        }
                    }
                }
            }
        }
        GalleryBlock("Radius") {
            HStack(spacing: 12) {
                ForEach([("sm", Radius.sm), ("md", Radius.md), ("lg", Radius.lg), ("xl", Radius.xl), ("2xl", Radius.xl2), ("3xl", Radius.xl3), ("4xl", Radius.xl4)], id: \.0) { name, r in
                    VStack(spacing: 4) {
                        Color.clear.frame(width: 56, height: 40).depth(.raised, fill: t[.surfaceRaised], in: RoundedRectangle(cornerRadius: r))
                        Text("\(name) \(Int(r))").font(.system(size: 11)).mutedForeground()
                    }
                }
            }
        }
        GalleryBlock("Type") {
            VStack(alignment: .leading, spacing: 8) {
                Text("Body 13/20 — For things worth repeating").textStyle(.body)
                Text("text-sm 14/20 medium").textStyle(KZTextStyle.sm.weight(.medium))
                Text("text-xs 12/16 muted").textStyle(.xs).mutedForeground()
                Text("11 pt label, wide tracking").textStyle(KZTextStyle.label11.tracking(Tracking.wide)).faint()
                Text("1,284").textStyle(.insight)
                Text(";kudzu {{date:yyyy-MM-dd}}").textStyle(.editor)
                Text("Paper Mono \(KZFonts.hasPaperMono ? "registered" : "missing — falling back to SF Mono")").font(.system(size: 11)).faint()
            }
        }
    }
}

// MARK: Depth

struct GalleryDepth: View {
    @Environment(\.colorScheme) private var scheme
    var body: some View {
        let t = KZTheme(scheme)
        GalleryBlock("Depth levels") {
            LazyVGrid(columns: [GridItem(.adaptive(minimum: 140), spacing: 24)], alignment: .leading, spacing: 24) {
                ForEach(Depth.all, id: \.name) { depth in
                    Text(depth.name)
                        .font(.system(size: 12, weight: .medium))
                        .foregroundStyle(depth.name == "accent" ? t[.irisContrast] : t[.foreground])
                        .frame(width: 140, height: 72)
                        .depth(depth, fill: fill(depth, t), in: RoundedRectangle(cornerRadius: Radius.lg))
                }
            }
            .padding(16)
            .background(t[.background])
        }
        GalleryBlock("Sheen · noise") {
            HStack(spacing: 16) {
                Text("sheen").frame(width: 120, height: 48).sheen(.normal, in: RoundedRectangle(cornerRadius: Radius.lg)).depth(.raised, fill: t[.secondary], in: RoundedRectangle(cornerRadius: Radius.lg))
                Text("sheen-strong").foregroundStyle(t[.primaryForeground]).frame(width: 120, height: 48).sheen(.strong, in: RoundedRectangle(cornerRadius: Radius.lg)).background(t[.primary], in: RoundedRectangle(cornerRadius: Radius.lg))
                Text("noise").frame(width: 120, height: 48).noise().background(t[.card], in: RoundedRectangle(cornerRadius: Radius.lg)).clipShape(RoundedRectangle(cornerRadius: Radius.lg))
            }
        }
        GalleryBlock("Materials") {
            HStack(spacing: 16) {
                Text("sidebar").frame(width: 160, height: 80).background(VisualEffect.sidebar).clipShape(RoundedRectangle(cornerRadius: Radius.xl))
                Text("popover").frame(width: 160, height: 80).background(VisualEffect(material: .popover, state: .active, cornerRadius: Radius.palettePanel))
                VibrantSurface { Text("bg-sidebar (vibrant)").frame(width: 160, height: 80).background(.kzSidebar) }
            }
        }
    }

    private func fill(_ d: Depth, _ t: KZTheme) -> Color {
        switch d.name {
        case "accent": t[.iris]
        case "well", "pressed": t.dark ? KZTheme.black(0.15) : t[.foreground, alpha: 0.025]
        case "key": t[.foreground, alpha: 0.06]
        case "card": t[.card]
        case "float", "panel": t[.popover]
        default: t[.surfaceRaised]
        }
    }
}
