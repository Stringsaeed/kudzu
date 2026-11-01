import Foundation

/// One run of chip-editor content: decoded literal text or a single tag chip.
public enum TemplateSegment: Equatable, Sendable {
    case text(String)
    case chip(Tag)

    /// The editor's placeholder for a chip; it occupies exactly one UTF-16 unit.
    public static let chipCharacter: Character = "\u{FFFC}"

    /// Length in editor coordinates, where a chip counts as one UTF-16 unit.
    public var displayLength: Int {
        switch self {
        case .text(let text): text.utf16.count
        case .chip: 1
        }
    }
}

/// Converts between template source and chip-editor segments.
public enum TemplateSegments {
    /// Parses source into segments, merging adjacent text and clearing tag source ranges.
    public static func parse(_ source: String) -> [TemplateSegment] {
        normalized(TemplateParser.parse(source).nodes.map { node -> TemplateSegment in
            switch node {
            case .text(let text, _): return .text(text)
            case .tag(var tag):
                tag.range = 0..<0
                return .chip(tag)
            }
        })
    }

    /// Serializes segments to canonical source, exactly as `TemplateSerializer` would.
    public static func serialize(_ segments: [TemplateSegment]) -> String {
        // Adjacent text must be merged first: "a{" + "{b" escapes differently from "a{{b".
        TemplateSerializer.serialize(normalized(segments).map { segment -> TemplateNode in
            switch segment {
            case .text(let text): .text(text, range: 0..<0)
            case .chip(let tag): .tag(tag)
            }
        })
    }

    /// Interprets pasted plain text: as template source when it contains a valid tag, otherwise verbatim.
    public static func pasted(_ text: String) -> [TemplateSegment] {
        let cleaned = text.replacingOccurrences(of: String(TemplateSegment.chipCharacter), with: "")
        let segments = parse(cleaned)
        guard segments.contains(where: { if case .chip = $0 { true } else { false } }) else {
            return cleaned.isEmpty ? [] : [.text(cleaned)]
        }
        return segments
    }

    /// Merges adjacent text runs and drops empty ones.
    public static func normalized(_ segments: [TemplateSegment]) -> [TemplateSegment] {
        var result = [TemplateSegment]()
        for segment in segments {
            switch segment {
            case .text(let text):
                guard !text.isEmpty else { continue }
                if case .text(let previous)? = result.last {
                    result[result.count - 1] = .text(previous + text)
                } else {
                    result.append(segment)
                }
            case .chip:
                result.append(segment)
            }
        }
        return result
    }

    /// Finds a complete, valid tag typed as literal text that ends at `end` (UTF-16 offset in `text`).
    ///
    /// A tag preceded by a backslash is left alone so users can still type literal tag syntax.
    public static func typedTag(in text: String, endingAt end: Int) -> (range: Range<Int>, tag: Tag)? {
        let units = Array(text.utf16)
        let brace = UInt16(UInt8(ascii: "{")), close = UInt16(UInt8(ascii: "}")), backslash = UInt16(UInt8(ascii: "\\"))
        guard end >= 4, end <= units.count, units[end - 1] == close, units[end - 2] == close else { return nil }
        var start = end - 4
        while start >= 0 {
            if units[start] == brace, units[start + 1] == brace { break }
            if units[start] == UInt16(UInt8(ascii: "\n")) { return nil }
            start -= 1
        }
        guard start >= 0 else { return nil }
        // "{{{date}}" should chip the inner tag, so prefer the rightmost "{{".
        if start > 0, units[start - 1] == backslash { return nil }
        let candidate = parse(String(decoding: units[start..<end], as: UTF16.self))
        guard candidate.count == 1, case .chip(let tag) = candidate[0] else { return nil }
        return (start..<end, tag)
    }
}
