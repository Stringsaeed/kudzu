import Foundation
import KudzuCore

/// A manual check of the engine's OS plumbing, for debug menus and command-line runs: logs the
/// status a fresh engine would report and the frontmost app. Starts nothing.
public func engineProbe() -> String {
    let permissions = System.permissions()
    let (secureInput, owner) = System.secureInput()
    let status = EngineStatusBuilder.build(StatusInputs(
        paused: false,
        tap: .off,
        permissions: permissions,
        postEvent: AX.postEventAllowed(),
        secureInput: secureInput,
        secureInputOwner: owner
    ))
    let front = System.frontmostApp().map { "\($0.name) (\($0.bundleId), pid \($0.pid.map(String.init) ?? "?"))" } ?? "none"
    let report = """
    Kudzu engine probe
      accessibility:    \(permissions.accessibility)
      input monitoring: \(permissions.inputMonitoring)
      can post events:  \(status.canPostEvents)
      secure input:     \(secureInput)\(owner.map { " (\($0))" } ?? "")
      summary (idle):   \(status.summary)
      frontmost app:    \(front)
      locale:           \(System.locale())
    """
    EngineLog.logger.debug("\(report, privacy: .public)")
    return report
}
