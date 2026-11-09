import Foundation
import KudzuCore
import Testing
@testable import KudzuApp

/// Divergences from the web found in the T11 review, pinned as pure rules.
@Suite struct SettingsReviewTests {
    /// `insights.tsx` ranks with a stable `Array.sort` on count alone, so equal counts keep library
    /// order. Names do not break ties.
    @Test func insightsTiesKeepLibraryOrder() {
        let zed = Snippet(name: "Zed", triggers: [";z"], content: "z")
        let alpha = Snippet(name: "Alpha", triggers: [";a"], content: "a")
        let mid = Snippet(name: "Mid", triggers: [";m"], content: "m")
        let library = Library(snippets: [zed, alpha, mid])
        let usage: [UUID: SnippetUsage] = [
            zed.id: SnippetUsage(count: 4, chars: 4, last: 500),
            alpha.id: SnippetUsage(count: 4, chars: 4, last: 500),
            mid.id: SnippetUsage(count: 4, chars: 4, last: 900),
        ]
        let report = InsightsLogic.buildReport(library: library, usage: usage, wpm: 40)
        #expect(report.top.map(\.snippet.name) == ["Zed", "Alpha", "Mid"])
        // Recent re-sorts the ranked list by last use, again stably.
        #expect(report.recent.map(\.snippet.name) == ["Mid", "Zed", "Alpha"])
    }

    /// `onboarding.tsx` counts every folder imported so far (`completedImports.current.size`),
    /// including those from an earlier attempt, not only this retry's.
    @Test func espansoRetryCountsEveryCompletedFolder() {
        #expect(OnboardingLogic.espansoFailure("disk full", completed: 0, total: 3) == "disk full")
        #expect(
            OnboardingLogic.espansoFailure("disk full", completed: 2, total: 3)
                == "Imported 2 of 3 folders; retry brings in the rest. disk full"
        )
    }

    /// `settings.tsx` `Agents`: the skill button shows while setup loads, disabled, and keeps its
    /// label while installing (the spinner sits beside it).
    @Test func agentSkillButtonWaitsForSetup() {
        #expect(AgentCopy.skillButtonDisabled(setup: nil, installing: false))
        #expect(AgentCopy.skill(nil).button == "Install skill")
    }

    /// `settings.tsx` `HotkeyRecorder`: "Press shortcut…" while recording, and the spoken label.
    @Test func hotkeyRecorderCopy() {
        #expect(HotkeyRecorderLogic.prompt == "Press shortcut…")
        #expect(
            HotkeyRecorderLogic.accessibilityLabel(value: "Control+Alt+Space", label: "Quick search shortcut", recording: false)
                == "Quick search shortcut: ⌃ ⌥ Space. Press to change."
        )
        #expect(
            HotkeyRecorderLogic.accessibilityLabel(value: "Control+Alt+Space", label: "Quick search shortcut", recording: true)
                == "Press the new shortcut, or Escape to cancel"
        )
    }

    /// The web settings header is the section label only. The native subtitles were invented copy
    /// ("…and macOS permissions" on Expansion is wrong: permissions live in General).
    @Test func settingsHeaderIsTheSectionLabel() {
        #expect(SettingsNav.header(for: .expansion) == "Expansion")
        #expect(SettingsNav.header(for: .data) == "Backups & data")
    }
}
