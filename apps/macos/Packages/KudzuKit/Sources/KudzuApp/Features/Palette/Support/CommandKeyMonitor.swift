import AppKit

/// Lights the ⌘1–9 badges while Command is held: the web's keydown of `metaKey` / keyup of Meta
/// in `palette.tsx` and `clipboard-picker.tsx`. Shared by the palette and the clipboard picker.
@MainActor
final class CommandKeyMonitor {
    private var token: Any?

    /// Starts reporting whether Command is down. Calling it again while installed does nothing.
    func install(_ update: @escaping @MainActor (Bool) -> Void) {
        guard token == nil else { return }
        token = NSEvent.addLocalMonitorForEvents(matching: .flagsChanged) { event in
            let down = event.modifierFlags.contains(.command)
            MainActor.assumeIsolated { update(down) }
            return event
        }
    }

    func remove() {
        if let token { NSEvent.removeMonitor(token) }
        token = nil
    }
}
