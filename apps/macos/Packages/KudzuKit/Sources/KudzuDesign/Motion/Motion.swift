// Motion. "Three speeds: the bigger the thing that moves, the slower it goes. Two damped-spring
// curves: soft (1.5% overshoot) for things that travel, pop (10%) for things that land."
//
// The web wrote its springs as `linear()` sample tables because CSS has no spring easing. Here they
// are native SwiftUI springs fitted to those tables (`SpringFitTests`): both tables are damped
// springs sampled over the CSS duration, so the fit pins a bounce and the ratio of the spring's
// perceptual duration to the CSS duration. The web truncates the curve at the CSS duration (its last
// point is forced to 1); the native spring settles the remaining ≤1.5% naturally instead.
//
// Reduce Motion: views read `@Environment(\.accessibilityReduceMotion)` and collapse animations to
// 1 ms (the `prefers-reduced-motion` block). `Motion.reduced` is for non-view code only.

import AppKit
import os
import SwiftUI

/// Durations and native springs shared by Kudzu views.
public enum Motion {
    /// `--motion-fast`.
    public static let fast: TimeInterval = 0.080
    /// `--motion-moderate`.
    public static let moderate: TimeInterval = 0.160
    /// `--motion-slow`.
    public static let slow: TimeInterval = 0.240
    /// The reduced-motion duration (`animation-duration: 1ms !important`).
    public static let reducedDuration: TimeInterval = 0.001

    // MARK: Springs (fitted to globals.css `--spring-soft` / `--spring-pop`)

    /// `--spring-soft` bounce (≈1.5% overshoot).
    public static let softBounce: Double = 0.2
    /// Spring perceptual duration per second of CSS duration for `--spring-soft` (0.262 s at 240 ms).
    public static let softDurationRatio: Double = 0.262 / 0.240
    /// `--spring-pop` bounce (≈10% overshoot).
    public static let popBounce: Double = 0.42
    /// Spring perceptual duration per second of CSS duration for `--spring-pop` (0.19 s at 240 ms).
    public static let popDurationRatio: Double = 0.190 / 0.240

    /// The native spring equivalent to `var(--spring-soft)` over a CSS duration.
    public static func softSpring(duration: TimeInterval = slow) -> Spring {
        Spring(duration: duration * softDurationRatio, bounce: softBounce)
    }

    /// The native spring equivalent to `var(--spring-pop)` over a CSS duration.
    public static func popSpring(duration: TimeInterval = slow) -> Spring {
        Spring(duration: duration * popDurationRatio, bounce: popBounce)
    }

    // MARK: Reduce Motion (non-view code)

    /// Whether the user asked to reduce motion, for code outside views (AppKit animations, panels).
    /// Views use `@Environment(\.accessibilityReduceMotion)`.
    public static var reduced: Bool {
        _ = observer
        return state.withLock { $0 }
    }

    private static let state = OSAllocatedUnfairLock(initialState: NSWorkspace.shared.accessibilityDisplayShouldReduceMotion)

    private static let observer: Void = {
        _ = NSWorkspace.shared.notificationCenter.addObserver(
            forName: NSWorkspace.accessibilityDisplayOptionsDidChangeNotification, object: nil, queue: nil
        ) { _ in
            let value = NSWorkspace.shared.accessibilityDisplayShouldReduceMotion
            state.withLock { $0 = value }
        }
    }()

    /// A duration as the CSS would run it (1 ms when `reduce`).
    public static func duration(_ d: TimeInterval, reduce: Bool) -> TimeInterval { reduce ? reducedDuration : d }
}

// MARK: Animations

extension Animation {
    /// Kudzu animations: `.kz.springSoft()`, `.kz.springPop(duration: Motion.moderate)`, `.kz.hover`.
    public static let kz = KZAnimations()
}

/// Native animations using Kudzu durations and fitted springs.
public struct KZAnimations: Sendable {
    /// `var(--spring-soft)` (`ease-spring`) over a CSS duration.
    public func springSoft(duration: TimeInterval = Motion.slow) -> Animation { .spring(Motion.softSpring(duration: duration)) }
    /// `var(--spring-pop)` (`ease-pop`) over a CSS duration.
    public func springPop(duration: TimeInterval = Motion.slow) -> Animation { .spring(Motion.popSpring(duration: duration)) }
    /// CSS `ease-out` = `cubic-bezier(0, 0, 0.58, 1)`, SwiftUI's `easeOut`.
    public func easeOut(duration: TimeInterval) -> Animation { .easeOut(duration: duration) }
    /// CSS `ease` = `cubic-bezier(0.25, 0.1, 0.25, 1)` (the default `transition-timing-function`).
    public func ease(duration: TimeInterval) -> Animation { .timingCurve(0.25, 0.1, 0.25, 1, duration: duration) }

    /// The `press` hover transition: colour, background, border, shadow, opacity, filter over
    /// `--motion-fast` with the default `ease`.
    public var hover: Animation { ease(duration: Motion.fast) }
    /// `transition-colors` (Tailwind's 150 ms `cubic-bezier(0.4, 0, 0.2, 1)`).
    public var colors: Animation { .timingCurve(0.4, 0, 0.2, 1, duration: 0.15) }
    /// Overlay enter (`animate-in … duration-(--motion-moderate) ease-pop`).
    public var overlayIn: Animation { springPop(duration: Motion.moderate) }
    /// Overlay exit (`data-closed:duration-100 data-closed:ease-out`).
    public var overlayOut: Animation { easeOut(duration: 0.1) }
    /// Dialog enter (`duration-(--motion-slow) ease-pop`).
    public var dialogIn: Animation { springPop(duration: Motion.slow) }
    /// Dialog exit (`data-closed:duration-(--motion-moderate) data-closed:ease-out`).
    public var dialogOut: Animation { easeOut(duration: Motion.moderate) }

    /// The reduced-motion stand-in: 1 ms.
    public var reduced: Animation { .linear(duration: Motion.reducedDuration) }
    /// `animation` normally, the 1 ms stand-in when `reduce`.
    public func honoring(_ reduce: Bool, _ animation: Animation) -> Animation { reduce ? reduced : animation }
}

extension View {
    /// `.animation(_:value:)` that collapses to 1 ms under Reduce Motion, like the CSS media query.
    public func kzAnimation<V: Equatable>(_ animation: Animation, value: V) -> some View {
        modifier(ReduceAwareAnimation(animation: animation, value: value))
    }
}

struct ReduceAwareAnimation<V: Equatable>: ViewModifier {
    var animation: Animation
    var value: V
    @Environment(\.accessibilityReduceMotion) private var reduce
    func body(content: Content) -> some View {
        content.animation(.kz.honoring(reduce, animation), value: value)
    }
}

/// `withAnimation` honouring a Reduce Motion flag read from the environment.
@MainActor
public func withKZAnimation<R>(_ animation: Animation, reduce: Bool, _ body: () throws -> R) rethrows -> R {
    try withAnimation(.kz.honoring(reduce, animation), body)
}
