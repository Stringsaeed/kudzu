// Icons are SF Symbols (`Icon.<name>`, see `LucideMap.swift`). Sizing follows the web's box sizes:
// a Lucide `size-4` icon is a 16 pt box whose 2-unit stroke glyph fills about 20/24 of it, so a
// symbol drawn at 13 pt in a 16 pt frame reads the same (`.icon(16)`).

import SwiftUI

/// SF Symbol equivalents for the web app’s icon vocabulary.
public enum Icon {
    /// The symbol point size that fills a Lucide box of `box` points like the web glyph.
    public static func symbolSize(forBox box: CGFloat) -> CGFloat { (box * 13 / 16).rounded(.toNearestOrEven) }
}

extension EnvironmentValues {
    /// The default icon box of a container (`[&_svg:not([class*='size-'])]:size-*`), in points.
    @Entry public var kzIconSize: CGFloat = 16
}

extension View {
    /// Sets the icon box for `.icon()` images inside this view.
    public func iconSize(_ points: CGFloat) -> some View { environment(\.kzIconSize, points) }

    /// `animate-spin`: one turn per second, linear, forever. Still under Reduce Motion.
    /// (For progress, prefer `ProgressView().controlSize(.small)`.)
    public func spin() -> some View { modifier(Spin()) }
}

extension Image {
    /// Draws a symbol in a Lucide-sized box: `size` points (`size-3` = 12, `size-3.5` = 14,
    /// `size-4` = 16), or the container's `iconSize` when nil.
    public func icon(_ size: CGFloat? = nil) -> some View {
        IconBox(image: self, size: size)
    }
}

private struct IconBox: View {
    var image: Image
    var size: CGFloat?
    @Environment(\.kzIconSize) private var inherited

    var body: some View {
        let box = size ?? inherited
        image
            .font(.system(size: Icon.symbolSize(forBox: box)))
            .imageScale(.medium)
            .frame(width: box, height: box)
            .accessibilityHidden(true)
    }
}

private struct Spin: ViewModifier {
    @State private var turned = false
    @Environment(\.accessibilityReduceMotion) private var reduce

    func body(content: Content) -> some View {
        content
            .rotationEffect(.degrees(turned && !reduce ? 360 : 0))
            .onAppear {
                guard !reduce else { return }
                withAnimation(.linear(duration: 1).repeatForever(autoreverses: false)) { turned = true }
            }
    }
}
