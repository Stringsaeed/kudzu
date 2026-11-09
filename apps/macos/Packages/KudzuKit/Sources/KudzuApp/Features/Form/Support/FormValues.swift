import Foundation
import KudzuCore

enum FormFieldChrome {
    static func needsFocusBoundary(focused: Bool) -> Bool { focused }
}

/// The backend supplies the initial values; the form keeps the payload intact.
enum FormValues {
    static func seed(_ request: FormPayload) -> [String: String] {
        request.initialValues
    }
}

/// The height the form asks the shell for (`resizeForm`). The web adds 24 pt of window padding
/// unless the surface is vibrant, and caps the report at 640; the shell then clamps to 160…680.
enum FormLayout {
    static let reportedMax = 640.0

    static func reportedHeight(content: Double, vibrant: Bool) -> Double {
        let pad = vibrant ? 0.0 : 24.0
        return min(ceil(max(0, content) + pad), reportedMax)
    }
}
