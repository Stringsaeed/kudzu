import Foundation

/// A diagnostic located both in editor coordinates (chips are one unit) and in canonical source.
public struct EditorDiagnostic: Equatable, Sendable {
    /// Diagnostic importance.
    public var severity: TemplateDiagnostic.Severity
    /// Human-readable explanation.
    public var message: String
    /// Half-open UTF-16 range in editor content, where each chip is one unit.
    public var displayRange: Range<Int>
    /// Half-open UTF-16 range in `TemplateSegments.serialize(segments)`.
    public var sourceRange: Range<Int>
    /// Whether the diagnostic points at a single chip rather than literal text.
    public var isChip: Bool

    /// The diagnostic expressed against canonical source.
    public var sourceDiagnostic: TemplateDiagnostic {
        TemplateDiagnostic(severity: severity, message: message, range: sourceRange)
    }
}

/// Finds problems the chip editor should surface while the user edits.
public enum TemplateEditorLint {
    /// Lints literal text that looks like a broken tag, plus chip-level issues.
    ///
    /// Literal text is checked as typed, so an unknown `{{tag}}` typed by hand keeps its underline even though
    /// canonical serialization escapes it. Unrecognized tags loaded from source are chips and are reported per chip. `knownSnippetIDs` (when given) flags missing references.
    public static func diagnostics(for segments: [TemplateSegment], knownSnippetIDs: Set<UUID>? = nil,
                                   currentSnippetID: UUID? = nil) -> [EditorDiagnostic] {
        var result = [EditorDiagnostic]()
        var display = 0, source = 0
        var seenCursor = false
        var formKinds = [String: FormFieldSpec.Kind]()
        for segment in TemplateSegments.normalized(segments) {
            let sourceText = TemplateSegments.serialize([segment])
            switch segment {
            case .text(let text):
                let parsed = TemplateParser.parse(text)
                let tagRanges = Set(parsed.nodes.compactMap { node -> Range<Int>? in
                    if case .tag(let tag) = node, tag.verbatim == nil { tag.range } else { nil }
                })
                for diagnostic in parsed.diagnostics where !tagRanges.contains(diagnostic.range) {
                    let lower = escapedLength(text, upTo: diagnostic.range.lowerBound)
                    let upper = escapedLength(text, upTo: diagnostic.range.upperBound)
                    result.append(EditorDiagnostic(
                        severity: diagnostic.severity, message: diagnostic.message,
                        displayRange: (display + diagnostic.range.lowerBound)..<(display + diagnostic.range.upperBound),
                        sourceRange: (source + lower)..<(source + max(lower, upper)), isChip: false))
                }
            case .chip(let tag):
                var messages = [String]()
                switch tag.typed {
                case .cursor:
                    if seenCursor { messages.append("Only the first cursor counts") }
                    seenCursor = true
                case .date, .time:
                    if let offset = tag.stringAttribute("offset"), DateOffset(offset) == nil {
                        messages.append("Invalid date offset; ignored")
                    }
                case .field(let name, _, let multiline):
                    if let message = conflict(name, multiline ? .multiline : .text, &formKinds) { messages.append(message) }
                case .choice(let name, let options, _):
                    if let message = conflict(name, .choice, &formKinds) { messages.append(message) }
                    if options.isEmpty { messages.append("Dropdown '\(name)' has no options") }
                case .snippet(let id):
                    if id == currentSnippetID { messages.append("A snippet can't include itself") }
                    else if let knownSnippetIDs, !knownSnippetIDs.contains(id) { messages.append("Missing snippet") }
                case .clipboard:
                    break
                case .unknown:
                    if let verbatim = tag.verbatim, let problem = TemplateParser.parse(verbatim).diagnostics.first {
                        messages.append(problem.message)
                    }
                }
                for message in messages {
                    result.append(EditorDiagnostic(
                        severity: .warning, message: message, displayRange: display..<(display + 1),
                        sourceRange: source..<(source + sourceText.utf16.count), isChip: true))
                }
            }
            display += segment.displayLength
            source += sourceText.utf16.count
        }
        return result
    }

    /// Whether `name` is already used by a form control of a different kind elsewhere in `segments`.
    public static func nameConflicts(_ name: String, kind: FormFieldSpec.Kind, in segments: [TemplateSegment],
                                     excludingChipAt excluded: Int? = nil) -> Bool {
        for (index, segment) in segments.enumerated() where index != excluded {
            guard case .chip(let tag) = segment else { continue }
            switch tag.typed {
            case .field(let other, _, let multiline) where other == name:
                if (multiline ? FormFieldSpec.Kind.multiline : .text) != kind { return true }
            case .choice(let other, _, _) where other == name:
                if kind != .choice { return true }
            default:
                continue
            }
        }
        return false
    }

    private static func conflict(_ name: String, _ kind: FormFieldSpec.Kind,
                                 _ kinds: inout [String: FormFieldSpec.Kind]) -> String? {
        if let existing = kinds[name] {
            return existing == kind ? nil : "'\(name)' is already a \(describe(existing)); the first definition wins"
        }
        kinds[name] = kind
        return nil
    }

    private static func describe(_ kind: FormFieldSpec.Kind) -> String {
        switch kind {
        case .text: "text field"
        case .multiline: "multiline field"
        case .choice: "dropdown"
        }
    }

    /// Escaped source length of the first `offset` UTF-16 units of literal `text`.
    private static func escapedLength(_ text: String, upTo offset: Int) -> Int {
        let units = text.utf16
        let end = units.index(units.startIndex, offsetBy: min(offset, units.count))
        let prefix = String(text[..<(end.samePosition(in: text) ?? text.endIndex)])
        return TemplateSegments.serialize([.text(prefix)]).utf16.count
    }
}
