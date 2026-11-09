// A 1-pt rule in a token color: `border-hairline` / `divide-hairline`, or `<Separator>` (`bg-border`).
// `Divider` draws the system separator color, and tinting it with an overlay stacks a second color on
// top, so the line comes out heavier than the web's.

import SwiftUI

/// A horizontal or vertical 1-pt line.
public struct Hairline: View {
    var axis: Axis
    var token: KZToken

    /// Creates Hairline with the supplied content and styling.
    public init(_ axis: Axis = .horizontal, token: KZToken = .hairline) {
        self.axis = axis
        self.token = token
    }

    /// The composed native view.
    public var body: some View {
        Rectangle()
            .fill(.token(token))
            .frame(width: axis == .vertical ? 1 : nil, height: axis == .horizontal ? 1 : nil)
            .accessibilityHidden(true)
    }
}
