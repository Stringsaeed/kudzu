import SwiftUI

/// A native Toggle with Kudzu's 32 × 20 switch treatment.
public struct KzToggle: View {
    private let label: String
    @Binding private var isOn: Bool
    /// Creates a switch whose label is spoken; visible labels belong in the settings row.
    public init(_ label: String, isOn: Binding<Bool>) { self.label = label; _isOn = isOn }
    public var body: some View { Toggle(label, isOn: $isOn).toggleStyle(KzToggleStyle()) }
}

/// Switch styling for native toggles, preserving their accessible on/off value.
public struct KzToggleStyle: ToggleStyle {
    /// Creates the switch style.
    public init() {}
    public func makeBody(configuration: Configuration) -> some View {
        Button { configuration.isOn.toggle() } label: {
            Color.clear.frame(width: 32, height: 20)
        }
        .buttonStyle(SwitchStyle(isOn: configuration.isOn))
        .accessibilityRepresentation {
            Toggle(isOn: configuration.$isOn) { configuration.label }.toggleStyle(.switch)
        }
    }
    private struct SwitchStyle: ButtonStyle {
        let isOn: Bool
        func makeBody(configuration: Configuration) -> some View {
            KzInteractive(Capsule(), isPressed: configuration.isPressed) { state in
                configuration.label
                    .background(state.isEnabled && isOn ? KzColor.vine : KzDisabled.fill, in: Capsule())
                    .overlay(Capsule().strokeBorder(state.isEnabled && isOn ? KzColor.vineInk : KzColor.edge))
                    .overlay(alignment: .leading) {
                        Circle().fill(!state.isEnabled ? KzDisabled.foreground : isOn ? KzColor.onVine : KzColor.edge)
                            .frame(width: 14, height: 14)
                            .background { if isOn { Circle().fill(.clear).kzShadow(.control, in: Circle()) } }
                            .offset(x: isOn ? 15 : 3)
                            .kzAnimation(.knob, value: isOn)
                    }
            }
        }
    }
}
