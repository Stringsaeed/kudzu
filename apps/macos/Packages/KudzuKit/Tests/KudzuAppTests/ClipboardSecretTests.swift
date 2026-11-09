import Foundation
import Testing
import KudzuCore
@testable import KudzuApp

@Test func clipboardSecretCorrectionsFollowDetection() {
    var entry = ClipEntry(kind: .text, hash: "test", firstCopiedAt: Date(), copiedAt: Date())
    #expect(ClipSecretRules.override(for: entry, secret: true) == true)
    #expect(ClipSecretRules.override(for: entry, secret: false) == nil)
    entry.secret = .password
    #expect(ClipSecretRules.override(for: entry, secret: true) == nil)
    #expect(ClipSecretRules.override(for: entry, secret: false) == false)
}
