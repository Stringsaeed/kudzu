import Foundation
import KudzuCore

/// Pure state transitions and practice-box expansion logic for `OnboardingScreen` (`onboarding.tsx`).
enum OnboardingLogic {
    enum Step: String, CaseIterable, Equatable, Sendable, Identifiable {
        case welcome
        case permissions
        case importStep = "import"
        case practice

        var id: String { rawValue }

        var label: String {
            switch self {
            case .welcome: "Welcome"
            case .permissions: "Access"
            case .importStep: "Import"
            case .practice: "Try it"
            }
        }

        var index: Int {
            Self.allCases.firstIndex(of: self) ?? 0
        }
    }

    static let practiceTrigger = ";kudzu"
    /// `SAMPLE_TEXT`. No trailing space — success is `value.contains` this string.
    static let practiceExpansion = "Hello from Kudzu!"

    struct PracticeResult: Equatable, Sendable {
        var text: String
        var didExpand: Bool
    }

    /// Replaces a trailing `;kudzu` (case-insensitive) with `practiceExpansion`.
    static func evaluatePracticeInput(_ value: String) -> PracticeResult {
        if value.lowercased().hasSuffix(practiceTrigger) {
            let prefixCount = value.count - practiceTrigger.count
            let prefix = String(value.prefix(max(0, prefixCount)))
            return PracticeResult(text: prefix + practiceExpansion, didExpand: true)
        }
        return PracticeResult(text: value, didExpand: false)
    }

    /// Welcome, then Access only on macOS and the browser preview, then Import and Try it.
    static func steps(for platform: Platform) -> [Step] {
        var list: [Step] = [.welcome]
        if platform == .macos || platform == .web {
            list.append(.permissions)
        }
        list.append(contentsOf: [.importStep, .practice])
        return list
    }

    static func index(of step: Step, in steps: [Step]) -> Int {
        steps.firstIndex(of: step) ?? 0
    }

    /// The step `offset` places away, or nil at either end.
    static func neighbour(of step: Step, in steps: [Step], offset: Int) -> Step? {
        let next = index(of: step, in: steps) + offset
        guard steps.indices.contains(next) else { return nil }
        return steps[next]
    }

    /// Snippets a result really added: the source count minus what deduplication merged away.
    static func newSnippetCount(_ summary: ImportSummary) -> Int {
        max(0, summary.importedSnippets - (summary.dedupeReport?.mergedSnippets ?? 0))
    }

    /// One source's latest result. `accumulate` adds to a previous import from the same source.
    struct ImportOutcome: Equatable, Sendable {
        var count: Int
        var changed: Int
        var firstChange: String?
    }

    static func recording(_ summary: ImportSummary, onto existing: ImportOutcome?, accumulate: Bool) -> ImportOutcome {
        let added = newSnippetCount(summary)
        let before = accumulate ? existing : nil
        return ImportOutcome(
            count: (before?.count ?? 0) + added,
            changed: (before?.changed ?? 0) + summary.warnings.count,
            firstChange: before?.firstChange ?? summary.warnings.first?.message
        )
    }

    /// The Espanso row's error after a detected folder fails. `completed` counts every folder
    /// imported so far, earlier attempts included (`completedImports.current.size`).
    static func espansoFailure(_ message: String, completed: Int, total: Int) -> String {
        completed > 0 ? "Imported \(completed) of \(total) folders; retry brings in the rest. \(message)" : message
    }
}
