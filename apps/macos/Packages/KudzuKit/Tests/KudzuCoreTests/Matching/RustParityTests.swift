import Foundation
import Testing
@testable import KudzuCore

// The Rust port combined these original Swift cases. Keep its test names and all original assertions.
extension KeyBufferTests {
    @Test func bufferAppendsBackspacesAndResets() {
        appendsCharactersInOrder()
        backspaceDropsLastCharacter()
        backspaceOnEmptyBufferIsNoOp()
        resetClearsEverything()
        emptyStringInputIsIgnored()
    }
    @Test func bufferCapsAtCapacityKeepingNewest() {
        capsAtCapacityKeepingNewestCharacters()
        defaultCapacityIs64()
    }
    @Test func bufferHoldsGraphemeClusters() {
        multiScalarInputIsOneGraphemeCluster()
        combiningMarkTypedSeparatelyJoinsPreviousCharacter()
        regionalIndicatorsPairIntoFlags()
    }
    @Test func bufferCopiesAreIndependent() {
        copiesAreIndependent()
    }
}
extension UndoTrackerTests {
    @Test func undoTrackerBasics() {
        backspaceRightAfterImmediateExpansionUndoes()
        wholeWordUndoAlsoRemovesAndRestoresSeparator()
        nothingPendingMeansNoUndo()
    }
    @Test func undoIsOneShotReplaceableAndClearable() {
        undoIsOneShot()
        newExpansionReplacesPendingOne()
        clearDropsPending()
        zeroInsertedCountStillRestoresTrigger()
    }
}
extension ExpansionMatcherTests {
    @Test func expandsOnTriggerAndIsQuietOtherwise() {
        expandsOnTrigger()
        noEventWhenNothingMatches()
    }
    @Test func backspaceAndResetMidTrigger() {
        backspaceMidTriggerThroughFacade()
        resetMidTriggerThroughFacade()
    }
    @Test func undoThenNormalTypingContinues() {
        undoAfterExpansionThenNormalTypingContinues()
    }
    @Test func replaceIndexSwapsIndexAndClearsUndo() {
        reloadSwapsIndexAndClearsUndo()
    }
}
extension TriggerConflictsTests {
    @Test func duplicateCaseRules() {
        duplicateIsCaseInsensitiveWhenEitherSidePropagates()
        exactTriggersDifferingByCaseDoNotConflict()
        exactTriggersWithSameCaseConflict()
        sameSnippetRepeatingATriggerIsNotAConflict()
    }
    @Test func shadowingFireModeRules() {
        shadowingAppliesToWholeWordLongerTrigger()
        wholeWordPrefixDoesNotShadow()
    }
    @Test func shadowingCaseRules() {
        shadowingIsCaseInsensitiveWhenShadowerPropagates()
        exactShadowerDoesNotShadowPropagatingLongerTriggerWithCasedPrefix()
        exactShadowerDoesShadowWhenPrefixHasNoCase()
        exactShadowsExactOnlyWithMatchingCase()
    }
    @Test func prefixWithinSameSnippetAndEqualTriggers() {
        prefixWithinSameSnippetIsIgnored()
        equalTriggersReportAsDuplicateNotShadowing()
    }
    @Test func ruleOverlap() {
        disjointAllowListsDoNotConflict()
        allowListInsideDenyListDoesNotConflict()
        groupRulesAreInheritedForOverlapCheck()
    }
    @Test func conflictsScaleToManySnippets() {
        scalesToManySnippets()
    }
}
extension EffectiveSettingsTests {
    @Test func ungroupedSnippetHasNoRestrictions() {
        ungroupedSnippetHasNoRestrictionsAndDefaultInsertion()
    }
    @Test func ruleSummaries() {
        allowRuleDescription()
        emptyRuleDescriptions()
    }
}
extension TriggerIndexFiringTests {
    @Test func immediateDoesNotFireOnPartialOrLaterKeystroke() {
        immediateDoesNotFireOnPartialTrigger()
        immediateDoesNotFireOnLaterKeystroke()
    }
    @Test func nonCharacterLastInputAndEmptyBufferNeverMatch() {
        nonCharacterLastInputNeverMatches()
        emptyBufferNeverMatches()
    }
    @Test func multiScalarInputCharactersMatchCanonically() throws {
        try multiScalarInputCharactersMatch()
    }
    @Test func propagateMatchesCaseInsensitivelyAndReportsStyle() throws {
        try propagateMatchesCaseInsensitively()
        try propagateReportsCaseStyle()
    }
}
extension TriggerIndexRuleTests {
    @Test func disabledSnippetsAndGroupsNeverFire() {
        disabledSnippetNeverFires()
        disabledParentGroupDisablesSnippet()
        disabledGrandparentGroupDisablesSnippet()
    }
    @Test func enabledChainAndSiblingGroups() {
        enabledChainFires()
        disabledLeafGroupOnlyAffectsItsOwnSnippets()
    }
    @Test func groupCyclesDoNotHangAndStillHonourEnablement() {
        groupCycleDoesNotHangAndStillHonoursEnablement()
        groupSelfParentCycle()
        unknownGroupIDTreatedAsUngrouped()
    }
    @Test func allowAndDenyRules() {
        allowRuleMatchesOnlyListedApps()
        denyRuleMatchesEverywhereExceptListedApps()
    }
    @Test func nearestAncestorGroupRuleWinsAndIsInherited() {
        nearestAncestorGroupRuleWins()
        ruleInheritedThroughGroupWithoutRule()
    }
    @Test func ruleInsertionOverride() throws {
        try ruleInsertionOverrideAppliesWhenAppListed()
        try ruleInsertionOverrideInheritedFromGroupRule()
        try ruleInsertionOverrideNotAppliedToUnlistedApp()
        try ruleWithoutInsertionLeavesInsertionAlone()
    }
}

@Test func duplicateTriggersInOneSnippetAreIndexedOnce() {
    #expect(makeIndex([snip([";a",";A"])]).triggerCount == 1)
    #expect(makeIndex([snip([";a",";A"],case:.exact)]).triggerCount == 2)
}
@Test func resetClearsBufferAndUndo() {
    var m = ExpansionMatcher(snippets:[snip([";sig"])],groups:[],config:MatchingConfig())
    for c in ";sig" { if case .expand(let match) = m.handle(.character(String(c)),bundleID:nil) { m.didExpand(match:match,insertedCount:3) } }
    m.reset(); #expect(m.handle(.backspace,bundleID:nil) == nil)
    for c in "xy" { _ = m.handle(.character(String(c)),bundleID:nil) }
    #expect(m.buffer.string == "xy"); m.reset(); #expect(m.buffer.string == "")
}
@Test func validatesTriggers() {
    #expect(validateTrigger("  ;sig ",existing:[]) == .success(";sig"))
    #expect(validateTrigger("",existing:[]) == .failure(.empty))
    for input in ["   ","\t\n"] { #expect(validateTrigger(input,existing:[]) == .failure(.whitespaceOnly)) }
    #expect(validateTrigger(";SIG",existing:[";sig"]) == .failure(.duplicate))
    #expect(validateTrigger(";SIG",existing:[";sig"],caseSensitive:true) == .success(";SIG"))
    #expect(validateTrigger(";sig",existing:[";sig"],caseSensitive:true) == .failure(.duplicate))
}
@Test func lengthLimitIsTheKeyBufferCapacityInClusters() {
    #expect(TriggerValidator.maxLength == 64)
    let ok = String(repeating:"a",count:64), family = String(repeating:"👨‍👩‍👧",count:64)
    #expect(validateTrigger(ok,existing:[]) == .success(ok))
    #expect(validateTrigger(ok+"a",existing:[]) == .failure(.tooLong(length:65)))
    #expect(validateTrigger(family,existing:[]) == .success(family))
}
@Test func issueMessagesAndTokens() {
    for issue in [TriggerIssue.empty,.whitespaceOnly,.duplicate,.tooLong(length:70)] { #expect(!issue.message.isEmpty) }
    #expect(TriggerIssue.tooLong(length:70).message.contains("70"))
    #expect(triggerTokens(" a  b\tc\nd ") == ["a","b","c","d"])
    #expect(triggerTokens("   ").isEmpty)
}
