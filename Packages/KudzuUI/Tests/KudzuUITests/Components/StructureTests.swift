@testable import KudzuUI
import SwiftUI
import Testing

@MainActor
@Suite struct StructureSnapshotTests {
    @Test func disclosure() throws {
        try snapshot("disclosure", width: 420, background: .surfaceRaised) {
            VStack(alignment: .leading, spacing: KzSpace.s4) {
                KzDisclosure("Rules", isInitiallyExpanded: true) {
                    HStack(spacing: KzSpace.s3) {
                        PlaceholderSegmented(options: ["Inherit from group", "Custom"])
                        Text("Works in every app").kzTextStyle(.callout).foregroundStyle(KzColor.inkMuted)
                    }
                    HStack(spacing: KzSpace.s3) {
                        Text("Insertion").kzTextStyle(.bodyStrong).foregroundStyle(KzColor.ink).frame(width: 72, alignment: .leading)
                        PlaceholderSelect(value: "Inherit")
                    }
                }
                KzDisclosure("Advanced", summary: "Word boundary, case, undo") { Text("Hidden while closed") }
                KzDisclosure("Focused", summary: "Keyboard focus ring") { Text("x") }
                    .kzPreviewInteraction(focused: true)
            }
        }
    }

    @Test func emptyState() throws {
        try snapshot("empty-state", width: 640, background: .surfaceRaised) {
            HStack(alignment: .top, spacing: 0) {
                KzEmptyState("No snippet selected") {
                    KzMessage("Choose a snippet to edit it, or press \(kbd: "⌘N") to create one.")
                }
                .frame(maxWidth: .infinity)
                KzEmptyState("No matches", icon: .search, message: "Try a different search, or clear the filter.") {
                    PlaceholderButton(title: "Clear search", small: true)
                }
                .frame(maxWidth: .infinity)
            }
        }
    }

    @Test func field() throws {
        try snapshot("field", width: 420, background: .surfaceRaised) {
            KzFieldStack {
                KzField("Triggers", aside: "Space or return to add") { PlaceholderField(text: ":an") }
                KzField("Group", hint: "Snippets in a group share its rules.") { PlaceholderSelect(value: "base") }
                KzField("Name") { PlaceholderField(placeholder: "Untitled snippet") }
            }
        }
    }

    @Test func settings() throws {
        try snapshot("settings-group", width: 560) {
            VStack(alignment: .leading, spacing: KzSpace.s6) {
                KzSettingsGroup("Startup", description: "Kudzu runs in the menu bar and expands snippets in every app.") {
                    KzSettingsRow("Launch Kudzu at login", description: "Starts quietly in the menu bar", icon: .keyboard) { PlaceholderToggle(isOn: true) }
                    KzSettingsRow("Show welcome again") { PlaceholderButton(title: "Show…", small: true) }
                    KzSettingsRow("Expansion speed", description: "How long Kudzu waits after the trigger") { PlaceholderSegmented(options: ["Fast", "Normal", "Slow"], selected: 1) }
                }
                KzSettingsGroup("Backups", action: { PlaceholderButton(title: "Back up now", small: true) }) {
                    KzSettingsRow("Keep backups", description: "Keeps the last 14 snapshots.") { PlaceholderSelect(value: "14 snapshots") }
                }
                KzSettingsGroup("iCloud") {
                    KzSettingsRow(description: "Automatic CloudKit sync across your Macs lands in milestone 4.", label: {
                        HStack(spacing: KzSpace.s2) { Text("iCloud sync"); KzBadge("Coming soon") }
                    }, control: { PlaceholderToggle(isOn: false) })
                }
            }
        }
    }

    @Test func stats() throws {
        try snapshot("stats", width: 600, background: .surfaceRaised) {
            VStack(alignment: .leading, spacing: KzSpace.s6) {
                KzStatTileRow {
                    KzStatTile("Expansions", value: "1,284", icon: .bolt, note: "+12% on last week")
                    KzStatTile("Characters saved", value: "30", icon: .type)
                    KzStatTile("Time saved", value: "2h 14m", icon: .timer)
                }
                KzSettingsGroup("Most used") {
                    VStack(spacing: 4) {
                        KzUsageBar(":m", value: 128, maximum: 128)
                        KzUsageBar(":an", value: 64, maximum: 128)
                        KzUsageBar(";sig", value: 12, maximum: 128)
                        KzUsageBar(":date", value: 1, maximum: 128)
                        KzUsageBar(":a-longer-trigger", value: 1, maximum: 0)
                    }
                    .padding(.horizontal, KzSpace.s4)
                    .padding(.vertical, KzSpace.s3)
                }
            }
        }
    }
}

@Suite struct StructureLogicTests {
    @Test func usageBarFraction() {
        #expect(KzUsageBar.fraction(value: 64, maximum: 128) == 0.5)
        #expect(KzUsageBar.fraction(value: 128, maximum: 128) == 1)
        #expect(KzUsageBar.fraction(value: 1, maximum: 1000) == 0.02) // never thinner than 2%
        #expect(KzUsageBar.fraction(value: 5, maximum: 0) == 0)
        #expect(KzUsageBar.fraction(value: 200, maximum: 100) == 1)
    }

    @MainActor
    @Test func messageSplitsWordsAndKeycaps() {
        let content: KzMessageContent = "Press \(kbd: "⌘N"). Or \(kbd: "/") to insert."
        let items = KzMessage.items(from: KzMessage(content).segments)
        // Press ⌘N . Or / to insert.  — the full stop is glued to the keycap.
        #expect(items.count == 7)
        #expect(items.map(\.spaceBefore) == [false, true, false, true, true, true, true])
        #expect(KzMessage(content).spokenText == "Press Command N. Or / to insert.")
    }
}
