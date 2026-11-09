// A clipboard history row and its leading tile (`Row` in `clip-list.tsx`, `ClipTile` in
// `clip-tile.tsx`). Shared by the history screen and the picker panel. Selection and arrow keys
// belong to the native `List`; the row draws Kudzu's highlight instead of the accent color.

import AppKit
import KudzuCore
import KudzuDesign
import SwiftUI

/// Row metrics (`SIZES` in `clip-list.tsx`).
enum ClipDensity {
    case comfortable, compact

    /// The row slot, including 1 pt above and below the highlight.
    var row: CGFloat { self == .comfortable ? 58 : 50 }
    /// A day heading's slot.
    var header: CGFloat { self == .comfortable ? 30 : 28 }
    var tile: CGFloat { self == .comfortable ? 36 : 32 }
}

/// The leading visual: a thumbnail for images, a swatch for colors, a kind glyph for everything
/// else, with the source app's icon in the corner like a Finder badge.
struct ClipTile: View {
    let entry: ClipEntry
    var active = false
    var size: CGFloat = 36
    var badge = true
    @Environment(\.colorScheme) private var scheme

    private var radius: CGFloat { (size * 0.26).rounded() }
    private var isPicture: Bool { entry.kind == .image || entry.kind == .color }

    var body: some View {
        let shape = RoundedRectangle(cornerRadius: radius)
        content
            .frame(width: size, height: size)
            .overlay {
                // `ring-2 ring-iris/70 ring-offset-1` around a focused thumbnail or swatch.
                if active && isPicture {
                    RoundedRectangle(cornerRadius: radius + 3)
                        .strokeBorder(Color.kz.iris.opacity(0.7), lineWidth: 2)
                        .padding(-3)
                }
            }
            .overlay(alignment: .bottomTrailing) {
                if badge, let source = entry.source {
                    AppIcon(path: source.path, name: source.name, size: 14)
                        .clipShape(RoundedRectangle(cornerRadius: 3))
                        .padding(1)
                        .background(Color.kz.background, in: RoundedRectangle(cornerRadius: 5))
                        .overlay(RoundedRectangle(cornerRadius: 5).strokeBorder(Color.kz.hairline))
                        .offset(x: 4, y: 4)
                }
            }
            .accessibilityHidden(true)
            .animation(.easeOut(duration: 0.1), value: active)
            .contentShape(shape)
    }

    @ViewBuilder private var content: some View {
        let shape = RoundedRectangle(cornerRadius: radius)
        if entry.kind == .color, let rgb = Clipboard.cssToRGB(entry.color ?? entry.text ?? "") {
            shape.fill(Color(.sRGB, red: rgb.r, green: rgb.g, blue: rgb.b, opacity: rgb.a))
                .overlay(shape.strokeBorder(Color.black.opacity(0.12)))
        } else if entry.kind == .image || (entry.kind == .file && entry.image != nil) {
            ZStack {
                TransparencyCheckerboard()
                ClipMedia(entry: entry, kind: .thumb, fill: true) { glyph(Icon.image) }
            }
            .clipShape(shape)
            .overlay(shape.strokeBorder(Color.kz.hairline))
        } else if entry.kind == .file {
            ClipMedia(entry: entry, kind: .icon) { glyph(entry.files.count > 1 ? Icon.files : Icon.file) }
        } else {
            glyph(entry.isSecret ? Icon.keyRound : Self.kindIcon(entry.kind))
        }
    }

    /// `bg-iris text-iris-contrast shadow-accent sheen-strong` when active, else a quiet well.
    private func glyph(_ icon: Image) -> some View {
        let shape = RoundedRectangle(cornerRadius: radius)
        return icon.icon(size >= 32 ? 16 : 14)
            .foregroundStyle(active ? Color.kz.irisContrast : Color.kz.mutedForeground)
            .frame(width: size, height: size)
            .background {
                if active {
                    Color.clear.depth(.accent, fill: Color.kz.iris, in: shape).sheen(.strong, in: shape)
                } else {
                    shape.fill(Color.kz.foreground.opacity(0.055)).overlay(shape.strokeBorder(Color.kz.hairline))
                }
            }
    }

    static func kindIcon(_ kind: ClipKind) -> Image {
        switch kind {
        case .text, .color: Icon.alignLeft
        case .code: Icon.braces
        case .link: Icon.globe
        case .image: Icon.image
        case .file: Icon.file
        case .other: Icon.package
        }
    }
}

struct ClipRowView: View {
    @Environment(AppModel.self) private var model
    let entry: ClipEntry
    let dated: Bool
    let terms: [String]
    var density: ClipDensity = .comfortable
    /// The previewed row (`active`).
    var active = false
    /// Part of a multi-selection (`selected`).
    var inSelection = false
    /// The list has focus or holds one selection (`focused`).
    var focused = true
    /// ⌘1–9 in the picker.
    var number: Int?
    var numbersAccent = false
    @State private var hovering = false

    private var concealed: Bool { entry.isSecret && !model.settings.clipboardShowSecrets }
    private var veiled: Bool { concealed && !hovering }
    private var mono: Bool { entry.kind == .code || entry.kind == .color }

    var body: some View {
        let described = Clipboard.describe(entry)
        HStack(spacing: 12) {
            ClipTile(entry: entry, active: active && focused, size: density.tile)
            VStack(alignment: .leading, spacing: 2) {
                title(described.title)
                subtitle(described.subtitle)
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            VStack(alignment: .trailing, spacing: 4) {
                Text(Clipboard.clipTime(entry.copiedAt, dated: dated))
                    .font(.system(size: 11).monospacedDigit())
                    .foregroundStyle(Color.kz.faint)
                HStack(spacing: 4) {
                    if entry.copyCount > 1 {
                        Text("×\(entry.copyCount)")
                            .font(.system(size: 10.5).monospacedDigit())
                            .foregroundStyle(Color.kz.faint)
                            .help("Copied \(entry.copyCount) times")
                    }
                    if entry.pinned {
                        Icon.pin.icon(12).rotationEffect(.degrees(-45)).foregroundStyle(Color.kz.irisForeground.opacity(0.8))
                    }
                }
                .frame(height: 14)
            }
            if let number { Shortcut("mod+\(number)", tone: numbersAccent ? .accent : .ghost) }
        }
        .padding(.horizontal, 10)
        .frame(height: density.row - 2)
        .background { highlight }
        .contentShape(Rectangle())
        .onHover { hovering = $0 }
        .animation(.easeOut(duration: 0.075), value: hovering) // `duration-75`
        .kzOwnSelection()
        .accessibilityElement(children: .ignore)
        .accessibilityLabel(Clipboard.spokenLabel(entry, concealed: concealed))
    }

    private func title(_ text: String) -> some View {
        let shown = veiled ? Self.dots(text.count) : ClipPreviewRules.rowText(text.isEmpty ? "Empty" : text)
        return Text(ClipMarkedText.make(shown, terms: veiled ? [] : terms))
            .font(mono ? .kz.mono(12.5) : .system(size: 13, weight: .medium))
            .italic(text.isEmpty)
            .foregroundStyle(text.isEmpty ? Color.kz.mutedForeground : Color.kz.foreground)
            .lineLimit(1)
            .frame(height: 18)
    }

    private func subtitle(_ text: String) -> some View {
        let shown = veiled ? "\(Clipboard.secretReason(entry)) · hover to show" : ClipPreviewRules.rowText(text)
        return Text(ClipMarkedText.make(shown, terms: veiled ? [] : terms))
            .font(entry.kind == .link && !veiled ? .kz.mono(11) : .system(size: 11.5))
            .foregroundStyle(Color.kz.mutedForeground.opacity(0.85))
            .lineLimit(1)
            .frame(height: 16)
    }

    /// `hover:bg-foreground/[0.035]`; the multi-selection in `foreground/[0.06] shadow-selected`; the
    /// previewed row iris-soft with an inset `ring-iris/30` while the list is focused, else gray.
    @ViewBuilder private var highlight: some View {
        let shape = RoundedRectangle(cornerRadius: 10)
        if active && focused {
            Color.clear
                .depth(.selected, fill: Color.kz.irisSoft, in: shape)
                .overlay(shape.strokeBorder(Color.kz.iris.opacity(0.3), lineWidth: 1))
        } else if active {
            Color.clear.depth(.selected, fill: Color.kz.foreground.opacity(0.07), in: shape)
        } else if inSelection {
            Color.clear.depth(.selected, fill: Color.kz.foreground.opacity(0.06), in: shape)
        } else if hovering {
            shape.fill(Color.kz.foreground.opacity(0.035))
        }
    }

    /// Dots standing in for hidden text, about as long as the text (`dots` in `secret.tsx`).
    static func dots(_ length: Int) -> String { String(repeating: "•", count: min(max(length, 8), 18)) }
}

/// A day heading in the list (`Today 5`): 11 pt medium faint, sitting on the slot's bottom edge.
struct ClipSectionHeader: View {
    let label: String
    let count: Int
    var density: ClipDensity = .comfortable

    var body: some View {
        HStack(spacing: 6) {
            Text(label)
            Text("\(count)").monospacedDigit().opacity(0.7)
        }
        .font(.system(size: 11, weight: .medium))
        .foregroundStyle(Color.kz.faint)
        .padding(.horizontal, 16)
        .padding(.bottom, 6)
        .frame(maxWidth: .infinity, minHeight: density.header, alignment: .bottomLeading)
        .accessibilityAddTraits(.isHeader)
    }
}
