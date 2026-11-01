import AppKit
import SwiftUI

/// The text of a `KzMessage`: a string literal that may contain `\(kbd:)` keycaps.
public struct KzMessageContent: ExpressibleByStringInterpolation, Sendable, Equatable {
    enum Segment: Equatable, Sendable {
        case text(String)
        case kbd(String)
    }

    /// Collects literal text and keycaps.
    public struct StringInterpolation: StringInterpolationProtocol {
        var segments: [Segment] = []

        /// Creates an empty interpolation.
        public init(literalCapacity: Int, interpolationCount: Int) {}

        /// Appends literal text.
        public mutating func appendLiteral(_ literal: String) {
            segments.append(.text(literal))
        }

        /// Appends a keycap.
        public mutating func appendInterpolation(kbd keys: String) {
            segments.append(.kbd(keys))
        }
    }

    let segments: [Segment]

    /// Creates content from a plain string.
    public init(stringLiteral value: String) { segments = [.text(value)] }

    /// Creates content from an interpolated string.
    public init(stringInterpolation: StringInterpolation) { segments = stringInterpolation.segments }
}

/// A sentence that can contain keycaps: "Choose a snippet to edit it, or press ⌘N to create one."
///
/// SwiftUI can't set a view inside wrapping text, so the message lays its words and keycaps out with
/// a flow layout. Write it as a string with interpolations:
///
/// ```swift
/// KzMessage("Choose a snippet to edit it, or press \(kbd: "⌘N") to create one.")
/// ```
///
/// VoiceOver reads it as one sentence, with the keys spelled out.
public struct KzMessage: View {
    typealias Segment = KzMessageContent.Segment

    let segments: [Segment]
    private let alignment: TextAlignment

    /// Creates a message.
    public init(_ content: KzMessageContent) {
        self.segments = content.segments
        self.alignment = .center
    }

    private init(segments: [Segment], alignment: TextAlignment) {
        self.segments = segments
        self.alignment = alignment
    }

    /// The same message with a different line alignment (centred by default).
    public func multilineTextAlignment(_ alignment: TextAlignment) -> KzMessage {
        KzMessage(segments: segments, alignment: alignment)
    }

    /// One laid-out item: a word or a keycap, and whether a space precedes it.
    struct Item: Identifiable {
        let id: Int
        let segment: Segment
        let spaceBefore: Bool
    }

    /// Splits segments into words and keycaps. A keycap or word that directly follows another
    /// without whitespace ("⌘N." or "(⌘N)") stays glued to it.
    static func items(from segments: [Segment]) -> [Item] {
        var items: [Item] = []
        var pendingSpace = false
        func add(_ segment: Segment) {
            items.append(Item(id: items.count, segment: segment, spaceBefore: pendingSpace && !items.isEmpty))
            pendingSpace = false
        }
        for segment in segments {
            switch segment {
            case .kbd:
                add(segment)
            case .text(let text):
                var word = ""
                for character in text {
                    if character.isWhitespace {
                        if !word.isEmpty { add(.text(word)); word = "" }
                        pendingSpace = true
                    } else {
                        word.append(character)
                    }
                }
                if !word.isEmpty { add(.text(word)) }
            }
        }
        return items
    }

    /// The message as plain text for VoiceOver, with keys spelled out.
    var spokenText: String {
        var out = ""
        for item in Self.items(from: segments) {
            if item.spaceBefore { out += " " }
            switch item.segment {
            case .text(let text): out += text
            case .kbd(let keys): out += KzKbd.spokenName(keys)
            }
        }
        return out
    }

    public var body: some View {
        let spaceWidth = " ".size(withAttributes: [.font: KzTextStyle.body.nsFont]).width
        KzFlowLayout(spaceWidth: spaceWidth, lineHeight: KzTextStyle.body.spec.lineHeight, alignment: alignment) {
            ForEach(Self.items(from: segments)) { item in
                Group {
                    switch item.segment {
                    case .text(let text):
                        Text(text).kzTextStyle(.body)
                    case .kbd(let keys):
                        KzKbd(keys)
                    }
                }
                .layoutValue(key: KzFlowSpaceBefore.self, value: item.spaceBefore)
            }
        }
        .accessibilityElement(children: .ignore)
        .accessibilityLabel(spokenText)
    }
}

// MARK: - Flow layout

struct KzFlowSpaceBefore: LayoutValueKey {
    static let defaultValue = false
}

/// Places subviews left to right and wraps them like words in a paragraph. Each line is aligned
/// within the width; a subview marked `KzFlowSpaceBefore` is preceded by `spaceWidth`.
struct KzFlowLayout: Layout {
    var spaceWidth: CGFloat
    var lineHeight: CGFloat
    var alignment: TextAlignment

    struct Line {
        var range: Range<Int>
        var width: CGFloat
    }

    private func lines(for subviews: Subviews, maxWidth: CGFloat) -> [Line] {
        var lines: [Line] = []
        var start = 0
        var width: CGFloat = 0
        for index in subviews.indices {
            let size = subviews[index].sizeThatFits(.unspecified)
            let gap = (index > start && subviews[index][KzFlowSpaceBefore.self]) ? spaceWidth : 0
            if index > start, width + gap + size.width > maxWidth {
                lines.append(Line(range: start..<index, width: width))
                start = index
                width = size.width
            } else {
                width += gap + size.width
            }
        }
        if start < subviews.count { lines.append(Line(range: start..<subviews.count, width: width)) }
        return lines
    }

    func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) -> CGSize {
        let maxWidth = proposal.width ?? .infinity
        let lines = lines(for: subviews, maxWidth: maxWidth)
        let width = lines.map(\.width).max() ?? 0
        return CGSize(width: width, height: CGFloat(lines.count) * lineHeight)
    }

    func placeSubviews(in bounds: CGRect, proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) {
        let lines = lines(for: subviews, maxWidth: bounds.width)
        for (row, line) in lines.enumerated() {
            let offset: CGFloat = switch alignment {
            case .center: (bounds.width - line.width) / 2
            case .trailing: bounds.width - line.width
            default: 0
            }
            var x = bounds.minX + offset
            for index in line.range {
                let size = subviews[index].sizeThatFits(.unspecified)
                if index > line.range.lowerBound, subviews[index][KzFlowSpaceBefore.self] { x += spaceWidth }
                subviews[index].place(
                    at: CGPoint(x: x, y: bounds.minY + CGFloat(row) * lineHeight + lineHeight / 2),
                    anchor: .leading, proposal: ProposedViewSize(size)
                )
                x += size.width
            }
        }
    }
}
