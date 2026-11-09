import Foundation

/// One slice of a palette title (`Highlight` in `palette.tsx`). Hit indices are UTF-16 units into
/// the name, which is what `SearchHit.nameHits` records.
struct HighlightRun: Equatable, Sendable {
    var text: String
    var hit: Bool
}

enum PaletteHighlight {
    static func runs(_ text: String, hits: [Int]) -> [HighlightRun] {
        guard !text.isEmpty else { return [] }
        guard !hits.isEmpty else { return [HighlightRun(text: text, hit: false)] }
        let marked = Set(hits)
        var index = 0
        var runs: [HighlightRun] = []
        for character in text {
            let count = character.utf16.count
            let hit = (0..<count).contains { marked.contains(index + $0) }
            if let last = runs.last, last.hit == hit {
                runs[runs.count - 1].text.append(character)
            } else {
                runs.append(HighlightRun(text: String(character), hit: hit))
            }
            index += count
        }
        return runs
    }
}
