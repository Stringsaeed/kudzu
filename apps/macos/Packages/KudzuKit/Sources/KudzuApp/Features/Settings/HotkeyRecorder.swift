import AppKit
import KudzuCore
import KudzuDesign
import SwiftUI

/// Pure interpretation of a recorded key press (`settings.tsx` `HotkeyRecorder`).
enum HotkeyRecorderLogic {
    enum Outcome: Equatable, Sendable {
        case cancel
        case ignore
        case accepted(String)
        case rejected(String)
    }

    private static let modifierKeyCodes: Set<UInt16> = [54, 55, 56, 57, 58, 59, 60, 61, 62, 63]
    private static let disallowedKeys: Set<String> = [
        "Escape", "Backspace", "Delete", "ArrowUp", "ArrowDown", "ArrowLeft", "ArrowRight",
    ]

    /// The recorder's text while it listens.
    static let prompt = "Press shortcut…"

    /// The spoken label (`aria-label` in `settings.tsx`).
    static func accessibilityLabel(value: String, label: String, recording: Bool) -> String {
        recording
            ? "Press the new shortcut, or Escape to cancel"
            : "\(label): \(Keys.acceleratorLabels(value).joined(separator: " ")). Press to change."
    }

    static func interpret(keyCode: UInt16, modifiers: KeyModifiers) -> Outcome {
        if keyCode == 53 {
            return .cancel
        }
        if modifierKeyCodes.contains(keyCode) {
            return .ignore
        }
        if let accelerator = Keys.acceleratorFromEvent(keyCode: keyCode, modifiers: modifiers) {
            return .accepted(accelerator)
        }
        guard let codeName = Keys.codes[keyCode], !disallowedKeys.contains(codeName) else {
            return .rejected("That key cannot be used for a global shortcut")
        }
        return .rejected("Include ⌘, ⌃, or ⌥ in the shortcut")
    }
}

/// Interactive global hotkey recorder button (`settings.tsx` `HotkeyRecorder`): a raised keycap
/// button (`h-8 min-w-36 rounded-lg px-3 bg-surface-raised shadow-raised sheen`) that turns iris
/// with a 2 pt ring and a pulsing "Press shortcut…" while it listens. Escape cancels; a key without
/// ⌘/⌃/⌥ is ignored and listening goes on, as on the web; clicking elsewhere or leaving the
/// window stops it, like the web's blur. A shortcut that's taken is reported by the caller's toast.
public struct HotkeyRecorder: View {
    public var value: String
    public var label: String
    public var onChange: (String) async -> Bool

    @State private var recording: Bool = false
    @State private var eventMonitor: Any?

    public init(value: String, label: String = "Quick search shortcut", onChange: @escaping (String) async -> Bool) {
        self.value = value
        self.label = label
        self.onChange = onChange
    }

    public var body: some View {
        Button {
            if !recording { startRecording() }
        } label: {
            if recording {
                Text(HotkeyRecorderLogic.prompt)
                    .font(.system(size: 12))
                    .foregroundStyle(.kz.irisForeground)
                    .modifier(Pulse())
            } else {
                Shortcut(keys: Keys.acceleratorLabels(value), small: false)
            }
        }
        .buttonStyle(RecorderStyle(recording: recording))
        .accessibilityLabel(HotkeyRecorderLogic.accessibilityLabel(value: value, label: label, recording: recording))
        .onDisappear { stopRecording() }
        .onReceive(NotificationCenter.default.publisher(for: NSWindow.didResignKeyNotification)) { _ in stopRecording() }
    }

    private func startRecording() {
        stopRecording()
        recording = true
        eventMonitor = NSEvent.addLocalMonitorForEvents(matching: [.keyDown, .leftMouseDown, .rightMouseDown]) { event in
            if event.type == .leftMouseDown || event.type == .rightMouseDown {
                MainActor.assumeIsolated {
                    stopRecording()
                }
                return event
            }
            let keyCode = event.keyCode
            let modifiers = KeyModifiers(rawValue: UInt64(event.modifierFlags.rawValue))
            MainActor.assumeIsolated {
                handleKeyPress(keyCode: keyCode, modifiers: modifiers)
            }
            return nil
        }
    }

    private func stopRecording() {
        recording = false
        if let eventMonitor {
            NSEvent.removeMonitor(eventMonitor)
            self.eventMonitor = nil
        }
    }

    private func handleKeyPress(keyCode: UInt16, modifiers: KeyModifiers) {
        switch HotkeyRecorderLogic.interpret(keyCode: keyCode, modifiers: modifiers) {
        case .cancel:
            stopRecording()
        case .ignore, .rejected:
            break
        case let .accepted(accelerator):
            stopRecording()
            Task { _ = await onChange(accelerator) }
        }
    }
}

/// The recorder's surface: raised (hover, pressed) keycap button, or iris-soft with an outer ring.
private struct RecorderStyle: ButtonStyle {
    var recording: Bool

    func makeBody(configuration: Configuration) -> some View {
        RecorderBody(configuration: configuration, recording: recording)
    }
}

private struct RecorderBody: View {
    var configuration: ButtonStyleConfiguration
    var recording: Bool
    @State private var hovered = false

    var body: some View {
        let shape = RoundedRectangle(cornerRadius: Radius.lg)
        let depth: Depth = configuration.isPressed ? .pressed : (hovered ? .raisedHover : .raised)
        configuration.label
            .padding(.horizontal, 12)
            .frame(minWidth: 144, minHeight: 32, maxHeight: 32)
            .background {
                if recording {
                    shape.fill(.token(.irisSoft))
                } else {
                    ZStack {
                        Color.clear.depth(depth, fill: Color.kz.surfaceRaised, in: shape)
                        shape.fill(Sheen.normal.gradient)
                    }
                }
            }
            .overlay {
                if recording {
                    // `ring-2 ring-iris/60`: outside the button's edge.
                    RoundedRectangle(cornerRadius: Radius.lg + 2)
                        .strokeBorder(.token(.iris, alpha: 0.6), lineWidth: 2)
                        .padding(-2)
                }
            }
            .contentShape(shape)
            .onHover { hovered = $0 }
            .animation(.kz.hover, value: hovered)
            .animation(.kz.hover, value: configuration.isPressed)
            .animation(.kz.colors, value: recording)
    }
}

/// `animate-pulse`: opacity 1 → 0.5 → 1 every 2 s (`cubic-bezier(0.4, 0, 0.6, 1)`). Off under
/// Reduce Motion.
private struct Pulse: ViewModifier {
    @State private var dim = false
    @Environment(\.accessibilityReduceMotion) private var reduce

    func body(content: Content) -> some View {
        content
            .opacity(reduce ? 1 : dim ? 0.5 : 1)
            .task(id: reduce) {
                var reset = Transaction(animation: nil)
                reset.disablesAnimations = true
                withTransaction(reset) { dim = false }
                guard !reduce else { return }
                withAnimation(.timingCurve(0.4, 0, 0.6, 1, duration: 1).repeatForever(autoreverses: true)) { dim = true }
            }
    }
}
