import SwiftUI

/// A keycap for shortcuts in hints, menus and empty states. Pass Mac glyphs (`⌘N`, `⌘,`, `/`), never "Cmd".
public struct KzKbd: View {
    private let keys: String

    /// Creates a keycap showing `keys`.
    public init(_ keys: String) { self.keys = keys }

    public var body: some View {
        let shape = RoundedRectangle(cornerRadius: KzRadius.xs)
        Text(keys)
            .kzTextStyle(.monoCaption, matchLineBox: false)
            .fontWeight(.medium)
            .foregroundStyle(KzColor.inkMuted)
            .padding(.horizontal, 5)
            .frame(minWidth: 18, minHeight: 18, maxHeight: 18)
            .background(KzColor.chip, in: shape)
            .overlay(shape.strokeBorder(KzColor.lineStrong))
            .fixedSize()
            .accessibilityLabel(Self.spokenName(keys))
    }

    /// Spells modifier glyphs out for VoiceOver ("Command N").
    static func spokenName(_ keys: String) -> String {
        let names: [Character: String] = ["⌘": "Command", "⌥": "Option", "⇧": "Shift", "⌃": "Control",
                                          "↩": "Return", "⌫": "Delete", "⎋": "Escape", "⇥": "Tab"]
        return keys.map { names[$0] ?? String($0) }.joined(separator: " ")
    }
}
