// The editor's text storage is the template source with every chip and image collapsed to one
// attachment character (U+FFFC). The source never changes shape: an atom only *presents* a range
// of it, so serializing the storage gives the bound string back byte for byte. `EditorMap`
// converts between source offsets (UTF-16, as in the TS and the stored JSON) and storage offsets.

import Foundation
import KudzuCore

/// One storage character standing for a range of the source: a template tag chip, or (formatted
/// mode) an embedded image whose source range holds its text alternative.
public struct EditorAtom: Hashable, Sendable {
    public enum Kind: Hashable, Sendable {
        case chip(TagToken)
        case image(dataUrl: String, width: Int)
    }

    public var kind: Kind
    /// UTF-16 range of the source this atom presents.
    public var range: Range<Int>

    public init(kind: Kind, range: Range<Int>) {
        self.kind = kind
        self.range = range
    }

    public var length: Int { range.count }

    public var tag: TagToken? {
        if case .chip(let tag) = kind { return tag }
        return nil
    }
}

public enum EditorAtoms {
    /// Every atom of `source`, sorted. Images (from already validated spans) come first; a tag that
    /// intersects an image stays inside it, like CodeMirror's replacing decorations.
    public static func atoms(source: String, spans: [FormatSpan]?) -> [EditorAtom] {
        var images: [EditorAtom] = []
        for span in (spans ?? []).sorted(by: { $0.start < $1.start }) {
            guard case let .image(dataUrl, width) = span.style, span.start < span.end else { continue }
            if let last = images.last, span.start < last.range.upperBound { continue }
            images.append(EditorAtom(kind: .image(dataUrl: dataUrl, width: Int(width)), range: span.start..<span.end))
        }
        var out = images
        for token in Template.tokenize(source) {
            guard case .tag(let tag) = token else { continue }
            let range = tag.from..<tag.to
            if images.contains(where: { $0.range.overlaps(range) }) { continue }
            out.append(EditorAtom(kind: .chip(tag), range: range))
        }
        return out.sorted { $0.range.lowerBound < $1.range.lowerBound }
    }
}

/// Offsets between the source (UTF-16) and the storage (one character per atom).
public struct EditorMap: Hashable, Sendable {
    /// Where a source offset strictly inside an atom lands.
    public enum Bias: Sendable { case before, after, nearest }

    public let atoms: [EditorAtom]
    public let sourceLength: Int

    public init(atoms: [EditorAtom], sourceLength: Int) {
        self.atoms = atoms
        self.sourceLength = sourceLength
    }

    public init(source: String, spans: [FormatSpan]?) {
        self.init(atoms: EditorAtoms.atoms(source: source, spans: spans), sourceLength: source.utf16.count)
    }

    public var storageLength: Int { sourceLength - atoms.reduce(0) { $0 + $1.length - 1 } }

    /// Storage offset of a source offset; positions inside an atom snap to one of its edges.
    public func storageOffset(_ source: Int, bias: Bias = .before) -> Int {
        let source = min(max(0, source), sourceLength)
        var shift = 0
        for atom in atoms {
            if source <= atom.range.lowerBound { break }
            if source >= atom.range.upperBound { shift += atom.length - 1; continue }
            let start = atom.range.lowerBound - shift
            switch bias {
            case .before: return start
            case .after: return start + 1
            case .nearest: return source - atom.range.lowerBound < atom.range.upperBound - source ? start : start + 1
            }
        }
        return source - shift
    }

    /// Source offset of a storage offset (always an atom edge).
    public func sourceOffset(_ storage: Int) -> Int {
        var shift = 0
        for atom in atoms {
            if storage <= atom.range.lowerBound - shift { break }
            shift += atom.length - 1
        }
        return min(storage + shift, sourceLength)
    }

    public func sourceRange(_ storage: Range<Int>) -> Range<Int> {
        sourceOffset(storage.lowerBound)..<max(sourceOffset(storage.lowerBound), sourceOffset(storage.upperBound))
    }

    /// Storage range covering a source range; partial atoms are included whole. A caret inside an
    /// atom snaps to its nearest edge.
    public func storageRange(_ source: Range<Int>) -> Range<Int> {
        if source.isEmpty {
            let at = storageOffset(source.lowerBound, bias: .nearest)
            return at..<at
        }
        let from = storageOffset(source.lowerBound, bias: .before)
        return from..<max(from, storageOffset(source.upperBound, bias: .after))
    }

    /// Source range snapped outward to atom edges (selection never splits a chip).
    public func snapped(_ source: Range<Int>) -> Range<Int> { sourceRange(storageRange(source)) }

    /// The atom drawn at a storage index.
    public func atom(atStorage index: Int) -> EditorAtom? {
        var shift = 0
        for atom in atoms {
            let at = atom.range.lowerBound - shift
            if at == index { return atom }
            if at > index { return nil }
            shift += atom.length - 1
        }
        return nil
    }

    /// Storage index of an atom.
    public func storageIndex(of atom: EditorAtom) -> Int? {
        var shift = 0
        for candidate in atoms {
            if candidate == atom { return candidate.range.lowerBound - shift }
            shift += candidate.length - 1
        }
        return nil
    }
}
