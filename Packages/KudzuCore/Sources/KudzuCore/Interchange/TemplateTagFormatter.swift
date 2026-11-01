import Foundation

/// Builds template tag source for the Espanso importer.
///
/// Tags are assembled as `Tag` values and written by `TemplateSerializer`, so quoting and escaping always match
/// what `TemplateParser` reads back.
enum TemplateTagFormatter {
    /// Builds a `{{date ...}}` tag.
    static func dateTag(format: String?, offset: String? = nil, locale: String? = nil) -> String {
        stringTag("date", [("format", format), ("offset", offset), ("locale", locale)])
    }

    /// Builds a `{{clipboard}}` tag.
    static func clipboardTag(trim: Bool = false) -> String {
        Tag(name: "clipboard", attributes: trim ? [Attribute(name: "trim", value: .bool(true))] : []).source
    }

    /// Builds a `{{cursor}}` tag.
    static var cursorTag: String {
        Tag(name: "cursor").source
    }

    /// Builds a `{{snippet id="..."}}` tag.
    static func snippetTag(id: UUID) -> String {
        Tag(name: "snippet", attributes: [Attribute(name: "id", value: .string(id.uuidString))]).source
    }

    /// Builds a `{{field ...}}` tag.
    static func fieldTag(name: String, defaultValue: String? = nil, multiline: Bool = false) -> String {
        var tag = Tag(name: "field", attributes: [Attribute(name: "name", value: .string(name))])
        if let defaultValue, !defaultValue.isEmpty { tag.attributes.append(Attribute(name: "default", value: .string(defaultValue))) }
        if multiline { tag.attributes.append(Attribute(name: "multiline", value: .bool(true))) }
        return tag.source
    }

    /// Builds a `{{choice ...}}` tag; a `|` inside an option is written as `\|`.
    static func choiceTag(name: String, options: [String], defaultValue: String? = nil) -> String {
        var tag = Tag(name: "choice", attributes: [
            Attribute(name: "name", value: .string(name)),
            Attribute(name: "options", value: .string(ChoiceOptions.encode(options))),
        ])
        if let defaultValue, !defaultValue.isEmpty { tag.attributes.append(Attribute(name: "default", value: .string(defaultValue))) }
        return tag.source
    }

    /// A tag whose attributes are all strings; nil and empty values are omitted.
    private static func stringTag(_ name: String, _ attributes: [(String, String?)]) -> String {
        Tag(name: name, attributes: attributes.compactMap { key, value in
            value.flatMap { $0.isEmpty ? nil : Attribute(name: key, value: .string($0)) }
        }).source
    }
}
