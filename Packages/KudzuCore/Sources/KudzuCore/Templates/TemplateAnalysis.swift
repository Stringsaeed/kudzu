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

    /// Creates the definition of a form control.
    public init(name: String, kind: Kind, options: [String] = [], defaultValue: String? = nil) {
        self.name = name
        self.kind = kind
        self.options = options
        self.defaultValue = defaultValue
    }
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
    /// Collects unique form fields across nested snippets in first-appearance order.
    public static func formFields(
        in source: String,
        rootSnippetID: UUID? = nil,
        resolveSnippet: (UUID) -> String?
    ) -> [FormFieldSpec] {
        var fields: [FormFieldSpec] = []
        var names = Set<String>()
        func visit(_ source: String, path: Set<UUID>, depth: Int) {
            for node in TemplateParser.parse(source).nodes {
                guard case .tag(let tag) = node else { continue }
                let typed = tag.typed
                switch typed {
                case .field, .choice:
                    if let spec = fieldSpec(for: typed), names.insert(spec.name).inserted {
                        fields.append(spec)
                    }
                case .snippet(let id):
                    if depth < 8, !path.contains(id), let nested = resolveSnippet(id) {
                        visit(nested, path: path.union([id]), depth: depth + 1)
                    }
                default: break
                }
            }
        }
        visit(source, path: rootSnippetID.map { [$0] } ?? [], depth: 0)
        return fields
    }

    /// Returns whether `source` or any reachable nested snippet up to the recursion limit uses `{{clipboard}}`.
    public static func usesClipboard(
        in source: String,
        rootSnippetID: UUID? = nil,
        resolveSnippet: (UUID) -> String?
    ) -> Bool {
        func visit(_ source: String, path: Set<UUID>, depth: Int) -> Bool {
            for node in TemplateParser.parse(source).nodes {
                guard case .tag(let tag) = node else { continue }
                let typed = tag.typed
                switch typed {
                case .clipboard:
                    return true
                case .snippet(let id):
                    if depth < 8, !path.contains(id), let nested = resolveSnippet(id),
                       visit(nested, path: path.union([id]), depth: depth + 1) {
                        return true
                    }
                default:
                    break
                }
            }
            return false
        }
        return visit(source, path: rootSnippetID.map { [$0] } ?? [], depth: 0)
    }

    /// Collects form-control diagnostics (conflicting definitions for the same field name or empty dropdown options)
    /// across `source` and any reachable nested snippets.
    public static func formDiagnostics(
        in source: String,
        rootSnippetID: UUID? = nil,
        resolveSnippet: (UUID) -> String?
    ) -> [TemplateDiagnostic] {
        var diagnostics: [TemplateDiagnostic] = []
        var seen: [String: FormFieldSpec.Kind] = [:]
        func kindLabel(_ kind: FormFieldSpec.Kind) -> String {
            switch kind {
            case .text: "text field"
            case .multiline: "multi-line field"
            case .choice: "dropdown"
            }
        }
        func visit(_ source: String, path: Set<UUID>, depth: Int) {
            for node in TemplateParser.parse(source).nodes {
                guard case .tag(let tag) = node else { continue }
                let typed = tag.typed
                switch typed {
                case .field, .choice:
                    guard let spec = fieldSpec(for: typed) else { continue }
                    if let first = seen[spec.name] {
                        if first != spec.kind {
                            diagnostics.append(.init(
                                severity: .warning,
                                message: "'\(spec.name)' is already a \(kindLabel(first)); the first definition wins",
                                range: tag.range
                            ))
                        }
                    } else {
                        seen[spec.name] = spec.kind
                    }
                    if spec.kind == .choice, spec.options.isEmpty {
                        diagnostics.append(.init(
                            severity: .warning,
                            message: "Dropdown '\(spec.name)' has no options",
                            range: tag.range
                        ))
                    }
                case .snippet(let id):
                    if depth < 8, !path.contains(id), let nested = resolveSnippet(id) {
                        visit(nested, path: path.union([id]), depth: depth + 1)
                    }
                default:
                    break
                }
            }
        }
        visit(source, path: rootSnippetID.map { [$0] } ?? [], depth: 0)
        return diagnostics
    }

    private static func fieldSpec(for typed: TemplateTag) -> FormFieldSpec? {
        switch typed {
        case .field(let name, let value, let multiline):
            return .init(name: name, kind: multiline ? .multiline : .text, options: [], defaultValue: value)
        case .choice(let name, let options, let value):
            return .init(name: name, kind: .choice, options: options, defaultValue: value)
        default:
            return nil
        }
    }

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
