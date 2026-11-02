import AppKit
import Carbon
import Observation
import KudzuUI
import SwiftUI

/// Settings control for the search palette shortcut: a recorder, "Reset to Default" and any
/// registration error.
struct PaletteShortcutSetting: View {
    @Environment(AppModel.self) private var model

    var body: some View {
        VStack(alignment: .trailing, spacing: 4) {
            HStack(spacing: 8) {
                ShortcutRecorder(
                    shortcut: model.paletteShortcut,
                    onBegin: { model.setPaletteShortcutSuspended(true) },
                    onEnd: { model.setPaletteShortcutSuspended(false) },
                    onRecord: { model.setPaletteShortcut($0) }
                )
                KzButton("Reset", variant: .ghost, size: .sm) { model.resetPaletteShortcut() }
                    .disabled(model.paletteShortcut == .paletteDefault && model.paletteShortcutError == nil)
            }
            if let error = model.paletteShortcutError {
                Text(error)
                    .kzTextStyle(.caption)
                    .foregroundStyle(KzColor.danger)
                    .multilineTextAlignment(.trailing)
                    .fixedSize(horizontal: false, vertical: true)
            }
        }
    }
}

/// A click-to-record shortcut field. While recording, the next key press with ⌘, ⌃ or ⌥ (or a
/// function key) becomes the shortcut; Esc or a click elsewhere cancels.
struct ShortcutRecorder: View {
    var shortcut: HotKeyShortcut
    var onBegin: () -> Void = {}
    var onEnd: () -> Void = {}
    var onRecord: (HotKeyShortcut) -> Void

    @State private var recorder = Recorder()

    var body: some View {
        Button {
            if recorder.isRecording { recorder.stop() } else { start() }
        } label: {
            KzKbd(label).padding(.horizontal, KzSpace.s2).frame(minWidth: 100, minHeight: 30)
        }
        .buttonStyle(ShortcutWellStyle())
        .help(recorder.isRecording ? "Type the new shortcut, or press Esc to cancel" : "Click to record a new shortcut")
        .accessibilityLabel("Search palette shortcut")
        .accessibilityValue(recorder.isRecording ? "Recording" : shortcut.displayString)
        .onDisappear { recorder.stop() }
    }

    private var label: String {
        guard recorder.isRecording else { return shortcut.displayString }
        return recorder.liveModifiers.isEmpty ? "Type shortcut…" : HotKeyShortcut.modifierSymbols(recorder.liveModifiers) + "…"
    }

    private func start() {
        recorder.start(
            onBegin: onBegin,
            onEnd: onEnd,
            onRecord: onRecord
        )
    }

    /// Owns the local event monitor for one recording session.
    @MainActor @Observable
    final class Recorder {
        private(set) var isRecording = false
        private(set) var liveModifiers: NSEvent.ModifierFlags = []
        @ObservationIgnored private var monitor: Any?
        @ObservationIgnored private var resignObserver: NSObjectProtocol?
        @ObservationIgnored private var onEnd: (() -> Void)?

        func start(onBegin: () -> Void, onEnd: @escaping () -> Void, onRecord: @escaping (HotKeyShortcut) -> Void) {
            guard !isRecording else { return }
            isRecording = true
            liveModifiers = []
            self.onEnd = onEnd
            onBegin()
            monitor = NSEvent.addLocalMonitorForEvents(matching: [.keyDown, .flagsChanged]) { [weak self] event in
                // NSEvent isn't Sendable, so only the decision crosses into the isolated closure.
                let type = event.type, keyCode = event.keyCode, flags = event.modifierFlags
                let shortcut = HotKeyShortcut(event: event)
                let swallow = MainActor.assumeIsolated {
                    self?.handle(type: type, keyCode: keyCode, flags: flags, shortcut: shortcut, onRecord: onRecord) ?? false
                }
                return swallow ? nil : event
            }
            // Leaving the Settings window ends the session rather than leaving the hotkey suspended.
            resignObserver = NotificationCenter.default.addObserver(
                forName: NSWindow.didResignKeyNotification, object: NSApp.keyWindow, queue: .main
            ) { [weak self] _ in
                MainActor.assumeIsolated { self?.stop() }
            }
        }

        func stop() {
            guard isRecording else { return }
            isRecording = false
            liveModifiers = []
            if let monitor { NSEvent.removeMonitor(monitor) }
            if let resignObserver { NotificationCenter.default.removeObserver(resignObserver) }
            monitor = nil
            resignObserver = nil
            let onEnd = onEnd
            self.onEnd = nil
            onEnd?()
        }

        /// Returns true to swallow the event.
        private func handle(type: NSEvent.EventType, keyCode: UInt16, flags: NSEvent.ModifierFlags,
                            shortcut: HotKeyShortcut?, onRecord: (HotKeyShortcut) -> Void) -> Bool {
            let flags = flags.intersection([.command, .option, .control, .shift])
            if type == .flagsChanged {
                liveModifiers = flags
                return false
            }
            if flags.isEmpty, Int(keyCode) == kVK_Escape {
                stop()
                return true
            }
            guard let shortcut else {
                NSSound.beep()
                return true
            }
            // Re-enable the old shortcut first so `onRecord` replaces a live registration.
            stop()
            onRecord(shortcut)
            return true
        }
    }
}

private struct ShortcutWellStyle: ButtonStyle {
    func makeBody(configuration: Configuration) -> some View {
        let shape = RoundedRectangle(cornerRadius: KzRadius.md)
        KzInteractive(shape, isPressed: configuration.isPressed) { _ in
            configuration.label.background(KzColor.surfaceSunken, in: shape)
                .overlay(shape.strokeBorder(KzColor.line))
        }
    }
}
