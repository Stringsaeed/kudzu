import AppKit
import KudzuCore
import KudzuUI

/// Builds editor attributed content from segments and reads segments back out of text storage.
@MainActor
struct ChipFactory {
    /// Snippet names by ID, for snippet chip labels.
    var snippetNames: [UUID: String] = [:]

    /// Monospace attributes for literal text loaded, typed or pasted into the editor.
    var textAttributes: [NSAttributedString.Key: Any] { TemplateTextStyle.attributes() }

    /// The presentation of `tag` given the current snippet names.
    func label(for tag: KudzuCore.Tag) -> ChipLabel {
        ChipLabel(tag: tag) { snippetNames[$0] }
    }

    /// A one-character attributed string holding a chip for `tag`.
    func chip(_ tag: KudzuCore.Tag) -> NSAttributedString {
        let attachment = ChipAttachment(tag: tag, label: label(for: tag))
        let result = NSMutableAttributedString(attachment: attachment)
        result.addAttributes(textAttributes, range: NSRange(location: 0, length: result.length))
        return result
    }

    /// Editor content for `segments`.
    func attributedString(for segments: [TemplateSegment]) -> NSAttributedString {
        let result = NSMutableAttributedString()
        for segment in segments {
            switch segment {
            case .text(let text): result.append(NSAttributedString(string: text, attributes: textAttributes))
            case .chip(let tag): result.append(chip(tag))
            }
        }
        return result
    }

    /// Segments for `range` of editor content. Stray non-chip attachments are dropped.
    static func segments(in content: NSAttributedString, range: NSRange? = nil) -> [TemplateSegment] {
        let range = range ?? NSRange(location: 0, length: content.length)
        let string = content.string as NSString
        let placeholder = String(TemplateSegment.chipCharacter)
        var result = [TemplateSegment]()
        content.enumerateAttribute(.attachment, in: range) { value, run, _ in
            if value is ChipAttachment {
                // Each attachment character is its own chip; re-read per index in case a run repeats one.
                for index in run.location..<NSMaxRange(run) {
                    if let chip = Self.chip(in: content, at: index) { result.append(.chip(chip.tag)) }
                }
            } else {
                let text = string.substring(with: run).replacingOccurrences(of: placeholder, with: "")
                if !text.isEmpty { result.append(.text(text)) }
            }
        }
        return TemplateSegments.normalized(result)
    }

    /// Visits every chip in `content` with its character index.
    static func forEachChip(in content: NSAttributedString, _ body: (ChipAttachment, Int) -> Void) {
        let length = content.length
        content.enumerateAttribute(.attachment, in: NSRange(location: 0, length: length)) { _, run, _ in
            for index in run.location..<NSMaxRange(run) {
                if let chip = content.attribute(.attachment, at: index, effectiveRange: nil) as? ChipAttachment {
                    body(chip, index)
                }
            }
        }
    }

    /// The chip at `index`, if that character is one.
    static func chip(in content: NSAttributedString, at index: Int) -> ChipAttachment? {
        guard index >= 0, index < content.length else { return nil }
        return content.attribute(.attachment, at: index, effectiveRange: nil) as? ChipAttachment
    }
}
