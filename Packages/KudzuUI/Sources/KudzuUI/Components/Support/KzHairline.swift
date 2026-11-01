import SwiftUI

/// A 1px `line` hairline. Horizontal by default; vertical when `axis` is `.vertical`.
struct KzHairline: View {
    var axis: Axis = .horizontal
    var color: Color = KzColor.line

    var body: some View {
        switch axis {
        case .horizontal: color.frame(height: 1)
        case .vertical: color.frame(width: 1)
        }
    }
}
