import Foundation
import Testing
@testable import KudzuCore

// Replays `matching-scenarios.json` and `matching-conflicts.json` (export_fixtures.rs, matching
// section): several snippets with groups, app rules and configs, Unicode clusters and separators.

private struct ScenarioCase: Decodable {
    struct Config: Decodable {
        var separators: String?
        var deniedBundleIDs: [String]?
        var appInsertions: [String: InsertionMethod]?
        var defaultInsertion: InsertionMethod?

        /// Rust builds the separator set from `char`s (Unicode scalars), so a decomposed `é` is two separators.
        var matching: MatchingConfig {
            var config = MatchingConfig()
            if let separators { config.separators = Set(separators.unicodeScalars.map(Character.init)) }
            if let deniedBundleIDs { config.deniedBundleIDs = Set(deniedBundleIDs) }
            if let appInsertions { config.appInsertions = appInsertions }
            if let defaultInsertion { config.defaultInsertion = defaultInsertion }
            return config
        }
    }
    struct Step: Decodable {
        struct Event: Decodable {
            var kind: String
            var snippetID: UUID?
            var trigger: String?
            var typedTrigger: String?
            var deleteCount: Int
            var deleteCountIncludingSeparator: Int?
            var separator: String?
            var insertion: InsertionMethod?
            var caseStyle: CaseStyle?
            var fireMode: FireMode?
            var retype: String?
        }
        var key: String
        var event: Event?
        var buffer: String
    }
    var name: String
    var snippets: [Snippet]
    var groups: [Group]
    var config: Config
    var bundleID: String?
    var insertedCount: Int
    var steps: [Step]
}

private struct ConflictCase: Decodable {
    var snippets: [Snippet]
    var groups: [Group]
    var expected: [TriggerConflict]
}

/// Byte equality: `String ==` is canonical equivalence and would hide NFC/NFD differences.
private func same(_ a: String?, _ b: String?) -> Bool {
    switch (a, b) {
    case (nil, nil): true
    case let (a?, b?): a.utf8.elementsEqual(b.utf8)
    default: false
    }
}

@Test func matchingScenarioFixtures() throws {
    let cases = try loadT1Fixture("matching-scenarios.json", as: [ScenarioCase].self)
    #expect(cases.count > 20)
    for c in cases {
        let index = TriggerIndex(snippets: c.snippets, groups: c.groups, config: c.config.matching)
        var matcher = ExpansionMatcher(index: index)
        for (n, step) in c.steps.enumerated() {
            let label = "\(c.name) [\(c.bundleID ?? "nil")] step \(n) key \(step.key.debugDescription)"
            let input: KeyInput = step.key == "RESET" ? .reset : step.key == "BACKSPACE" ? .backspace : .character(step.key)
            switch matcher.handle(input, bundleID: c.bundleID) {
            case .expand(let match):
                let e = step.event
                #expect(e?.kind == "expand", "\(label): unexpected expansion of \(match.trigger.debugDescription)")
                #expect(e?.snippetID == match.snippetID && same(e?.trigger, match.trigger) && same(e?.typedTrigger, match.typedTrigger), "\(label)")
                #expect(e?.deleteCount == match.deleteCount && e?.deleteCountIncludingSeparator == match.deleteCountIncludingSeparator && same(e?.separator, match.separator), "\(label)")
                #expect(e?.insertion == match.insertion && e?.caseStyle == match.caseStyle && e?.fireMode == match.fireMode, "\(label)")
                matcher.didExpand(match: match, insertedCount: c.insertedCount)
            case .undo(let undo):
                #expect(step.event?.kind == "undo" && step.event?.deleteCount == undo.deleteCount && same(step.event?.retype, undo.retype), "\(label)")
            case nil:
                #expect(step.event == nil, "\(label): expected \(step.event?.kind ?? "") \(step.event?.trigger ?? "")")
            }
            #expect(same(matcher.buffer.string, step.buffer), "\(label): buffer")
        }
    }
}

@Test func matchingConflictFixtures() throws {
    let cases = try loadT1Fixture("matching-conflicts.json", as: [ConflictCase].self)
    #expect(!cases.isEmpty)
    for (n, c) in cases.enumerated() {
        #expect(findConflicts(snippets: c.snippets, groups: c.groups) == c.expected, "case \(n)")
    }
}

private struct CaseGoldenFile: Decodable {
    struct Detect: Decodable { var typed: String; var style: CaseStyle }
    struct Apply: Decodable { var text: String; var style: CaseStyle; var result: String }
    var detect: [Detect]
    var apply: [Apply]
}

@Test func casePropagationGoldenFixtures() throws {
    let file = try loadT1Fixture("matching-case.json", as: CaseGoldenFile.self)
    for c in file.detect {
        #expect(CasePropagation.detect(typedTrigger: c.typed) == c.style, "detect: \(c.typed.debugDescription)")
    }
    for c in file.apply {
        #expect(same(CasePropagation.apply(c.style, to: c.text), c.result), "apply \(c.style): \(c.text.debugDescription)")
    }
}
