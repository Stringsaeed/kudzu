import AppKit
import Observation

@MainActor @Observable
final class FrontmostApp {
    private(set) var bundleID = NSWorkspace.shared.frontmostApplication?.bundleIdentifier
    var onActivation: (() -> Void)?
    private var observer: NSObjectProtocol?

    init() {
        observer = NSWorkspace.shared.notificationCenter.addObserver(
            forName: NSWorkspace.didActivateApplicationNotification, object: nil, queue: .main
        ) { [weak self] _ in
            MainActor.assumeIsolated {
                self?.bundleID = NSWorkspace.shared.frontmostApplication?.bundleIdentifier
                self?.onActivation?()
            }
        }
    }

    isolated deinit { stop() }

    func stop() {
        if let observer { NSWorkspace.shared.notificationCenter.removeObserver(observer) }
        observer = nil
    }
}
