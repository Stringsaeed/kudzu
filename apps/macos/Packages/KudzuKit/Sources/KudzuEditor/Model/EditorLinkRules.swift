import Foundation

/// The link field's JavaScript policy. Stored/imported formatting retains the Rust library
/// policy; these differ for BOM and C1 controls, so the editor keeps its own input boundary.
enum EditorLinkRules {
    static func safeLink(_ url: String) -> Bool {
        let prefix = url.utf8.prefix(8).map { (65...90).contains($0) ? $0 + 32 : $0 }
        let lower = String(decoding: prefix, as: UTF8.self)
        guard ["http://", "https://", "mailto:"].contains(where: lower.hasPrefix) else { return false }
        return !url.unicodeScalars.contains { scalar in
            switch scalar.value {
            case 0...32, 127, 160, 0x1680, 0x2000...0x200A, 0x2028, 0x2029, 0x202F, 0x205F, 0x3000, 0xFEFF:
                true
            default:
                "\"'<>".unicodeScalars.contains(scalar)
            }
        }
    }
}
