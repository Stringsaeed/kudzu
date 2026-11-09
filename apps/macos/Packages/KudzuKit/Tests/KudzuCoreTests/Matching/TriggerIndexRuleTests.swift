import Foundation
import Testing
@testable import KudzuCore

@Suite("TriggerIndex – enablement, rules, insertion")
struct TriggerIndexRuleTests {
    // MARK: enabled

    func disabledSnippetNeverFires() {
        let index = makeIndex([snip([";x"], enabled: false)])
        #expect(index.triggerCount == 0)
        #expect(type(";x", on: index) == nil)
    }

    func disabledParentGroupDisablesSnippet() {
        let g = group(enabled: false)
        let index = makeIndex([snip([";x"], group: g.id)], groups: [g])
        #expect(type(";x", on: index) == nil)
    }

    func disabledGrandparentGroupDisablesSnippet() {
        let top = group("top", enabled: false)
        let mid = group("mid", parent: top.id)
        let leaf = group("leaf", parent: mid.id)
        let index = makeIndex([snip([";x"], group: leaf.id)], groups: [leaf, mid, top])
        #expect(type(";x", on: index) == nil)
    }

    func enabledChainFires() {
        let top = group("top")
        let leaf = group("leaf", parent: top.id)
        let index = makeIndex([snip([";x"], group: leaf.id)], groups: [leaf, top])
        #expect(type(";x", on: index) != nil)
    }

    func disabledLeafGroupOnlyAffectsItsOwnSnippets() {
        let on = group("on")
        let off = group("off", enabled: false)
        let index = makeIndex(
            [snip([";a"], group: on.id), snip([";b"], group: off.id)],
            groups: [on, off]
        )
        #expect(type(";a", on: index) != nil)
        #expect(type(";b", on: index) == nil)
    }

    func groupCycleDoesNotHangAndStillHonoursEnablement() {
        let aID = UUID(), bID = UUID()
        let a = group("a", parent: bID, enabled: true, id: aID)
        let b = group("b", parent: aID, enabled: true, id: bID)
        let index = makeIndex([snip([";x"], group: aID)], groups: [a, b])
        #expect(type(";x", on: index) != nil)

        let disabledB = group("b", parent: aID, enabled: false, id: bID)
        let index2 = makeIndex([snip([";x"], group: aID)], groups: [a, disabledB])
        #expect(type(";x", on: index2) == nil, "a disabled group inside the cycle still disables")
    }

    func groupSelfParentCycle() {
        let id = UUID()
        let g = group("self", parent: id, id: id)
        let index = makeIndex([snip([";x"], group: id)], groups: [g])
        #expect(type(";x", on: index) != nil)
    }

    func unknownGroupIDTreatedAsUngrouped() {
        let index = makeIndex([snip([";x"], group: UUID())])
        #expect(type(";x", on: index) != nil)
    }

    // MARK: app rules

    func allowRuleMatchesOnlyListedApps() {
        let rule = AppRule(mode: .allow, bundleIDs: ["com.apple.Notes"])
        let index = makeIndex([snip([";x"], rule: rule)])
        #expect(type(";x", on: index, bundleID: "com.apple.Notes") != nil)
        #expect(type(";x", on: index, bundleID: "com.apple.Safari") == nil)
        #expect(type(";x", on: index, bundleID: nil) == nil, "unknown frontmost app cannot satisfy an allow-list")
    }

    func denyRuleMatchesEverywhereExceptListedApps() {
        let rule = AppRule(mode: .deny, bundleIDs: ["com.apple.Terminal"])
        let index = makeIndex([snip([";x"], rule: rule)])
        #expect(type(";x", on: index, bundleID: "com.apple.Terminal") == nil)
        #expect(type(";x", on: index, bundleID: "com.apple.Notes") != nil)
        #expect(type(";x", on: index, bundleID: nil) != nil)
    }

    @Test func snippetRuleOverridesGroupRule() {
        let g = group(rule: AppRule(mode: .allow, bundleIDs: ["com.a"]))
        let own = snip([";own"], group: g.id, rule: AppRule(mode: .allow, bundleIDs: ["com.b"]))
        let inherited = snip([";inh"], group: g.id)
        let index = makeIndex([own, inherited], groups: [g])
        #expect(type(";own", on: index, bundleID: "com.b") != nil)
        #expect(type(";own", on: index, bundleID: "com.a") == nil)
        #expect(type(";inh", on: index, bundleID: "com.a") != nil)
        #expect(type(";inh", on: index, bundleID: "com.b") == nil)
    }

    func nearestAncestorGroupRuleWins() {
        let top = group("top", rule: AppRule(mode: .allow, bundleIDs: ["com.top"]))
        let mid = group("mid", parent: top.id, rule: AppRule(mode: .allow, bundleIDs: ["com.mid"]))
        let leaf = group("leaf", parent: mid.id)
        let index = makeIndex([snip([";x"], group: leaf.id)], groups: [top, mid, leaf])
        #expect(type(";x", on: index, bundleID: "com.mid") != nil)
        #expect(type(";x", on: index, bundleID: "com.top") == nil)
    }

    func ruleInheritedThroughGroupWithoutRule() {
        let top = group("top", rule: AppRule(mode: .deny, bundleIDs: ["com.bad"]))
        let leaf = group("leaf", parent: top.id)
        let index = makeIndex([snip([";x"], group: leaf.id)], groups: [top, leaf])
        #expect(type(";x", on: index, bundleID: "com.bad") == nil)
        #expect(type(";x", on: index, bundleID: "com.ok") != nil)
    }

    @Test func globalDenylistAlwaysApplies() {
        let config = MatchingConfig(deniedBundleIDs: ["com.1password.1password"])
        // Even an explicit allow rule naming the denied app cannot override the global list.
        let rule = AppRule(mode: .allow, bundleIDs: ["com.1password.1password"])
        let index = makeIndex([snip([";x"], rule: rule), snip([";y"])], config: config)
        #expect(type(";x", on: index, bundleID: "com.1password.1password") == nil)
        #expect(type(";y", on: index, bundleID: "com.1password.1password") == nil)
        #expect(type(";y", on: index, bundleID: "com.apple.Notes") != nil)
    }

    @Test func defaultConfigDeniesPasswordManagers() {
        let index = makeIndex([snip([";y"])])
        for id in ["com.1password.1password", "com.bitwarden.desktop", "com.apple.keychainaccess", "com.apple.loginwindow"] {
            #expect(type(";y", on: index, bundleID: id) == nil, "\(id) should be denied by default")
        }
    }

    @Test func ruleExcludedSnippetDoesNotBlockOtherCandidate() throws {
        // The longer trigger is not allowed in this app, so the shorter one must still fire.
        let blocked = snip(["xyz"], rule: AppRule(mode: .allow, bundleIDs: ["com.only"]))
        let ok = snip(["yz"])
        let index = makeIndex([blocked, ok])
        let match = try #require(type("xyz", on: index, bundleID: "com.other"))
        #expect(match.snippetID == ok.id)
    }

    // MARK: insertion

    @Test func insertionFallsBackToConfigDefault() throws {
        let index = makeIndex([snip([";x"])], config: MatchingConfig(defaultInsertion: .paste))
        #expect(try #require(type(";x", on: index)).insertion == .paste)
    }

    @Test func insertionPrecedenceSnippetThenGroupThenConfig() throws {
        let top = group("top", insertion: .type)
        let leaf = group("leaf", parent: top.id)
        let config = MatchingConfig(defaultInsertion: .paste)
        let index = makeIndex(
            [
                snip([";own"], group: leaf.id, insertion: .automatic),
                snip([";grp"], group: leaf.id),
                snip([";cfg"]),
            ],
            groups: [top, leaf],
            config: config
        )
        #expect(try #require(type(";own", on: index)).insertion == .automatic)
        #expect(try #require(type(";grp", on: index)).insertion == .type)
        #expect(try #require(type(";cfg", on: index)).insertion == .paste)
    }

    @Test func nearestGroupInsertionWins() throws {
        let top = group("top", insertion: .type)
        let leaf = group("leaf", parent: top.id, insertion: .paste)
        let index = makeIndex([snip([";x"], group: leaf.id)], groups: [top, leaf])
        #expect(try #require(type(";x", on: index)).insertion == .paste)
    }

    func ruleInsertionOverrideAppliesWhenAppListed() throws {
        let rule = AppRule(mode: .allow, bundleIDs: ["com.term", "com.notes"], insertion: .paste)
        let index = makeIndex([snip([";x"], insertion: .type, rule: rule)])
        #expect(try #require(type(";x", on: index, bundleID: "com.term")).insertion == .paste)
        #expect(try #require(type(";x", on: index, bundleID: "com.notes")).insertion == .paste)
    }

    func ruleInsertionOverrideInheritedFromGroupRule() throws {
        let g = group(rule: AppRule(mode: .allow, bundleIDs: ["com.term"], insertion: .paste))
        let index = makeIndex([snip([";x"], group: g.id)], groups: [g])
        #expect(try #require(type(";x", on: index, bundleID: "com.term")).insertion == .paste)
    }

    func ruleInsertionOverrideNotAppliedToUnlistedApp() throws {
        // A deny rule with an insertion override: unlisted apps keep the normal insertion.
        let rule = AppRule(mode: .deny, bundleIDs: ["com.term"], insertion: .paste)
        let index = makeIndex([snip([";x"], insertion: .type, rule: rule)])
        #expect(try #require(type(";x", on: index, bundleID: "com.notes")).insertion == .type)
    }

    func ruleWithoutInsertionLeavesInsertionAlone() throws {
        let rule = AppRule(mode: .allow, bundleIDs: ["com.term"])
        let index = makeIndex([snip([";x"], insertion: .type, rule: rule)])
        #expect(try #require(type(";x", on: index, bundleID: "com.term")).insertion == .type)
    }

    @Test func defaultConfigUsesPasteForDefaultTerminals() throws {
        let index = makeIndex([snip([";x"])])
        let terminals = [
            "com.apple.Terminal",
            "com.googlecode.iterm2",
            "dev.warp.Warp-Stable",
            "com.mitchellh.ghostty",
            "org.alacritty",
            "net.kovidgoyal.kitty",
            "com.github.wez.wezterm",
        ]
        for bundleID in terminals {
            let match = try #require(type(";x", on: index, bundleID: bundleID))
            #expect(match.insertion == .paste, "\(bundleID) should default to paste")
        }
        #expect(try #require(type(";x", on: index, bundleID: "com.apple.Notes")).insertion == .automatic)
    }

    @Test func globalPerAppInsertionOverrideAppliesBeforeDefault() throws {
        let config = MatchingConfig(
            defaultInsertion: .automatic,
            appInsertions: ["com.custom.app": .type]
        )
        let index = makeIndex([snip([";x"])], config: config)
        #expect(try #require(type(";x", on: index, bundleID: "com.custom.app")).insertion == .type)
        #expect(try #require(type(";x", on: index, bundleID: "com.other.app")).insertion == .automatic)
        #expect(try #require(type(";x", on: index, bundleID: nil)).insertion == .automatic)
    }

    @Test func fullInsertionPrecedenceOrder() throws {
        // Precedence:
        // 1. Matching snippet/group AppRule.insertion
        // 2. snippet.insertion
        // 3. group.insertion
        // 4. global per-app override (config.appInsertions)
        // 5. config.defaultInsertion
        let app = "com.target.app"
        let otherApp = "com.other.app"
        let config = MatchingConfig(
            defaultInsertion: .automatic,
            appInsertions: [app: .paste]
        )

        // Level 1 beats Level 2, 3, 4, 5
        let ruleWithInsertion = AppRule(mode: .allow, bundleIDs: [app], insertion: .type)
        let g1 = group("g1", insertion: .automatic)
        let s1 = snip([";1"], group: g1.id, insertion: .automatic, rule: ruleWithInsertion)

        // Level 2 (snippet.insertion) beats Level 3, 4, 5
        let g2 = group("g2", insertion: .automatic)
        let s2 = snip([";2"], group: g2.id, insertion: .type)

        // Level 3 (group.insertion) beats Level 4, 5
        let g3 = group("g3", insertion: .type)
        let s3 = snip([";3"], group: g3.id)

        // Level 4 (global per-app override) beats Level 5 in `app`, falls back to Level 5 in `otherApp`
        let s4 = snip([";4"])

        let index = makeIndex([s1, s2, s3, s4], groups: [g1, g2, g3], config: config)
        #expect(try #require(type(";1", on: index, bundleID: app)).insertion == .type)
        #expect(try #require(type(";2", on: index, bundleID: app)).insertion == .type)
        #expect(try #require(type(";3", on: index, bundleID: app)).insertion == .type)
        #expect(try #require(type(";4", on: index, bundleID: app)).insertion == .paste)
        #expect(try #require(type(";4", on: index, bundleID: otherApp)).insertion == .automatic)
    }
}
