// `html.theme-switching`: "Flip instantly: without this, every colour transition animates and text
// washes out mid-switch." The web adds the class, flips `.dark`, and removes the class two frames
// later. `SelectionPill` is exempt (it only animates position and size, so the Appearance control
// can still slide).

import AppKit
import SwiftUI

@MainActor
@Observable
/// Coordinates instant appearance changes across animated views.
public final class ThemeSwitch {
    /// The shared presentation coordinator.
    public static let shared = ThemeSwitch()

    /// True for the two frames around a theme flip; colour transitions should not animate.
    public private(set) var isSwitching = false

    private var generation = 0

    /// Runs `change` (e.g. setting `NSApp.appearance`) with implicit animations disabled, then keeps
    /// `isSwitching` set for two more frames, like `requestAnimationFrame(() => requestAnimationFrame(…))`.
    public func flip(_ change: () -> Void) {
        isSwitching = true
        generation += 1
        let current = generation
        var transaction = Transaction()
        transaction.disablesAnimations = true
        withTransaction(transaction, change)
        // Two display frames at 60 Hz.
        DispatchQueue.main.asyncAfter(deadline: .now() + 2.0 / 60.0) { [weak self] in
            MainActor.assumeIsolated {
                guard let self, self.generation == current else { return }
                self.isSwitching = false
            }
        }
    }

    /// Sets the app's appearance for a theme (`nil` follows the system), instantly.
    public func apply(_ appearance: NSAppearance?) {
        flip { NSApp.appearance = appearance }
    }

    /// The animation to use for a colour/shadow transition right now (nil while switching).
    public func transition(_ animation: Animation) -> Animation? { isSwitching ? nil : animation }
}

extension View {
    /// Animates `value` with `animation` except during a theme flip (`html.theme-switching *`).
    public func themeAwareAnimation<V: Equatable>(_ animation: Animation, value: V) -> some View {
        modifier(ThemeAwareAnimation(animation: animation, value: value))
    }
}

struct ThemeAwareAnimation<V: Equatable>: ViewModifier {
    var animation: Animation
    var value: V
    @Environment(\.accessibilityReduceMotion) private var reduce
    func body(content: Content) -> some View {
        content.animation(ThemeSwitch.shared.transition(.kz.honoring(reduce, animation)), value: value)
    }
}
