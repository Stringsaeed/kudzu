// The `@keyframes` from `globals.css`, with native SwiftUI mechanisms and the CSS numbers as spec:
//
// - `rise`, `pop`, `check-in`, `bar-grow`, `permission-confirm`: one-way from → to animations, as
//   insertion transitions (`KZTransition`) and as on-appear modifiers (`.rise()`, `.pop()`, …) for
//   views that arrive with their parent, where a transition would not run.
// - `tick` and `star-burst`: replayed on a trigger with `keyframeAnimator`.
// - `.stagger(i)`: `rise` with the per-child delays of `.stagger > :nth-child(n)`.
//
// Every one collapses under Reduce Motion (`@Environment(\.accessibilityReduceMotion)`).

import SwiftUI

/// The keyframe values from `globals.css`.
public enum KeyframeSpec {
    /// `rise`: `translateY(4px)`, opacity 0 → none, 1; `--motion-slow`, `--spring-soft`.
    public static let riseOffset: CGFloat = 4
    /// `pop`: `scale(0.94)`, opacity 0 → 1; `--motion-slow`, `--spring-pop`.
    public static let popScale: CGFloat = 0.94
    /// `check-in`: `scale: 0.4`, opacity 0 → 1; `--motion-slow`, `--spring-pop`.
    public static let checkScale: CGFloat = 0.4
    /// `tick`: `scale: 1.3` + `color: var(--iris-foreground)` → 1; `--motion-slow`, `--spring-soft`.
    public static let tickScale: CGFloat = 1.3
    /// `star-burst 420ms ease-out`: 0% scale 0.5 rotate −24°, 55% scale 1.25 rotate 6°, 100% 1 / 0°.
    public static let starDuration: TimeInterval = 0.420
    /// The normalized time of the star’s peak pose.
    public static let starMidpoint: Double = 0.55
    /// The star’s initial scale and angle.
    public static let starFrom: (scale: CGFloat, rotate: Double) = (0.5, -24)
    /// The star’s peak scale and angle.
    public static let starMid: (scale: CGFloat, rotate: Double) = (1.25, 6)
    /// `bar-grow 480ms var(--spring-soft) backwards`: `scale: 0 1` → `1 1`, `origin-left`.
    public static let barDuration: TimeInterval = 0.480
    /// `permission-confirm 240ms ease-out`: opacity 0, `translateY(4px) scale(.97)` → none.
    public static let confirmDuration: TimeInterval = 0.240
    /// The confirmation’s initial vertical offset in points.
    public static let confirmOffset: CGFloat = 4
    /// The confirmation’s initial scale.
    public static let confirmScale: CGFloat = 0.97
    /// `shimmer 1.6s linear infinite`.
    public static let shimmerPeriod: TimeInterval = 1.6
}

// MARK: One-way animations

/// The from-state of a one-way keyframe animation (the to-state is identity).
public enum KZArrival: Sendable, Hashable {
    case rise, pop, checkIn, barGrow, permissionConfirm

    /// The native animation for this arrival effect.
    public var animation: Animation {
        switch self {
        case .rise: .kz.springSoft(duration: Motion.slow)
        case .pop, .checkIn: .kz.springPop(duration: Motion.slow)
        case .barGrow: .kz.springSoft(duration: KeyframeSpec.barDuration)
        case .permissionConfirm: .kz.easeOut(duration: KeyframeSpec.confirmDuration)
        }
    }

    var from: ArrivalEffect {
        switch self {
        case .rise: ArrivalEffect(opacity: 0, offsetY: KeyframeSpec.riseOffset)
        case .pop: ArrivalEffect(opacity: 0, scale: KeyframeSpec.popScale)
        case .checkIn: ArrivalEffect(opacity: 0, scale: KeyframeSpec.checkScale)
        case .barGrow: ArrivalEffect(scaleX: 0, anchor: .leading)
        case .permissionConfirm: ArrivalEffect(opacity: 0, offsetY: KeyframeSpec.confirmOffset, scale: KeyframeSpec.confirmScale)
        }
    }

    var identity: ArrivalEffect {
        var e = ArrivalEffect()
        e.anchor = from.anchor
        return e
    }
}

/// Opacity, translate and scale, applied like the CSS (`translateY(…) scale(…)`: the translation is
/// not scaled).
struct ArrivalEffect: ViewModifier, Hashable, Sendable {
    var opacity: Double = 1
    var offsetY: CGFloat = 0
    var scale: CGFloat = 1
    var scaleX: CGFloat = 1
    var anchor: UnitPoint = .center

    func body(content: Content) -> some View {
        content
            .scaleEffect(x: scaleX * scale, y: scale, anchor: anchor)
            .offset(y: offsetY)
            .opacity(opacity)
    }
}

/// Insertion transitions for Kudzu’s arrival effects.
public enum KZTransition {
    /// The arrival as an insertion transition with its CSS animation attached (removal is instant,
    /// as the CSS has no exit animation for these).
    public static func arrival(_ kind: KZArrival) -> AnyTransition {
        .asymmetric(
            insertion: AnyTransition.modifier(active: kind.from, identity: kind.identity).animation(kind.animation),
            removal: .identity)
    }

    /// The `rise` design-system value.
    public static var rise: AnyTransition { arrival(.rise) }
    /// The `pop` design-system value.
    public static var pop: AnyTransition { arrival(.pop) }
    /// The `checkIn` design-system value.
    public static var checkIn: AnyTransition { arrival(.checkIn) }
    /// The `barGrow` design-system value.
    public static var barGrow: AnyTransition { arrival(.barGrow) }
    /// The `permissionConfirm` design-system value.
    public static var permissionConfirm: AnyTransition { arrival(.permissionConfirm) }
}

extension View {
    /// Inserts this view with an arrival transition (identity under Reduce Motion).
    public func kzTransition(_ kind: KZArrival) -> some View {
        modifier(ArrivalTransition(kind: kind))
    }

    /// Plays an arrival when this view appears (with its parent), after `delay`. The from-state
    /// shows during the delay (`animation-fill-mode: backwards`).
    public func arrival(_ kind: KZArrival, delay: TimeInterval = 0) -> some View {
        modifier(ArrivalOnAppear(kind: kind, delay: delay))
    }

    /// `animate-rise` on appear: 4 pt up and fade in.
    public func rise(delay: TimeInterval = 0) -> some View { arrival(.rise, delay: delay) }
    /// `animate-pop` on appear: scale 0.94 → 1 and fade in.
    public func pop(delay: TimeInterval = 0) -> some View { arrival(.pop, delay: delay) }
    /// `animate-check` on appear: scale 0.4 → 1 and fade in.
    public func checkIn(delay: TimeInterval = 0) -> some View { arrival(.checkIn, delay: delay) }
    /// `animate-bar` on appear: x-scale 0 → 1 from the leading edge (`origin-left`), after `delay`.
    public func barGrow(delay: TimeInterval = 0) -> some View { arrival(.barGrow, delay: delay) }
    /// `.permission-success` on appear: opacity, 4 pt and 0.97 scale, 240 ms ease-out.
    public func permissionConfirm() -> some View { arrival(.permissionConfirm) }
}

struct ArrivalTransition: ViewModifier {
    var kind: KZArrival
    @Environment(\.accessibilityReduceMotion) private var reduce
    func body(content: Content) -> some View {
        content.transition(reduce ? .identity : KZTransition.arrival(kind))
    }
}

struct ArrivalOnAppear: ViewModifier {
    var kind: KZArrival
    var delay: TimeInterval
    @State private var arrived = false
    @Environment(\.accessibilityReduceMotion) private var reduce

    func body(content: Content) -> some View {
        content
            .modifier(arrived || reduce ? kind.identity : kind.from)
            .onAppear {
                guard !arrived else { return }
                guard !reduce else { arrived = true; return }
                withAnimation(kind.animation.delay(delay)) { arrived = true }
            }
    }
}

// MARK: Triggered keyframes

struct TickValue {
    var scale: CGFloat = 1
    var tint: Double = 0
}

struct StarValue {
    var scale: CGFloat = 1
    var rotate: Double = 0
}

extension View {
    /// `animate-tick`, replayed when `trigger` changes (`key={count}`): scale 1.3 → 1 while the
    /// colour returns from iris-foreground to the element's own, on the soft spring.
    public func tick<T: Equatable & Sendable>(_ trigger: T) -> some View {
        modifier(TickModifier(trigger: trigger))
    }

    /// `animate-star`, replayed when `trigger` changes: 0.5 / −24° → 1.25 / 6° at 55% → 1 / 0°,
    /// 420 ms, `ease-out` per segment (CSS applies the timing function to each keyframe interval).
    public func starBurst<T: Equatable & Sendable>(_ trigger: T) -> some View {
        modifier(StarBurstModifier(trigger: trigger))
    }
}

struct TickModifier<T: Equatable & Sendable>: ViewModifier {
    var trigger: T
    @Environment(\.accessibilityReduceMotion) private var reduce
    @Environment(\.colorScheme) private var scheme

    func body(content: Content) -> some View {
        if reduce {
            content
        } else {
            let iris = KZTheme(scheme)[.irisForeground]
            content.keyframeAnimator(initialValue: TickValue(), trigger: trigger) { view, value in
                view
                    .overlay { if value.tint > 0 { view.foregroundStyle(iris).opacity(value.tint) } }
                    .scaleEffect(value.scale)
            } keyframes: { _ in
                KeyframeTrack(\.scale) {
                    MoveKeyframe(KeyframeSpec.tickScale)
                    SpringKeyframe(1, spring: Motion.softSpring())
                }
                KeyframeTrack(\.tint) {
                    MoveKeyframe(1)
                    SpringKeyframe(0, spring: Motion.softSpring())
                }
            }
        }
    }
}

struct StarBurstModifier<T: Equatable & Sendable>: ViewModifier {
    var trigger: T
    @Environment(\.accessibilityReduceMotion) private var reduce

    func body(content: Content) -> some View {
        if reduce {
            content
        } else {
            let first = KeyframeSpec.starDuration * KeyframeSpec.starMidpoint
            let second = KeyframeSpec.starDuration - first
            content.keyframeAnimator(initialValue: StarValue(), trigger: trigger) { view, value in
                view.rotationEffect(.degrees(value.rotate)).scaleEffect(value.scale)
            } keyframes: { _ in
                KeyframeTrack(\.scale) {
                    MoveKeyframe(KeyframeSpec.starFrom.scale)
                    LinearKeyframe(KeyframeSpec.starMid.scale, duration: first, timingCurve: .easeOut)
                    LinearKeyframe(1, duration: second, timingCurve: .easeOut)
                }
                KeyframeTrack(\.rotate) {
                    MoveKeyframe(KeyframeSpec.starFrom.rotate)
                    LinearKeyframe(KeyframeSpec.starMid.rotate, duration: first, timingCurve: .easeOut)
                    LinearKeyframe(0, duration: second, timingCurve: .easeOut)
                }
            }
        }
    }
}

// MARK: Stagger

/// "Children rise one after another, for a whole page arriving": child n waits 30 ms × (n − 1),
/// capped at 150 ms from the sixth child on; fill is `backwards`.
public enum Stagger {
    /// The delay added per arriving child.
    public static let step: TimeInterval = 0.030
    /// The maximum arrival delay.
    public static let cap: TimeInterval = 0.150

    /// The delay of the child at zero-based `index`.
    public static func delay(for index: Int) -> TimeInterval { min(Double(max(0, index)) * step, cap) }
}

extension View {
    /// Child `index` (zero-based) of a `.stagger` container: rises after its delay.
    public func stagger(_ index: Int) -> some View { rise(delay: Stagger.delay(for: index)) }
}

/// A vertical stack whose children rise one after another (`.stagger` on a flex column).
public struct StaggerStack<Data: RandomAccessCollection, ID: Hashable, Content: View>: View {
    var data: Data
    var id: KeyPath<Data.Element, ID>
    var alignment: HorizontalAlignment
    var spacing: CGFloat?
    var content: (Data.Element) -> Content

    /// Creates StaggerStack with the supplied content and styling.
    public init(_ data: Data, id: KeyPath<Data.Element, ID>, alignment: HorizontalAlignment = .leading, spacing: CGFloat? = nil, @ViewBuilder content: @escaping (Data.Element) -> Content) {
        self.data = data
        self.id = id
        self.alignment = alignment
        self.spacing = spacing
        self.content = content
    }

    /// The composed native view.
    public var body: some View {
        VStack(alignment: alignment, spacing: spacing) {
            ForEach(Array(data.enumerated()), id: \.offset) { index, element in
                content(element).stagger(index).id(element[keyPath: id])
            }
        }
    }
}
