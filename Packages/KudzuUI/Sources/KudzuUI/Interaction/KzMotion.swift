import SwiftUI

/// Kudzu's motion tokens. Motion never applies to layout, and Reduce Motion turns it off.
public enum KzMotion: Sendable, CaseIterable {
    /// Colour changes (hover, press, selection, focus): 160 ms ease-out.
    case color
    /// The toggle knob: 180 ms with a slight overshoot.
    case knob

    /// Duration in seconds.
    public var duration: Double {
        switch self {
        case .color: 0.16
        case .knob: 0.18
        }
    }

    /// The animation, ignoring accessibility settings.
    public var animation: Animation {
        switch self {
        case .color: .easeOut(duration: duration)
        // The CSS uses cubic-bezier(.3,.7,.4,1); the README asks for a slight overshoot,
        // so the second control point rises just past 1.
        case .knob: .timingCurve(0.3, 1.25, 0.4, 1, duration: duration)
        }
    }

    /// The animation to use, or `nil` (no animation) when Reduce Motion is on.
    public func animation(reduceMotion: Bool) -> Animation? {
        reduceMotion ? nil : animation
    }
}

private struct KzAnimationModifier<V: Equatable>: ViewModifier {
    let motion: KzMotion
    let value: V
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    func body(content: Content) -> some View {
        content.animation(motion.animation(reduceMotion: reduceMotion), value: value)
    }
}

extension View {
    /// Animates changes to `value` with a Kudzu motion token, or not at all under Reduce Motion.
    public func kzAnimation<V: Equatable>(_ motion: KzMotion, value: V) -> some View {
        modifier(KzAnimationModifier(motion: motion, value: value))
    }
}

/// Runs a state change with a Kudzu motion token, honouring Reduce Motion.
@MainActor
public func withKzAnimation<Result>(_ motion: KzMotion, reduceMotion: Bool, _ body: () throws -> Result) rethrows -> Result {
    try withAnimation(motion.animation(reduceMotion: reduceMotion), body)
}
