import Foundation

/// A key the search field saw, before it is turned into a ``PaletteKey``.
enum PaletteInput: Equatable, Sendable {
    case down, up, pageDown, pageUp, escape, tab, `return`
    case character(String)
}

/// The `onKeyDown` switch in `palette.tsx`. Character keys the field should insert return nil.
enum PaletteKeys {
    static func interpret(_ input: PaletteInput, command: Bool, control: Bool, shift: Bool) -> PaletteKey? {
        switch input {
        case .down: return .down
        case .up: return .up
        case .pageDown: return .pageDown
        case .pageUp: return .pageUp
        case .escape: return .escape
        case .tab: return .tab(shift: shift)
        case .return: return .enter(command: command)
        case let .character(raw):
            let key = raw.lowercased()
            // Control-N / Control-P are checked before ⌘N, matching the web's if/else order.
            if control, key == "n" { return .down }
            if control, key == "p" { return .up }
            if command, key == "e" { return .edit }
            if command, key == "n" { return .new }
            if command, key.count == 1, let number = Int(key), (1...9).contains(number) { return .number(number) }
            return nil
        }
    }
}
