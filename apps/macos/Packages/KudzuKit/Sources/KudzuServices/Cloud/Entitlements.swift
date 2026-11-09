import Foundation
import Security

/// Code-signing entitlements of the running process.
public enum Entitlements {
    /// The value of `key` in this process's signature, if any.
    public static func value(_ key: String) -> Any? {
        guard let task = SecTaskCreateFromSelf(nil) else { return nil }
        return SecTaskCopyValueForEntitlement(task, key as CFString, nil)
    }

    /// True when this process is signed with the iCloud container entitlement. Touching
    /// CloudKit without it raises an Objective-C exception, so this gates everything.
    public static func hasICloudContainer(_ container: String) -> Bool {
        guard let list = value("com.apple.developer.icloud-container-identifiers") as? [String] else { return false }
        return list.contains(container)
    }
}
