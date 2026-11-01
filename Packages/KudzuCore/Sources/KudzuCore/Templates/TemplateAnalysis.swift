import Foundation

/// The form control requested by a template.
public struct FormFieldSpec: Equatable, Sendable {
    /// Supported form control kinds.
    public enum Kind: String, Sendable { case text, multiline, choice }
    /// Shared form-value key.
    public var name: String
    /// Control presentation.
    public var kind: Kind
    /// Ordered dropdown items, empty for text controls.
    public var options: [String]
    /// Initial value when no supplied form value exists.
    public var defaultValue: String?
}

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
    /// Analyzes recognized tags; the first definition of each form name wins.
    public static func analyze(_ parsed: ParsedTemplate) -> TemplateInfo {
        var result = TemplateInfo(formFields: [], referencedSnippetIDs: [], hasCursor: false, usesClipboard: false)
        var names = Set<String>(), ids = Set<UUID>()
        for case .tag(let tag) in parsed.nodes {
            switch tag.typed {
            case .field(let name, let value, let multiline):
                if names.insert(name).inserted { result.formFields.append(.init(name: name, kind: multiline ? .multiline : .text, options: [], defaultValue: value)) }
            case .choice(let name, let options, let value):
                if names.insert(name).inserted { result.formFields.append(.init(name: name, kind: .choice, options: options, defaultValue: value)) }
            case .snippet(let id): if ids.insert(id).inserted { result.referencedSnippetIDs.append(id) }
            case .clipboard: result.usesClipboard = true
            case .cursor: result.hasCursor = true
            default: break
            }
        }
        return result
    }
}
