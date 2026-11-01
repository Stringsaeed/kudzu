import SwiftUI

/// The content editor's frame: an Insert slot and a hint bar on top, and the content area below at
/// the `content` style, inside a `radius-lg` card with a `line` border.
///
/// This is chrome only. The content slot hosts the app's `NSTextView` chip editor (see
/// `KzEditorStyle` for the values it needs). Focus is reported by the text view, not SwiftUI, so
/// pass `isFocused` from the editor to get the 2px `vine-ink` edge.
public struct KzEditorFrame<Insert: View, Content: View>: View {
    private let minHeight: CGFloat
    private let isFocused: Bool?
    private let insert: Insert
    private let content: Content

    /// Creates the frame.
    /// - Parameters:
    ///   - minHeight: Minimum height of the content area; 140 by default.
    ///   - isFocused: Whether the editor has keyboard focus (draws the `vine-ink` edge).
    ///   - insert: The Insert menu button (a ghost small `KzButton` with a chevron).
    ///   - content: The editor view.
    public init(minHeight: CGFloat = KzEditorStyle.minHeight, isFocused: Bool? = nil, @ViewBuilder insert: () -> Insert, @ViewBuilder content: () -> Content) {
        self.minHeight = minHeight
        self.isFocused = isFocused
        self.insert = insert()
        self.content = content()
    }

    public var body: some View {
        let shape = RoundedRectangle(cornerRadius: KzRadius.lg)
        VStack(spacing: 0) {
            HStack(spacing: KzSpace.s2) {
                insert
                Spacer(minLength: 0)
                hint
            }
            .padding(.leading, 6)
            .padding(.trailing, KzSpace.s2)
            .frame(height: 36)
            KzHairline()
            content
                .frame(maxWidth: .infinity, minHeight: minHeight, alignment: .topLeading)
        }
        .background(KzColor.surfaceRaised, in: shape)
        .clipShape(shape)
        .overlay(shape.strokeBorder(KzColor.line))
        .kzFocusRing(shape, style: .edge, isFocused: isFocused)
    }

    private var hint: some View {
        HStack(spacing: KzSpace.s1) {
            Text("Type")
            KzKbd("/")
            Text("to insert a tag")
        }
        .kzTextStyle(.callout, matchLineBox: false)
        .foregroundStyle(KzColor.inkMuted)
        .accessibilityElement(children: .ignore)
        .accessibilityLabel("Type slash to insert a tag")
    }
}
