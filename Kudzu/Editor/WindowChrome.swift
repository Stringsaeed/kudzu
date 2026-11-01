import AppKit
import SwiftUI

/// Gives the main window a Dock icon while it is open (Kudzu is otherwise a menu-bar app).
///
/// Attach with `.background(DockPolicyAccessor())`. Opening switches to `.regular` and activates;
/// closing returns to `.accessory` unless another ordinary window is still open.
struct DockPolicyAccessor: NSViewRepresentable {
    func makeCoordinator() -> Coordinator { Coordinator() }

    func makeNSView(context: Context) -> NSView {
        let view = TrackingView()
        view.onWindowChange = { [weak coordinator = context.coordinator] window in
            coordinator?.attach(to: window)
        }
        return view
    }

    func updateNSView(_ nsView: NSView, context: Context) {}

    private final class TrackingView: NSView {
        var onWindowChange: ((NSWindow?) -> Void)?
        override func viewDidMoveToWindow() {
            super.viewDidMoveToWindow()
            onWindowChange?(window)
        }
    }

    @MainActor
    final class Coordinator {
        private var observer: NSObjectProtocol?
        private weak var window: NSWindow?

        func attach(to window: NSWindow?) {
            guard let window, window !== self.window else { return }
            self.window = window
            NSApp.setActivationPolicy(.regular)
            NSApp.activate()
            window.makeKeyAndOrderFront(nil)

            if let observer { NotificationCenter.default.removeObserver(observer) }
            let closingID = ObjectIdentifier(window)
            observer = NotificationCenter.default.addObserver(
                forName: NSWindow.willCloseNotification, object: window, queue: .main
            ) { _ in
                MainActor.assumeIsolated { Self.windowWillClose(closingID) }
            }
        }

        private static func windowWillClose(_ closing: ObjectIdentifier) {
            // The window is still visible during willClose; look again once it is gone.
            Task { @MainActor in
                let othersOpen = NSApp.windows.contains { window in
                    ObjectIdentifier(window) != closing && window.isVisible && !(window is NSPanel)
                        && window.level == .normal && window.canBecomeMain
                }
                if !othersOpen { NSApp.setActivationPolicy(.accessory) }
            }
        }

        isolated deinit {
            if let observer { NotificationCenter.default.removeObserver(observer) }
        }
    }
}
