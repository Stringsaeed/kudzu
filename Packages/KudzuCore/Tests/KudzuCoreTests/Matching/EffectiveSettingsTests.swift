import Foundation
import Testing
@testable import KudzuCore

@Suite("EffectiveSettings & GroupResolver")
struct EffectiveSettingsTests {
    private func makeGroup(
        _ name: String,
        parent: UUID? = nil,
        enabled: Bool = true,
        rule: AppRule? = nil,
        insertion: InsertionMethod? = nil,
        id: UUID = UUID()
    ) -> GroupRecord {
        GroupRecord(id: id, name: name, parentID: parent, isEnabled: enabled, insertion: insertion, rule: rule)
    }

    @Test func ungroupedSnippetHasNoRestrictionsAndDefaultInsertion() {
        let resolver = GroupResolver(groups: [])
        let settings = resolver.settings(forGroup: nil)
        #expect(settings.isEnabled)
        #expect(settings.rule == nil)
        #expect(settings.ruleGroup == nil)
        #expect(settings.insertion == nil)
        #expect(settings.insertionGroup == nil)
        #expect(settings.ruleDescription() == "No restrictions")
        #expect(settings.insertionDescription() == "Default (Automatic, or the app's setting in Settings → Apps)")
        #expect(settings.insertionDescription(defaultInsertion: .paste) == "Default (Paste, or the app's setting in Settings → Apps)")
    }

    @Test func inheritsRuleAndInsertionWithSourceGroupNames() {
        let workRule = AppRule(mode: .deny, bundleIDs: ["com.tinyspeck.slackmacgap", "com.apple.mail"])
        let work = makeGroup("Work", rule: workRule, insertion: .paste)
        let email = makeGroup("Email", parent: work.id)
        let resolver = GroupResolver(groups: [work, email])

        let names: [String: String] = [
            "com.tinyspeck.slackmacgap": "Slack",
            "com.apple.mail": "Mail",
        ]
        let inherited = resolver.settings(forGroup: email.id)
        #expect(inherited.rule == workRule)
        #expect(inherited.ruleGroup?.id == work.id)
        #expect(inherited.insertion == .paste)
        #expect(inherited.insertionGroup?.id == work.id)
        #expect(inherited.ruleDescription { names[$0] ?? $0 } == "Inherited from group 'Work': Block in Slack, Mail")
        #expect(inherited.insertionDescription() == "Inherited from group 'Work': Paste")
    }

    @Test func allowRuleDescription() {
        let codingRule = AppRule(mode: .allow, bundleIDs: ["com.apple.Terminal", "com.mitchellh.ghostty"])
        let coding = makeGroup("Coding", rule: codingRule)
        let resolver = GroupResolver(groups: [coding])
        let names = ["com.apple.Terminal": "Terminal", "com.mitchellh.ghostty": "Ghostty"]
        let inherited = resolver.settings(forGroup: coding.id)
        #expect(inherited.ruleDescription { names[$0] ?? $0 } == "Inherited from group 'Coding': Allow only in Terminal, Ghostty")
    }

    @Test func emptyRuleDescriptions() {
        let emptyAllow = AppRule(mode: .allow, bundleIDs: [])
        let emptyDeny = AppRule(mode: .deny, bundleIDs: [])
        #expect(emptyAllow.summary() == "Disabled everywhere")
        #expect(emptyDeny.summary() == "No restrictions")
    }

    @Test func nearestAncestorWinsIndependentlyForRuleAndInsertion() {
        let outerRule = AppRule(mode: .deny, bundleIDs: ["com.outer"])
        let innerRule = AppRule(mode: .allow, bundleIDs: ["com.inner"])
        let top = makeGroup("Top", rule: outerRule, insertion: .paste)
        let mid = makeGroup("Mid", parent: top.id, rule: innerRule)
        let leaf = makeGroup("Leaf", parent: mid.id)
        let resolver = GroupResolver(groups: [top, mid, leaf])

        let settings = resolver.settings(forGroup: leaf.id)
        #expect(settings.rule == innerRule)
        #expect(settings.ruleGroup?.name == "Mid")
        #expect(settings.insertion == .paste)
        #expect(settings.insertionGroup?.name == "Top")
    }

    @Test func groupItselfInheritsFromParentChainNotItself() {
        let parentRule = AppRule(mode: .deny, bundleIDs: ["com.parent"])
        let ownRule = AppRule(mode: .allow, bundleIDs: ["com.own"])
        let parent = makeGroup("Parent", rule: parentRule, insertion: .type)
        let child = makeGroup("Child", parent: parent.id, rule: ownRule, insertion: .paste)
        let resolver = GroupResolver(groups: [parent, child])

        // A snippet inside `child` sees `child`'s own settings.
        let forSnippet = resolver.settings(forGroup: child.id)
        #expect(forSnippet.rule == ownRule)
        #expect(forSnippet.ruleGroup?.id == child.id)
        #expect(forSnippet.insertion == .paste)

        // Editing `child` itself sees what `child` would inherit from `parent` if cleared.
        let forChildGroup = resolver.inheritedSettings(forGroupItself: child.id)
        #expect(forChildGroup.rule == parentRule)
        #expect(forChildGroup.ruleGroup?.id == parent.id)
        #expect(forChildGroup.insertion == .type)
        #expect(forChildGroup.insertionGroup?.id == parent.id)

        // Top-level `parent` has no parent group to inherit from.
        let forTopGroup = resolver.inheritedSettings(forGroupItself: parent.id)
        #expect(forTopGroup.rule == nil)
        #expect(forTopGroup.insertion == nil)
    }

    @Test func groupItselfInheritanceHandlesCycles() {
        let aID = UUID(), bID = UUID()
        let a = makeGroup("A", parent: bID, rule: AppRule(mode: .allow, bundleIDs: ["com.a"]), id: aID)
        let b = makeGroup("B", parent: aID, insertion: .paste, id: bID)
        let resolver = GroupResolver(groups: [a, b])

        let forA = resolver.inheritedSettings(forGroupItself: aID)
        #expect(forA.rule == nil, "A must not inherit its own rule via the cycle")
        #expect(forA.insertion == .paste)
        #expect(forA.insertionGroup?.id == bID)
    }

    @Test func effectiveSnippetPrecedence() {
        let groupRule = AppRule(mode: .allow, bundleIDs: ["com.term"], insertion: .paste)
        let g = makeGroup("G", rule: groupRule, insertion: .type)
        let resolver = GroupResolver(groups: [g])
        let config = MatchingConfig(
            defaultInsertion: .automatic,
            appInsertions: ["com.term": .automatic, "com.other": .paste]
        )

        let inheritedSnippet = SnippetRecord(name: "S1", triggers: [";1"], content: "x", groupID: g.id)
        let eff1 = EffectiveSnippet(inheritedSnippet, groups: resolver)
        #expect(eff1.ruleGroup?.id == g.id)
        #expect(eff1.insertionGroup?.id == g.id)
        // 1. AppRule.insertion wins for listed app
        #expect(eff1.insertion(for: "com.term", config: config) == .paste)

        // Without AppRule.insertion, group insertion beats global app override
        let gNoRuleInsertion = makeGroup("G2", insertion: .type)
        let resolver2 = GroupResolver(groups: [gNoRuleInsertion])
        let eff2 = EffectiveSnippet(
            SnippetRecord(name: "S2", triggers: [";2"], content: "x", groupID: gNoRuleInsertion.id),
            groups: resolver2
        )
        #expect(eff2.insertion(for: "com.other", config: config) == .type)

        // Ungrouped snippet with no insertion uses global per-app override, then defaultInsertion
        let eff3 = EffectiveSnippet(
            SnippetRecord(name: "S3", triggers: [";3"], content: "x"),
            groups: resolver2
        )
        #expect(eff3.insertion(for: "com.other", config: config) == .paste)
        #expect(eff3.insertion(for: "com.unlisted", config: config) == .automatic)
    }
}
