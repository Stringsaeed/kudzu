import Foundation
import KudzuCore

/// The form panel's backend calls (`form-panel.tsx`): submit or cancel the pending form by token,
/// and size the panel to its content.
extension AppModel {
    /// ↵ / Insert: fills the snippet with `values` (the backend remembers them per snippet).
    public func submitForm(token: UInt64, values: [String: String]) async {
        try? await backend.submitForm(token: token, values: values)
    }

    /// Esc: leaves the trigger exactly as typed.
    public func cancelForm(token: UInt64) async {
        try? await backend.cancelForm(token: token)
    }

    /// Asks the shell for a panel this tall (it clamps to 160…680).
    public func resizeForm(height: Double) async {
        try? await backend.resizeForm(height: height)
    }
}
