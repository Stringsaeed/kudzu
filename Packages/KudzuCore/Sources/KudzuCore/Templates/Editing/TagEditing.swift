import Foundation

extension Tag {
    /// The string value of an attribute, or nil when absent or not a string.
    public func stringAttribute(_ name: String) -> String? { string(name) }

    /// The boolean value of an attribute, false when absent or not a boolean.
    public func boolAttribute(_ name: String) -> Bool { bool(name) }

    /// Returns a copy with `name` set in place (or appended); nil removes it. Unknown attributes are kept.
    public func setting(_ name: String, to value: AttributeValue?) -> Tag {
        var copy = self
        if let value {
            if let index = copy.attributes.lastIndex(where: { $0.name == name }) {
                copy.attributes[index].value = value
            } else {
                copy.attributes.append(Attribute(name: name, value: value))
            }
        } else {
            copy.attributes.removeAll { $0.name == name }
        }
        return copy
    }

    /// Sets a string attribute, removing it when the value is nil or empty.
    public func setting(_ name: String, string value: String?) -> Tag {
        setting(name, to: value.flatMap { $0.isEmpty ? nil : .string($0) })
    }

    /// Sets a boolean attribute, removing it when false so canonical source stays minimal.
    public func setting(_ name: String, flag value: Bool) -> Tag {
        setting(name, to: value ? .bool(true) : nil)
    }

    /// Canonical source for this tag alone.
    public var source: String { TemplateSerializer.serialize([.tag(self)]) }
}

/// Encodes and decodes the pipe-separated `options` attribute of `choice` tags.
public enum ChoiceOptions {
    /// Joins options with `|`, escaping literal pipes as `\|`.
    public static func encode(_ options: [String]) -> String {
        options.map { $0.replacingOccurrences(of: "|", with: "\\|") }.joined(separator: "|")
    }

    /// Splits an `options` value exactly as the renderer does.
    public static func decode(_ value: String) -> [String] {
        guard case .choice(_, let options, _) = Tag(name: "choice", attributes: [
            Attribute(name: "name", value: .string("x")), Attribute(name: "options", value: .string(value)),
        ]).typed else { return [] }
        return options
    }
}

/// A named ICU pattern offered in the date/time chip editor.
public struct DateFormatPreset: Equatable, Sendable, Identifiable {
    /// Stable identifier.
    public var id: String
    /// Menu title.
    public var title: String
    /// ICU date format pattern.
    public var format: String
    /// Short chip text; the pattern itself when it already reads well.
    public var chipText: String

    /// Presets in menu order.
    public static let all: [DateFormatPreset] = [
        .init(id: "iso", title: "ISO date", format: "yyyy-MM-dd", chipText: "yyyy-MM-dd"),
        .init(id: "long", title: "Long date", format: "MMMM d, yyyy", chipText: "Long date"),
        .init(id: "short", title: "Short date", format: "M/d/yy", chipText: "Short date"),
        .init(id: "weekday", title: "Weekday", format: "EEEE", chipText: "Weekday"),
        .init(id: "time24", title: "24-hour time", format: "HH:mm", chipText: "HH:mm"),
        .init(id: "time12", title: "Short time", format: "h:mm a", chipText: "Short time"),
        .init(id: "datetime", title: "ISO date and time", format: "yyyy-MM-dd HH:mm", chipText: "yyyy-MM-dd HH:mm"),
    ]

    /// The preset with exactly this pattern, if any.
    public static func matching(_ format: String) -> DateFormatPreset? {
        all.first { $0.format == format }
    }

    /// The format a `date` or `time` tag uses when it has no `format` attribute.
    public static func defaultFormat(forTagNamed name: String) -> String {
        name == "time" ? "HH:mm" : "yyyy-MM-dd"
    }
}
