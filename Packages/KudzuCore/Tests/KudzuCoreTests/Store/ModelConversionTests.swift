import Foundation
import SwiftData
import Testing
@testable import KudzuCore

@MainActor
@Suite struct ModelConversionTests {
    @Test func snippetRoundTripsThroughModel() throws {
        let store = try KudzuStore.inMemory()
        let group = GroupRecord(name: "Work", sortIndex: 3, isEnabled: false, insertion: .paste,
                                rule: AppRule(mode: .deny, bundleIDs: ["com.agilebits.onepassword7"]))
        let snippet = SnippetRecord(
            name: "Sig", triggers: [";sig", ";Sig2", "with\nnewline"], content: "Best\n{{clipboard}}",
            groupID: group.id, fireMode: .wholeWord, caseMode: .exact, insertion: .type,
            rule: AppRule(mode: .allow, bundleIDs: ["com.apple.mail", "com.tinyspeck.slackmacgap"], insertion: .paste),
            isFavorite: true, isEnabled: false,
            createdAt: Date(timeIntervalSinceReferenceDate: 123.456), updatedAt: Date(timeIntervalSinceReferenceDate: 789.012)
        )
        try store.upsert(group)
        try store.upsert(snippet)

        let library = try store.library()
        #expect(library.groups == [group])
        #expect(library.snippets == [snippet])
    }

    @Test func modelInitAndRecordAreInverse() {
        let record = SnippetRecord(name: "n", triggers: ["a"], content: "c", insertion: .paste)
        let model = SnippetModel(record)
        #expect(model.record == record)
        var changed = record
        changed.content = "d"
        changed.triggers = ["b", "c"]
        changed.rule = nil
        model.update(from: changed)
        #expect(model.record == changed)

        let group = GroupRecord(name: "g", sortIndex: 2, insertion: .type)
        #expect(GroupModel(group).record == group)
        let usage = UsageRecord(snippetID: UUID(), deviceID: "d", count: 2, charsSaved: 9, lastUsedAt: .now)
        #expect(UsageModel(usage).record == usage)
    }

    @Test func unknownStoredEnumValuesFallBackToDefaults() {
        let model = SnippetModel(SnippetRecord(name: "n", triggers: ["x"], content: "c"))
        model.fireModeRaw = "onTheFullMoon"
        model.caseModeRaw = "sarcastic"
        model.insertionRaw = "telepathy"
        model.ruleJSON = #"{"mode":"sometimes","bundleIDs":["a"]}"#
        #expect(model.fireMode == .immediate)
        #expect(model.caseMode == .propagate)
        #expect(model.insertion == nil)
        #expect(model.rule == nil)

        // Extra keys from a newer client and an unknown insertion still decode the rest.
        model.ruleJSON = #"{"mode":"deny","bundleIDs":["a"],"insertion":"telepathy","schedule":"weekdays"}"#
        #expect(model.rule == AppRule(mode: .deny, bundleIDs: ["a"], insertion: nil))

        model.triggersJSON = "not json"
        #expect(model.triggers == [])
    }
}
