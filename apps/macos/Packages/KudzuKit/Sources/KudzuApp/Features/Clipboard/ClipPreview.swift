// The preview of a clipboard entry: the heading chips (`ClipHeading`), the content by kind
// (`clip-content.tsx`, covered while it is a secret), and the facts (`ClipDetails`, with the copy
// timeline and raw formats). Shared by the history screen, its Quick Look sheet and the picker.

import AppKit
import KudzuCore
import KudzuDesign
import SwiftUI

// MARK: Media

/// `checkerboard` in `clip-tile.tsx`: 14 pt conic tiles, foreground at 7%, so transparency reads.
struct TransparencyCheckerboard: View {
    var body: some View {
        Canvas { context, size in
            let cell: CGFloat = 7
            let color = GraphicsContext.Shading.color(Color.kz.mix(.foreground, alpha: 0.07))
            for row in 0..<Int(ceil(size.height / cell)) {
                for col in 0..<Int(ceil(size.width / cell)) where (row + col) % 2 == 1 {
                    context.fill(Path(CGRect(x: CGFloat(col) * cell, y: CGFloat(row) * cell, width: cell, height: cell)), with: color)
                }
            }
        }
        .accessibilityHidden(true)
    }
}

/// An image from the backend's media store (`ClipImage`): `fill` crops to the box (`cover`),
/// otherwise the image fits inside it. `fallback` shows while loading and when it can't load.
struct ClipMedia<Fallback: View>: View {
    @Environment(AppModel.self) private var model
    let entry: ClipEntry
    let kind: ClipMediaKind
    var index = 0
    var fill = false
    @ViewBuilder var fallback: Fallback
    @State private var image: NSImage?

    var body: some View {
        // A `ZStack`, not a `Group`: a group hands its modifiers to each child, so the load below
        // would land on a nested fallback `ClipMedia` instead of this view.
        ZStack {
            if let image {
                if fill {
                    Color.clear.overlay { Image(nsImage: image).resizable().interpolation(.high).scaledToFill() }.clipped()
                } else {
                    Image(nsImage: image).resizable().interpolation(.high).scaledToFit()
                }
            } else {
                fallback
            }
        }
        .task(id: "\(entry.id)-\(entry.hash)-\(kind.rawValue)-\(index)") {
            image = nil
            if let data = await model.backend.clipboardMedia(kind: kind, id: entry.id, index: index) { image = NSImage(data: data) }
        }
    }
}

extension ClipMedia where Fallback == EmptyView {
    init(entry: ClipEntry, kind: ClipMediaKind, index: Int = 0, fill: Bool = false) {
        self.init(entry: entry, kind: kind, index: index, fill: fill) { EmptyView() }
    }
}

/// Text with search matches filled iris-soft (`Marked`).
enum ClipMarkedText {
    static func make(_ text: String, terms: [String]) -> AttributedString {
        let value = text as NSString
        var result = AttributedString()
        var offset = 0
        func append(_ range: NSRange, hit: Bool) {
            guard range.length > 0, range.location >= 0, range.location + range.length <= value.length else { return }
            var piece = AttributedString(value.substring(with: range))
            if hit {
                piece.foregroundColor = Color.kz.irisForeground
                piece.backgroundColor = Color.kz.irisSoft
            }
            result.append(piece)
        }
        for range in Clipboard.matchRanges(text, terms: terms) where range.upperBound <= value.length {
            append(NSRange(location: offset, length: range.lowerBound - offset), hit: false)
            append(NSRange(location: range.lowerBound, length: range.upperBound - range.lowerBound), hit: true)
            offset = range.upperBound
        }
        append(NSRange(location: offset, length: max(0, value.length - offset)), hit: false)
        return result
    }
}

/// `rounded-xl bg-foreground/[0.025] shadow-well`: the recessed box previews sit in.
private struct PreviewWell: ViewModifier {
    var radius: CGFloat = Radius.xl
    func body(content: Content) -> some View {
        content.depth(.well, fill: Color.kz.foreground.opacity(0.025), in: RoundedRectangle(cornerRadius: radius))
    }
}

private extension View {
    func previewWell(radius: CGFloat = Radius.xl) -> some View { modifier(PreviewWell(radius: radius)) }
    /// `rounded-xl bg-surface shadow-card`.
    func previewCard() -> some View { depth(.card, fill: Color.kz.surface, in: RoundedRectangle(cornerRadius: Radius.xl)) }
}

/// A full-width well whose height follows `ratio` (width / height), clamped; no ratio takes the
/// largest height (`min-h-40 max-h-[420px]` with `aspect-ratio`). It measures its width, since a
/// vertical scroll view offers no height for `.aspectRatio` to work from.
private struct AspectWell<Content: View>: View {
    var ratio: CGFloat?
    var minHeight: CGFloat
    var maxHeight: CGFloat
    @ViewBuilder var content: Content
    @State private var width: CGFloat = 0

    var body: some View {
        let height = ratio.map { width > 0 ? width / $0 : minHeight } ?? maxHeight
        content
            .padding(16)
            .frame(maxWidth: .infinity)
            .frame(height: min(maxHeight, max(minHeight, height)))
            .onGeometryChange(for: CGFloat.self) { $0.size.width } action: { width = $0 }
            .previewWell()
    }
}

// MARK: Heading

/// The kind, Formatted, Secret and Pinned chips, and the copy time on the right.
struct ClipHeading: View {
    let entry: ClipEntry
    var onNotSecret: () -> Void = {}

    var body: some View {
        HStack(spacing: 8) {
            chip(Clipboard.kindLabel(entry.kind), fill: Color.kz.foreground.opacity(0.06), text: Color.kz.mutedForeground, weight: .medium)
            if entry.rich { chip("Formatted", fill: Color.kz.foreground.opacity(0.04), text: Color.kz.faint) }
            if entry.isSecret {
                HStack(spacing: 6) {
                    HStack(spacing: 4) { Icon.keyRound.icon(12); Text("Secret") }
                        .font(.system(size: 11, weight: .medium))
                        .foregroundStyle(Color.kz.warning)
                        .padding(.horizontal, 6).padding(.vertical, 2)
                        .background(Color.kz.warning.opacity(0.12), in: RoundedRectangle(cornerRadius: Radius.md))
                    NotSecretButton(action: onNotSecret)
                }
            }
            if entry.pinned {
                HStack(spacing: 4) { Icon.pin.icon(12).rotationEffect(.degrees(-45)); Text("Pinned") }
                    .font(.system(size: 11, weight: .medium))
                    .foregroundStyle(Color.kz.irisForeground)
                    .padding(.horizontal, 6).padding(.vertical, 2)
                    .background(Color.kz.irisSoft, in: RoundedRectangle(cornerRadius: Radius.md))
            }
            Spacer(minLength: 8)
            Text(Clipboard.clipTime(entry.copiedAt))
                .font(.system(size: 12).monospacedDigit())
                .foregroundStyle(Color.kz.faint)
                .lineLimit(1)
        }
    }

    private func chip(_ label: String, fill: Color, text: Color, weight: Font.Weight = .regular) -> some View {
        Text(label)
            .font(.system(size: 11, weight: weight))
            .foregroundStyle(text)
            .padding(.horizontal, 6).padding(.vertical, 2)
            .background(fill, in: RoundedRectangle(cornerRadius: Radius.md))
    }
}

private struct NotSecretButton: View {
    var action: () -> Void
    @State private var hovered = false
    var body: some View {
        Button("Not a secret?", action: action)
            .buttonStyle(.plain)
            .font(.system(size: 11))
            .underline(hovered)
            .foregroundStyle(hovered ? Color.kz.mutedForeground : Color.kz.faint)
            .onHover { hovered = $0 }
    }
}

// MARK: Content

/// The main preview of an entry, by kind (`ClipContent`). `large` is the Quick Look size.
struct ClipContentView: View {
    @Environment(AppModel.self) private var model
    let entry: ClipEntry
    var terms: [String] = []
    var large = false
    @State private var fullText: String?

    private var concealed: Bool { entry.isSecret && !model.settings.clipboardShowSecrets }

    var body: some View {
        Group {
            if concealed {
                Concealed(entry: entry) { content }
            } else {
                content
            }
        }
        .task(id: "\(entry.id)-\(entry.hash)") {
            fullText = nil
            fullText = await model.clipFullText(entry)
        }
    }

    private var text: String { fullText ?? entry.text ?? "" }

    @ViewBuilder private var content: some View {
        switch entry.kind {
        case .code: code
        case .link: link
        case .color: color
        case .image: image
        case .file: files
        case .other: other
        case .text: plain
        }
    }

    // `TextBody`: 13.5 pt (15 in Quick Look), relaxed leading, selectable.
    private var plain: some View {
        let preview = ClipPreviewRules.textPreview(text, terms: terms)
        return VStack(alignment: .leading, spacing: 12) {
            Text(ClipMarkedText.make(preview.shown, terms: terms))
                .font(.system(size: large ? 15 : 13.5))
                .lineSpacing(large ? 6 : 5)
                .foregroundStyle(Color.kz.foreground.opacity(0.9))
                .textSelection(.enabled)
                .frame(maxWidth: .infinity, alignment: .leading)
            if let more = ClipPreviewRules.moreCharacters(preview.hiddenCharacters) {
                Text(more).font(.system(size: 12)).foregroundStyle(Color.kz.faint)
            }
        }
    }

    // `CodeBody`: numbered lines in a well, mono 12.5 at 1.65 line height.
    private var code: some View {
        let preview = ClipPreviewRules.codePreview(text)
        let size: CGFloat = large ? 13.5 : 12.5
        let gutter = CGFloat(String(preview.lines.count).count) + 2.5
        return VStack(alignment: .leading, spacing: 8) {
            ScrollView(.horizontal, showsIndicators: false) {
                VStack(alignment: .leading, spacing: 0) {
                    ForEach(Array(preview.lines.enumerated()), id: \.offset) { index, line in
                        HStack(alignment: .firstTextBaseline, spacing: 0) {
                            Text("\(index + 1)")
                                .foregroundStyle(Color.kz.faint.opacity(0.7))
                                .frame(minWidth: gutter * size * 0.6, alignment: .trailing)
                                .padding(.leading, 12).padding(.trailing, 16)
                                .accessibilityHidden(true)
                            Text(ClipMarkedText.make(line.isEmpty ? " " : line, terms: terms))
                                .foregroundStyle(Color.kz.foreground.opacity(0.9))
                                .fixedSize()
                                .padding(.trailing, 16)
                        }
                        .frame(minHeight: size * 1.65)
                    }
                }
                .font(.kz.mono(size).monospacedDigit())
                .textSelection(.enabled)
                .padding(.vertical, 12)
            }
            if let more = ClipPreviewRules.moreLines(preview.hiddenLines) {
                Text(more).font(.system(size: 12)).foregroundStyle(Color.kz.faint).padding(.horizontal, 12).padding(.bottom, 4)
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .previewWell()
    }

    // `LinkBody`: globe tile and title, the URL in a well, then Open.
    private var link: some View {
        let url = (entry.text ?? text).trimmingCharacters(in: .whitespacesAndNewlines)
        let mail = url.lowercased().hasPrefix("mailto:")
        return VStack(alignment: .leading, spacing: 16) {
            HStack(spacing: 12) {
                Icon.globe.icon(20)
                    .foregroundStyle(Color.kz.mutedForeground)
                    .frame(width: 44, height: 44)
                    .background(Color.kz.foreground.opacity(0.05), in: RoundedRectangle(cornerRadius: Radius.xl))
                    .overlay(RoundedRectangle(cornerRadius: Radius.xl).strokeBorder(Color.kz.hairline))
                VStack(alignment: .leading, spacing: 0) {
                    Text(Clipboard.describe(entry).title)
                        .font(.system(size: large ? 20 : 17, weight: .semibold))
                        .tracking(-0.3)
                        .lineLimit(1)
                    Text(mail ? "Email address" : "Web link").font(.system(size: 12)).foregroundStyle(Color.kz.mutedForeground)
                }
            }
            Text(url)
                .font(.kz.mono(12.5))
                .lineSpacing(4)
                .foregroundStyle(Color.kz.foreground.opacity(0.85))
                .textSelection(.enabled)
                .padding(.horizontal, 14).padding(.vertical, 12)
                .frame(maxWidth: .infinity, alignment: .leading)
                .previewWell()
            Button { Task { try? await model.openClip(entry) } } label: {
                ButtonLabel(mail ? "Write email" : "Open in browser", icon: Icon.externalLink)
            }
            .buttonStyle(.kz(.secondary, size: .sm))
        }
    }

    // `ColorBody`: a big swatch with its hex and contrast chips, then each format to copy.
    @ViewBuilder private var color: some View {
        if let rgb = Clipboard.cssToRGB(entry.color ?? text) {
            let info = Clipboard.formats(rgb)
            let swatch = Color(.sRGB, red: rgb.r, green: rgb.g, blue: rgb.b, opacity: rgb.a)
            let shape = RoundedRectangle(cornerRadius: Radius.xl2)
            VStack(spacing: 16) {
                ZStack(alignment: .bottom) {
                    TransparencyCheckerboard()
                    swatch
                    HStack(alignment: .bottom) {
                        Text(info.values.first?.value ?? "")
                            .font(.kz.mono(14).weight(.medium))
                            .foregroundStyle(info.light ? Color.black.opacity(0.75) : Color.white.opacity(0.9))
                        Spacer()
                        HStack(spacing: 6) {
                            contrast(swatch, ratio: info.onWhite, on: .white, text: Color.black.opacity(0.8), help: "Contrast with white text")
                            contrast(swatch, ratio: info.onBlack, on: .black, text: Color.white.opacity(0.8), help: "Contrast with black text")
                        }
                    }
                    .padding(16)
                }
                .frame(height: large ? 256 : 160)
                .clipShape(shape)
                .overlay(shape.strokeBorder(Color.black.opacity(0.1)))
                .depth(.card, in: shape)
                VStack(spacing: 0) {
                    ForEach(Array(info.values.enumerated()), id: \.element.label) { index, value in
                        if index > 0 { Hairline() }
                        ColorFormatRow(label: value.label, value: value.value)
                    }
                }
                .previewCard()
            }
        }
    }

    private func contrast(_ swatch: Color, ratio: String, on background: Color, text: Color, help: String) -> some View {
        (Text("Aa").foregroundStyle(swatch) + Text(" \(ratio)").foregroundStyle(text))
            .font(.system(size: 11, weight: .semibold).monospacedDigit())
            .padding(.horizontal, 6).padding(.vertical, 2)
            .background(background, in: RoundedRectangle(cornerRadius: Radius.md))
            .help(help)
    }

    // `ImageBody`: the image at its own aspect in a well, the checkerboard only behind the image.
    private var image: some View {
        let ratio = entry.image.map { CGFloat($0.width) / CGFloat(max(1, $0.height)) } ?? 1.6
        return AspectWell(ratio: large ? nil : max(ratio, 0.9), minHeight: large ? 560 : 160, maxHeight: large ? 640 : 420) {
            naturalImage(kind: .image, maxWidth: entry.image.map { CGFloat($0.width) })
        }
        .accessibilityElement(children: .ignore)
        .accessibilityLabel("Copied image\(entry.image.map { ", \($0.width) by \($0.height) pixels" } ?? "")\(entry.source.map { ", from \($0.name)" } ?? "")")
    }

    private func naturalImage(kind: ClipMediaKind, maxWidth: CGFloat?) -> some View {
        ClipMedia(entry: entry, kind: kind) {
            ClipMedia(entry: entry, kind: .thumb)
        }
        .background(TransparencyCheckerboard())
        .clipShape(RoundedRectangle(cornerRadius: 6))
        .overlay(RoundedRectangle(cornerRadius: 6).strokeBorder(Color.kz.hairline))
        .shadow(color: .black.opacity(0.25), radius: 12, y: 8)
        .frame(maxWidth: maxWidth)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    // `FileBody`: a preview for a single image file, then the files in a card.
    private var files: some View {
        VStack(alignment: .leading, spacing: 12) {
            if entry.files.count == 1 && entry.image != nil {
                ClipMedia(entry: entry, kind: .file) { ClipMedia(entry: entry, kind: .thumb) }
                    .background(TransparencyCheckerboard())
                    .clipShape(RoundedRectangle(cornerRadius: 6))
                    .overlay(RoundedRectangle(cornerRadius: 6).strokeBorder(Color.kz.hairline))
                    .padding(16)
                    .frame(maxWidth: .infinity)
                    .frame(height: large ? 560 : 224)
                    .previewWell()
            }
            VStack(spacing: 0) {
                ForEach(Array(entry.files.prefix(200).enumerated()), id: \.offset) { index, path in
                    if index > 0 { Hairline() }
                    FileRow(entry: entry, index: index, path: path)
                }
            }
            .previewCard()
            if entry.files.count > 200 {
                Text("…and \(entry.files.count - 200) more.").font(.system(size: 12)).foregroundStyle(Color.kz.faint)
            }
        }
    }

    // `OtherBody`: app-specific data Kudzu can't draw but pastes back as it was.
    private var other: some View {
        VStack(alignment: .leading, spacing: 12) {
            Icon.package.icon(20)
                .foregroundStyle(Color.kz.mutedForeground)
                .frame(width: 40, height: 40)
                .background(Color.kz.foreground.opacity(0.06), in: RoundedRectangle(cornerRadius: Radius.xl))
            VStack(alignment: .leading, spacing: 4) {
                Text(entry.source.map { "Something copied in \($0.name)" } ?? "App-specific data")
                    .font(.system(size: 13.5, weight: .medium))
                Text("Kudzu can't show this kind of content, but it kept every format. Copy it again and it pastes \(entry.source.map { "into \($0.name)" } ?? "back") exactly as before.")
                    .font(.system(size: 12))
                    .lineSpacing(3)
                    .foregroundStyle(Color.kz.mutedForeground)
                    .frame(maxWidth: 384, alignment: .leading)
            }
        }
        .padding(16)
        .frame(maxWidth: .infinity, alignment: .leading)
        .previewWell()
    }
}

/// One copied file with Open and Show in Finder on hover.
private struct FileRow: View {
    @Environment(AppModel.self) private var model
    let entry: ClipEntry
    let index: Int
    let path: String
    @State private var hovered = false

    var body: some View {
        let name = Clipboard.basename(path)
        HStack(spacing: 12) {
            ClipMedia(entry: entry, kind: .icon, index: index) {
                RoundedRectangle(cornerRadius: Radius.lg).fill(Color.kz.foreground.opacity(0.06))
            }
            .frame(width: 32, height: 32)
            VStack(alignment: .leading, spacing: 0) {
                Text(name).font(.system(size: 13, weight: .medium)).lineLimit(1)
                Text(Clipboard.tildify(Clipboard.dirname(path)))
                    .font(.kz.mono(11))
                    .foregroundStyle(Color.kz.mutedForeground)
                    .lineLimit(1)
                    .textSelection(.enabled)
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            HStack(spacing: 2) {
                Button { Task { try? await model.openClip(entry, index: index) } } label: { Icon.externalLink.icon(14) }
                    .buttonStyle(.kz(.ghost, size: .iconXs))
                    .accessibilityLabel("Open \(name)")
                    .kzHelp("Open")
                Button { Task { try? await model.revealClip(entry, index: index) } } label: { Icon.folderOpen.icon(14) }
                    .buttonStyle(.kz(.ghost, size: .iconXs))
                    .accessibilityLabel("Show \(name) in Finder")
                    .kzHelp("Show in Finder")
            }
            .opacity(hovered ? 1 : 0)
        }
        .padding(.horizontal, 12).padding(.vertical, 10)
        .contentShape(Rectangle())
        .onHover { hovered = $0 }
        .animation(.kz.colors, value: hovered)
    }
}

/// One color format with a copy button that shows on hover (`CopyValue`). The check lasts 1.2 s.
private struct ColorFormatRow: View {
    let label: String
    let value: String
    @State private var copied = false
    @State private var hovered = false
    @State private var reset: Task<Void, Never>?

    var body: some View {
        HStack(spacing: 12) {
            Text(label).font(.system(size: 11, weight: .medium)).foregroundStyle(Color.kz.faint).frame(width: 48, alignment: .leading)
            Text(value).font(.kz.mono(12.5)).lineLimit(1).textSelection(.enabled).frame(maxWidth: .infinity, alignment: .leading)
            Button(action: copy) { (copied ? Icon.check : Icon.copy).icon(14) }
                .buttonStyle(.kz(.ghost, size: .iconXs))
                .foregroundStyle(copied ? Color.kz.success : Color.kz.mutedForeground)
                .opacity(hovered || copied ? 1 : 0)
                .accessibilityLabel("Copy \(label) value")
                .kzHelp("Copy \(label)")
        }
        .padding(.horizontal, 12)
        .frame(height: 36)
        .contentShape(Rectangle())
        .onHover { hovered = $0 }
    }

    private func copy() {
        let board = NSPasteboard.general
        board.clearContents()
        board.setString(value, forType: .string)
        copied = true
        reset?.cancel()
        reset = Task {
            try? await Task.sleep(for: .milliseconds(1200))
            if !Task.isCancelled { copied = false }
        }
    }
}

/// Covers a secret's preview until it is hovered or focused (`Concealed`). The content keeps its
/// layout underneath, so nothing jumps on reveal.
private struct Concealed<Content: View>: View {
    let entry: ClipEntry
    @ViewBuilder var content: Content
    @State private var hovered = false
    @FocusState private var focused: Bool

    var body: some View {
        let revealed = hovered || focused
        let shape = RoundedRectangle(cornerRadius: Radius.xl)
        ZStack {
            content.opacity(revealed ? 1 : 0).allowsHitTesting(revealed)
            if !revealed {
                VStack(spacing: 6) {
                    Icon.eyeOff.icon(16)
                        .foregroundStyle(Color.kz.mutedForeground)
                        .frame(width: 36, height: 36)
                        .background(Color.kz.foreground.opacity(0.06), in: RoundedRectangle(cornerRadius: Radius.xl))
                    Text(Clipboard.secretReason(entry)).font(.system(size: 13, weight: .medium))
                    Text("Hover to show").font(.system(size: 12)).foregroundStyle(Color.kz.faint)
                }
                .frame(maxWidth: .infinity, maxHeight: .infinity)
                .depth(.well, fill: Color.kz.foreground.opacity(0.025), in: shape)
                .accessibilityHidden(true)
            }
        }
        .frame(minHeight: 112)
        .contentShape(shape)
        .onHover { hovered = $0 }
        .focusable()
        .focused($focused)
        .accessibilityLabel("Hidden: \(Clipboard.secretReason(entry).lowercased()). Hover or focus to show.")
    }
}

// MARK: Details

/// When, where from, how big, how long it stays, plus the raw formats (`ClipDetails`).
struct ClipDetails: View {
    @Environment(AppModel.self) private var model
    let entry: ClipEntry
    var compact = false
    @State private var fullText: String?
    @State private var formatsOpen = false

    private var retention: Int { Int(model.settings.clipboardRetentionDays) }

    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            if !compact && entry.copyCount > 1 {
                VStack(alignment: .leading, spacing: 8) {
                    Text("Copied \(entry.copyCount) times").font(.system(size: 11, weight: .medium)).foregroundStyle(Color.kz.faint)
                    CopyTimeline(entry: entry, retentionDays: retention)
                }
                .accessibilityElement(children: .contain)
                .accessibilityLabel("Copy activity")
            }
            Grid(alignment: .leadingFirstTextBaseline, horizontalSpacing: 20, verticalSpacing: 6) {
                fact("Copied", Clipboard.exactTime(entry.copiedAt))
                if entry.copyCount > 1 { fact("First copied", Clipboard.exactTime(entry.firstCopiedAt)) }
                if compact && entry.copyCount > 1 { fact("Times", "\(entry.copyCount)×") }
                if let source = entry.source {
                    GridRow {
                        label("From")
                        HStack(spacing: 6) {
                            AppIcon(path: source.path, name: source.name, size: 14)
                            Text(source.name).lineLimit(1)
                        }
                        .foregroundStyle(Color.kz.foreground.opacity(0.85))
                    }
                }
                if entry.kind == .text || entry.kind == .code { fact("Length", lengthText) }
                if let image = entry.image { fact("Dimensions", "\(image.width) × \(image.height) px") }
                if entry.kind == .file { fact("Items", "\(entry.files.count)") }
                if entry.kind == .image || entry.kind == .other { fact("Size", Clipboard.formatBytes(Int(clamping: entry.size))) }
                if let last = entry.lastUsedAt, entry.useCount > 0 {
                    fact("Reused", "\(entry.useCount)× · last \(Clipboard.clipTime(last).lowercased())")
                }
                GridRow {
                    label("Expires")
                    Text(Clipboard.expiresIn(entry, retentionDays: retention))
                        .foregroundStyle(entry.pinned ? Color.kz.irisForeground : Color.kz.foreground.opacity(0.85))
                }
            }
            .font(.system(size: compact ? 11.5 : 12).monospacedDigit())
            if !compact && !entry.types.isEmpty { formats }
        }
        .task(id: "\(entry.id)-\(entry.hash)") {
            guard entry.kind == .text || entry.kind == .code else { return }
            fullText = await model.clipFullText(entry)
        }
    }

    private var lengthText: String {
        var parts = "\(entry.textLen.formatted()) character\(entry.textLen == 1 ? "" : "s")"
        if let fullText {
            let words = Clipboard.wordCount(fullText)
            parts += " · \(words.formatted()) word\(words == 1 ? "" : "s")"
        }
        if entry.lineCount > 1 { parts += " · \(entry.lineCount.formatted()) lines" }
        return parts
    }

    private func label(_ text: String) -> some View {
        Text(text).foregroundStyle(Color.kz.faint)
    }

    private func fact(_ title: String, _ value: String) -> some View {
        GridRow {
            label(title)
            Text(value).lineLimit(1).foregroundStyle(Color.kz.foreground.opacity(0.85))
        }
    }

    /// `<details>`: "N formats" with a turning chevron; the UTIs in mono when open.
    private var formats: some View {
        VStack(alignment: .leading, spacing: 8) {
            FormatsToggle(count: entry.types.count, rich: entry.rich, open: $formatsOpen)
            if formatsOpen {
                VStack(alignment: .leading, spacing: 2) {
                    ForEach(entry.types, id: \.self) { type in
                        Text(type).lineLimit(1)
                    }
                }
                .font(.kz.mono(11))
                .foregroundStyle(Color.kz.mutedForeground)
                .textSelection(.enabled)
                .padding(.leading, 16)
                .transition(.opacity)
            }
        }
        .font(.system(size: 12))
    }
}

private struct FormatsToggle: View {
    let count: Int
    let rich: Bool
    @Binding var open: Bool
    @State private var hovered = false

    var body: some View {
        Button {
            withAnimation(.kz.springSoft(duration: Motion.moderate)) { open.toggle() }
        } label: {
            HStack(spacing: 4) {
                Icon.chevronRight.icon(12).rotationEffect(.degrees(open ? 90 : 0))
                Text("\(count) format\(count == 1 ? "" : "s")")
                if rich {
                    Text("Formatted")
                        .font(.system(size: 10))
                        .padding(.horizontal, 4)
                        .background(Color.kz.foreground.opacity(0.06), in: RoundedRectangle(cornerRadius: 4))
                        .padding(.leading, 4)
                }
            }
            .foregroundStyle(hovered ? Color.kz.mutedForeground : Color.kz.faint)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .onHover { hovered = $0 }
        .accessibilityValue(open ? "expanded" : "collapsed")
    }
}

/// Every copy of this content on a track spanning the retention window (`CopyTimeline`). Hover a
/// dot for its exact time.
private struct CopyTimeline: View {
    let entry: ClipEntry
    let retentionDays: Int
    @State private var now = Date()

    var body: some View {
        let first = ClipPreviewRules.copyTimelineAnchor(timings: entry.timings, copiedAt: entry.copiedAt)
        let span = retentionDays > 0
            ? max(Double(retentionDays) * 86_400, now.timeIntervalSince(first))
            : max(86_400, now.timeIntervalSince(first))
        let start = now.addingTimeInterval(-span)
        let days = max(1, Int((span / 86_400).rounded()))
        VStack(alignment: .leading, spacing: 6) {
            GeometryReader { geo in
                ZStack(alignment: .topLeading) {
                    if days <= 31 {
                        ForEach(1..<max(days, 2), id: \.self) { i in
                            if i < days {
                                Rectangle().fill(Color.kz.hairline)
                                    .frame(width: 1, height: geo.size.height - 12)
                                    .offset(x: geo.size.width * CGFloat(i) / CGFloat(days), y: 6)
                            }
                        }
                    }
                    ForEach(Array(entry.timings.enumerated()), id: \.offset) { index, time in
                        let p = min(1, max(0, time.timeIntervalSince(start) / span))
                        TimelineDot(latest: index == entry.timings.count - 1, time: time)
                            .position(x: geo.size.width * (p * 0.96 + 0.02), y: geo.size.height / 2)
                    }
                }
            }
            .frame(height: 24)
            .depth(.well, fill: Color.kz.foreground.opacity(0.03), in: RoundedRectangle(cornerRadius: Radius.md))
            HStack {
                Text(retentionDays > 0 ? "\(days) day\(days == 1 ? "" : "s") ago" : start.formatted(.dateTime.month(.abbreviated).day()))
                Spacer()
                Text("Now")
            }
            .font(.system(size: 10.5).monospacedDigit())
            .foregroundStyle(Color.kz.faint)
        }
    }
}

private struct TimelineDot: View {
    let latest: Bool
    let time: Date
    @State private var hovered = false

    var body: some View {
        Circle()
            .fill(latest ? Color.kz.iris : Color.kz.iris.opacity(0.45))
            .frame(width: 8, height: 8)
            .background(Circle().fill(Color.kz.background).padding(-2))
            .scaleEffect(hovered ? 1.5 : 1)
            .animation(.kz.colors, value: hovered)
            .onHover { hovered = $0 }
            .help(Clipboard.exactTime(time))
    }
}

// MARK: Composition

/// Heading, content and facts stacked: the picker's preview pane (`compact`).
struct ClipDetail: View {
    let entry: ClipEntry
    var terms: [String] = []
    var onCorrect: () -> Void = {}
    var compact = false

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: compact ? 16 : 24) {
                ClipHeading(entry: entry, onNotSecret: onCorrect)
                ClipContentView(entry: entry, terms: terms)
                ClipDetails(entry: entry, compact: compact)
            }
            .padding(compact ? 16 : 24)
            .frame(maxWidth: .infinity, alignment: .leading)
        }
    }
}

#Preview("Clipboard History") {
    ClipboardPreviewHost(picker: false)
}

#Preview("Clipboard Picker") {
    ClipboardPreviewHost(picker: true)
}

private struct ClipboardPreviewHost: View {
    let picker: Bool
    @State private var model = AppModel(backend: MockBackend(), isDev: true)
    var body: some View {
        Group { if picker { ClipboardPickerView() } else { ClipboardHistoryView() } }
            .environment(model)
            .environment(LibraryFocus())
            .frame(width: 900, height: 600)
            .task { await model.start() }
    }
}
