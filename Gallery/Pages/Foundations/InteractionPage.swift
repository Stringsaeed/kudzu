import KudzuUI
import SwiftUI

struct InteractionPage: GalleryPage {
    static let id = "interaction"
    static let title = "Interaction"
    static let section = GallerySection.foundations
    static let icon = KzIconName.cursor

    @State private var knobOn = false
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        GalleryPageLayout(
            title: "Interaction",
            summary: "Shared by every control through KzInteractive: hover and pressed overlays, the keyboard focus ring, colour motion and the disabled treatment."
        ) {
            SpecimenSection("States", note: "Forced with .kzPreviewInteraction(hovered:pressed:focused:) so snapshots can show them.") {
                GalleryCard {
                    HStack(spacing: KzSpace.s4) {
                        DemoButton(title: "Rest")
                        DemoButton(title: "Hover").kzPreviewInteraction(hovered: true)
                        DemoButton(title: "Pressed").kzPreviewInteraction(pressed: true)
                        DemoButton(title: "Focus").kzPreviewInteraction(focused: true)
                        DemoButton(title: "Disabled").disabled(true)
                    }
                    .padding(KzSpace.s1)
                }
            }
            SpecimenSection("Live", note: "A native Button with a KzInteractive style. Hover, press, or Tab to it with Keyboard navigation on: the ring only appears for keyboard focus.") {
                GalleryCard {
                    HStack(spacing: KzSpace.s4) {
                        DemoButton(title: "Hover me")
                        DemoButton(title: "Tab to me")
                    }
                    .padding(KzSpace.s1)
                }
            }
            SpecimenSection("Focus styles", note: ".ring for controls (2px focus, 2px gap); .edge for fields (2px vine-ink edge).") {
                GalleryCard {
                    HStack(spacing: KzSpace.s8) {
                        RoundedRectangle(cornerRadius: KzRadius.md).fill(KzColor.surfaceRaised)
                            .overlay(RoundedRectangle(cornerRadius: KzRadius.md).strokeBorder(KzColor.line))
                            .frame(width: 120, height: KzSize.control)
                            .kzFocusRing(RoundedRectangle(cornerRadius: KzRadius.md), style: .ring, isFocused: true)
                        RoundedRectangle(cornerRadius: KzRadius.md).fill(KzColor.surfaceRaised)
                            .frame(width: 200, height: KzSize.field)
                            .kzFocusRing(RoundedRectangle(cornerRadius: KzRadius.md), style: .edge, isFocused: true)
                    }
                    .padding(KzSpace.s1)
                }
            }
            SpecimenSection("Motion", note: "KzMotion.color is 160 ms ease-out; KzMotion.knob is 180 ms with a slight overshoot. Both are off under Reduce Motion.") {
                GalleryCard {
                    HStack(spacing: KzSpace.s4) {
                        Button(knobOn ? "Move back" : "Move knob") {
                            withKzAnimation(.knob, reduceMotion: reduceMotion) { knobOn.toggle() }
                        }
                        .buttonStyle(DemoButtonStyle())
                        Capsule().fill(knobOn ? KzColor.vine : KzColor.surfaceSunken)
                            .overlay(Capsule().strokeBorder(knobOn ? KzColor.vineInk : KzColor.edge))
                            .frame(width: 32, height: 20)
                            .overlay(alignment: .leading) {
                                Circle().fill(knobOn ? KzColor.onVine : KzColor.edge).frame(width: 14, height: 14)
                                    .offset(x: knobOn ? 15 : 3)
                            }
                            .accessibilityHidden(true)
                        Text(reduceMotion ? "Reduce Motion is on" : "Reduce Motion is off")
                            .kzTextStyle(.callout).foregroundStyle(KzColor.inkMuted)
                    }
                }
            }
        }
    }
}

/// A minimal button built on KzInteractive, to show the shared behaviour (not the Button component).
private struct DemoButton: View {
    let title: String

    var body: some View {
        Button(title) {}
            .buttonStyle(DemoButtonStyle())
    }
}

private struct DemoButtonStyle: ButtonStyle {
    func makeBody(configuration: Configuration) -> some View {
        let shape = RoundedRectangle(cornerRadius: KzRadius.md)
        KzInteractive(shape, isPressed: configuration.isPressed) { state in
            HStack(spacing: 6) {
                KzIcon(.compose, size: 16)
                configuration.label.kzTextStyle(.bodyStrong)
            }
            .foregroundStyle(state.isEnabled ? KzColor.ink : KzDisabled.foreground)
            .padding(.horizontal, KzSpace.s3)
            .frame(height: KzSize.control)
            .background(KzColor.surfaceRaised, in: shape)
            .overlay(shape.strokeBorder(KzColor.line))
            .kzShadow(.control, in: shape)
        }
    }
}
