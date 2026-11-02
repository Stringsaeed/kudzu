import AppKit

/// Makes successful draft persistence a prerequisite for normal application termination.
@MainActor
final class KudzuAppDelegate: NSObject, NSApplicationDelegate {
    var prepareForTermination: () -> Bool = { true }

    func applicationShouldTerminate(_ sender: NSApplication) -> NSApplication.TerminateReply {
        prepareForTermination() ? .terminateNow : .terminateCancel
    }
}
