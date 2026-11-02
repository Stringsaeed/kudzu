import Foundation
import KudzuCore
import Security

/// Reads this process's signature and reports why iCloud can't be used, if it can't.
enum CloudSyncProbe {
    static func block() -> CloudSyncBlock? {
        let signature = Signature.current()
        return CloudSyncEligibility.block(
            teamID: signature.teamID,
            containerIdentifiers: signature.containerIdentifiers
        )
    }

    private struct Signature {
        var teamID: String?
        var containerIdentifiers: [String]

        static func current() -> Signature {
            var code: SecStaticCode?
            guard SecStaticCodeCreateWithPath(Bundle.main.bundleURL as CFURL, [], &code) == errSecSuccess,
                  let code else {
                return Signature(teamID: nil, containerIdentifiers: [])
            }
            var info: CFDictionary?
            let flags = SecCSFlags(rawValue: kSecCSSigningInformation)
            guard SecCodeCopySigningInformation(code, flags, &info) == errSecSuccess,
                  let info = info as? [String: Any] else {
                return Signature(teamID: nil, containerIdentifiers: [])
            }
            let team = info[kSecCodeInfoTeamIdentifier as String] as? String
            let entitlements = info[kSecCodeInfoEntitlementsDict as String] as? [String: Any]
            let containers = entitlements?["com.apple.developer.icloud-container-identifiers"] as? [String] ?? []
            return Signature(teamID: team, containerIdentifiers: containers)
        }
    }
}
