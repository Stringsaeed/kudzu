import AppKit
import Carbon
import Observation

@MainActor @Observable
final class SecureInputMonitor {
    private(set) var isEnabled = false
    private(set) var responsibleApp: String?

    func refresh() {
        isEnabled = IsSecureEventInputEnabled()
        responsibleApp = nil
        // This undocumented dictionary entry is advisory only; never gate detection on it.
        if isEnabled, let session = CGSessionCopyCurrentDictionary() as? [String: Any],
           let pid = session["kCGSSessionSecureInputPID"] as? NSNumber {
            responsibleApp = NSRunningApplication(processIdentifier: pid.int32Value)?.localizedName
        }
    }
}
