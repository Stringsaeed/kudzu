import Foundation

/// Static facts about a single template source.
public struct TemplateInfo: Equatable, Sendable {
    /// Form controls in first-appearance order, deduplicated by name.
    public var formFields: [FormFieldSpec]
    /// Whether this source requests a form.
    public var isForm: Bool { !formFields.isEmpty }
    /// Referenced IDs in first-appearance order, deduplicated.
    public var referencedSnippetIDs: [UUID]
    /// Whether this source contains a cursor tag.
    public var hasCursor: Bool
    /// Whether this source reads the clipboard.
    public var usesClipboard: Bool
}

/// Extracts editor and form metadata without resolving nested snippets.
public enum TemplateAnalysis {
    /// Resolves fields depth first, with the renderer's path guard and eight-level limit.
    public static func formFields(in source: String, resolveSnippet: (UUID) -> String?) -> [FormFieldSpec] {
        var fields: [FormFieldSpec] = []
        // Byte keys, like Rust's `HashSet<String>`: `é` and `e` + U+0301 are two fields.
        var names = Set<[UInt8]>()
        func visit(_ source: String, path: Set<UUID>, depth: Int) {
            for case .tag(let tag) in TemplateParser.parse(source).nodes {
                if case .snippet(let id) = tag.typed {
                    if depth < 8, !path.contains(id), let nested = resolveSnippet(id) {
                        visit(nested, path: path.union([id]), depth: depth + 1)
                    }
                } else {
                    for field in analyze(ParsedTemplate(nodes: [.tag(tag)], diagnostics: [])).formFields {
                        if names.insert(field.name.exactKey).inserted { fields.append(field) }
                    }
                }
            }
        }
        visit(source, path: [], depth: 0)
        return fields
    }

    /// Analyzes recognized tags; the first definition of each form name wins.
    public static func analyze(_ parsed: ParsedTemplate) -> TemplateInfo {
        var result = TemplateInfo(formFields: [], referencedSnippetIDs: [], hasCursor: false, usesClipboard: false)
        var names = Set<[UInt8]>(), ids = Set<UUID>()
        for case .tag(let tag) in parsed.nodes {
            switch tag.typed {
            case .field(let name, let value, let multiline):
                if names.insert(name.exactKey).inserted { result.formFields.append(.init(name: name, kind: multiline ? .multiline : .text, options: [], defaultValue: value)) }
            case .choice(let name, let options, let value):
                if names.insert(name.exactKey).inserted { result.formFields.append(.init(name: name, kind: .choice, options: options, defaultValue: value)) }
            case .snippet(let id): if ids.insert(id).inserted { result.referencedSnippetIDs.append(id) }
            case .clipboard: result.usesClipboard = true
            case .cursor: result.hasCursor = true
            default: break
            }
        }
        return result
    }
}
