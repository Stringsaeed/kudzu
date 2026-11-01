import SwiftUI

/// A pane's header row: `size-titlebar` (52pt) tall, content vertically centred, so it lines up with
/// the traffic lights. `KzListHeader` is this row for the list pane; use `KzPaneHeader` for the
/// sidebar and detail panes.
public struct KzPaneHeader<Content: View>: View {
    private let leading: CGFloat
    private let trailing: CGFloat
    private let content: Content

    /// Creates a header row.
    /// - Parameters:
    ///   - leading: Leading padding; 24 for the detail pane (`space-6`).
    ///   - trailing: Trailing padding; 12 (`space-3`).
    ///   - content: Laid out in a row, `space-2` apart.
    public init(leading: CGFloat = KzSpace.s6, trailing: CGFloat = KzSpace.s3, @ViewBuilder content: () -> Content) {
        self.leading = leading
        self.trailing = trailing
        self.content = content()
    }

    public var body: some View {
        HStack(spacing: KzSpace.s2) { content }
            .padding(.leading, leading)
            .padding(.trailing, trailing)
            .frame(maxWidth: .infinity)
            .frame(height: KzSize.titlebar)
            .kzWindowDragArea()
    }
}
