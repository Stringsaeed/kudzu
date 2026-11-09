import Foundation
import KudzuCore
import Testing
@testable import KudzuApp

@Suite struct SettingsLogicTests {
    @Test func retentionChoiceRoundsAndRejectsOutOfRange() {
        #expect(RetentionChoice.parseCustom("10.6") == 11)
        #expect(RetentionChoice.parseCustom("7") == 7)
        #expect(RetentionChoice.parseCustom("0") == nil)
        #expect(RetentionChoice.parseCustom("3651") == nil)
        #expect(RetentionChoice.selection(days: 3, customMode: false) == .custom)
        #expect(RetentionChoice.description(days: 0).contains("Pinned items always stay"))
    }

    @Test func syncLineAndUpdateRowMatchTheWebCopy() {
        #expect(SyncLine.of(.off).text == "Off. Your snippets stay on this Mac.")
        #expect(SyncLine.of(.needsUpdate).action == .update)
        let checking = UpdateRowContent.of(.checking)
        #expect(checking == .row(description: "Checking…", tone: .plain, button: "Check now", checking: true))
        let moved = UpdateRowContent.of(.error(message: "Move Kudzu", needsMove: true))
        #expect(moved == .row(description: "Move Kudzu", tone: .warning, button: "Check now", checking: false))
        let failed = UpdateRowContent.of(.error(message: "offline", needsMove: false))
        #expect(failed == .row(description: "Couldn’t check: offline", tone: .destructive, button: "Try again", checking: false))
    }

    @Test func releaseNotesDropTheVersionTitleAndMarkdown() {
        let notes = ReleaseNotes.parse("""
        # 1.2.3
        ## Added
        - Hello [docs](https://example.com) ([abcdef0](https://example.com/commit))
        """)
        #expect(notes.count == 1)
        #expect(notes[0].heading == "Added")
        #expect(notes[0].items == ["Hello docs"])
    }

    @Test func agentCopyAndAppInsertions() {
        #expect(AgentCopy.access(on: false, running: true) == "Off. Connected agents are told Kudzu is unavailable.")
        #expect(AgentCopy.skill(nil).button == "Install skill")
        let existing: [String: InsertionMethod] = ["com.apple.Terminal": .type]
        let next = AppInsertionsLogic.adding(["com.apple.Terminal", "com.example.Mail"], to: existing)
        #expect(next["com.apple.Terminal"] == .type)
        #expect(next["com.example.Mail"] == .paste)
    }

    @Test func shortcutCatalogMatchesTheCheatSheet() {
        let sections = ShortcutsCatalog.sections(settings: .default)
        #expect(sections.map(\.title) == ["Everywhere", "Navigate", "Snippet list", "Clipboard history", "Editor"])
        #expect(sections[0].rows[0].label == "Search & commands")
        #expect(sections[0].rows[0].binding == .key("mod+k"))
        #expect(SettingsNav.neighbour(of: .about, step: 1) == .general)
    }

    @Test func insightsRanksUsageAndLeavesUnusedTriggers() {
        let alpha = Snippet(name: "Alpha", triggers: [";a"], content: "a")
        let beta = Snippet(name: "Beta", triggers: [";b"], content: "b", isEnabled: false)
        let quiet = Snippet(name: "Quiet", triggers: [";q"], content: "q")
        let blank = Snippet(name: "Blank", triggers: [], content: "")
        let library = Library(snippets: [alpha, beta, quiet, blank])
        let usage: [UUID: SnippetUsage] = [
            alpha.id: SnippetUsage(count: 3, chars: 30, last: 2_000),
            beta.id: SnippetUsage(count: 9, chars: 10, last: 1_000),
        ]
        let report = InsightsLogic.buildReport(library: library, usage: usage, wpm: 40)
        #expect(report.top.map(\.snippet.name) == ["Beta", "Alpha"])
        #expect(report.recent.map(\.snippet.name) == ["Alpha", "Beta"])
        #expect(report.unused.map(\.name) == ["Quiet"])
        #expect(report.totals.expansions == 12)
        #expect(report.totals.activeSnippets == 3)
        #expect(report.turnedOff == 1)
        #expect(InsightsLogic.wordEstimate(chars: 40) == 8)
        #expect(InsightsLogic.clampWPM(12) == 10)
        #expect(InsightsLogic.clampWPM(47) == 45)
        #expect(InsightsLogic.clampWPM(153) == 150)
        #expect(InsightsLogic.barFraction(count: 3, maxCount: 9) == 1.0 / 3.0)
    }

    @Test func onboardingStepsAndPracticeExpansion() {
        #expect(OnboardingLogic.steps(for: .windows) == [.welcome, .importStep, .practice])
        #expect(OnboardingLogic.steps(for: .macos) == [.welcome, .permissions, .importStep, .practice])
        let expanded = OnboardingLogic.evaluatePracticeInput("Hi ;KUDZU")
        #expect(expanded.didExpand)
        #expect(expanded.text == "Hi Hello from Kudzu!")
        #expect(!expanded.text.hasSuffix(" "))
        let summary = ImportSummary(
            importedSnippets: 5,
            importedGroups: 1,
            dedupeReport: DedupeReport(mergedSnippets: 2),
            warnings: [ImportWarning(message: "kept as text")]
        )
        #expect(OnboardingLogic.newSnippetCount(summary) == 3)
        let recorded = OnboardingLogic.recording(summary, onto: nil, accumulate: false)
        #expect(recorded.count == 3)
        #expect(recorded.changed == 1)
        #expect(recorded.firstChange == "kept as text")
        #expect(OnboardingLogic.newSnippetCount(ImportSummary(importedSnippets: 1, importedGroups: 0, dedupeReport: DedupeReport(mergedSnippets: 4))) == 0)
    }

    @Test func permissionCenterFollowsTheCompanionRules() {
        let ready = EngineStatus(
            canPostEvents: true,
            canExpand: true,
            tap: .active,
            permissions: PermissionStatus(accessibility: true, inputMonitoring: true),
            summary: "Ready"
        )
        #expect(PermissionsLogic.isReady(ready))
        let grantedButIdle = EngineStatus(
            canPostEvents: false,
            tap: .off,
            permissions: PermissionStatus(accessibility: true, inputMonitoring: true),
            summary: "Waiting"
        )
        #expect(PermissionsLogic.needsConnection(selected: .accessibility, status: grantedButIdle))
        #expect(!PermissionsLogic.needsConnection(selected: .inputMonitoring, status: grantedButIdle))
        #expect(PermissionsLogic.headline(status: ready, selected: .accessibility) == "You’re all set.")
        #expect(PermissionsLogic.headline(status: grantedButIdle, selected: .accessibility) == "Access is allowed.")
        #expect(PermissionsLogic.headline(status: EngineStatus(summary: "Starting…"), selected: .accessibility) == "Let Kudzu do the typing.")
        let restart = PermissionsLogic.primaryAction(
            status: grantedButIdle,
            selected: .accessibility,
            handoff: .idle,
            restartOffered: true,
            appName: "Kudzu"
        )
        #expect(restart == .restart(appName: "Kudzu"))
        #expect(PermissionsLogic.primaryLabel(restart) == "Restart Kudzu")
        let setup = PermissionsLogic.primaryAction(
            status: grantedButIdle,
            selected: .inputMonitoring,
            handoff: .idle,
            restartOffered: false,
            appName: "Kudzu"
        )
        #expect(setup == .setupAppControl)
        let opening = PermissionsLogic.primaryAction(
            status: EngineStatus(summary: "Starting…"),
            selected: .accessibility,
            handoff: .opening,
            restartOffered: false,
            appName: "Kudzu"
        )
        #expect(PermissionsLogic.primaryLabel(opening) == "Opening Settings…")
        #expect(PermissionsLogic.rowTrailing(kind: .inputMonitoring, granted: false) == "Optional")
        #expect(PermissionsLogic.rowTrailing(kind: .accessibility, granted: true) == "Allowed")
        #expect(PermissionsLogic.setupButton(ready: true, opening: false) == "Review permissions")
    }

    @Test func hotkeyRecorderRejectsBareKeysAndEscape() {
        #expect(HotkeyRecorderLogic.interpret(keyCode: 53, modifiers: []) == .cancel)
        #expect(HotkeyRecorderLogic.interpret(keyCode: 55, modifiers: []) == .ignore)
        #expect(HotkeyRecorderLogic.interpret(keyCode: 40, modifiers: [.command]) == .accepted("Command+K"))
        #expect(HotkeyRecorderLogic.interpret(keyCode: 40, modifiers: []) == .rejected("Include ⌘, ⌃, or ⌥ in the shortcut"))
        #expect(HotkeyRecorderLogic.interpret(keyCode: 126, modifiers: [.command]) == .rejected("That key cannot be used for a global shortcut"))
    }
}
