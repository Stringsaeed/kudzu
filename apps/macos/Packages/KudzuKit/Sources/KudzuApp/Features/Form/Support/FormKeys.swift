import Foundation
import KudzuCore

/// A key the form dialog handles itself. Tab stays with the focus system.
enum FormKey: Equatable, Sendable {
    case escape
    /// Return. `command` is ⌘Return, which submits even inside a multi-line field.
    case `return`(command: Bool, control: Bool = false)
}

/// What that key does given the field that has focus (`form-panel.tsx`'s `onKeyDown`).
enum FormKeyEffect: Equatable, Sendable {
    case submit
    case cancel
    /// Plain Return in a multi-line field inserts a newline.
    case newline
}

enum FormKeys {
    static func effect(_ key: FormKey, field: FormFieldKind?) -> FormKeyEffect {
        switch key {
        case .escape: return .cancel
        case let .return(command, control):
            if field == .multiline, !command, !control { return .newline }
            return .submit
        }
    }
}
