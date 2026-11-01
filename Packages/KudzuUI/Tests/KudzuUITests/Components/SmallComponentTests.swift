@testable import KudzuUI
import SwiftUI
import Testing

@MainActor
@Suite struct SmallComponentTests {
    @Test func triggerChipSplitsLeadingSymbol() {
        #expect(KzTriggerChip.split(":an") == (":", "an"))
        #expect(KzTriggerChip.split("addr") == ("", "addr"))
        #expect(KzTriggerChip.split("//d") == ("/", "/d"))
        #expect(KzTriggerChip.split("") == ("", ""))
    }

    @Test func kbdSpellsModifiers() {
        #expect(KzKbd.spokenName("⌘N") == "Command N")
        #expect(KzKbd.spokenName("⌃⌥Space") == "Control Option S p a c e")
    }

    @Test func snapshots() throws {
        try snapshot("small-components", width: 520) {
            VStack(alignment: .leading, spacing: KzSpace.s3) {
                HStack(spacing: KzSpace.s2) { KzKbd("⌘N"); KzKbd("⌘,"); KzKbd("/"); KzKbd("⌃⌥Space") }
                HStack(spacing: KzSpace.s2) {
                    KzBadge("Manual"); KzBadge("On", tone: .vine); KzBadge("Favorite", tone: .bloom)
                    KzBadge("Overwrites", tone: .warning); KzBadge("Failed", tone: .danger)
                }
                HStack(spacing: KzSpace.s2) {
                    KzTriggerChip(":an"); KzTriggerChip(";sig"); KzTriggerChip("addr"); KzTriggerChip(":an") {}
                }
            }
        }
    }
}
