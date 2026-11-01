import Testing
@testable import KudzuCore

@Suite("TriggerValidator")
struct TriggerValidatorTests {
    @Test func acceptsAndTrimsPlainTrigger() {
        #expect(TriggerValidator.validate("  ;sig ", existing: []) == .success(";sig"))
    }

    @Test func rejectsEmpty() {
        #expect(TriggerValidator.validate("", existing: []) == .failure(.empty))
    }

    @Test func rejectsWhitespaceOnly() {
        #expect(TriggerValidator.validate("   ", existing: []) == .failure(.whitespaceOnly))
        #expect(TriggerValidator.validate("\t\n", existing: []) == .failure(.whitespaceOnly))
    }

    @Test func rejectsDuplicatesCaseInsensitivelyByDefault() {
        #expect(TriggerValidator.validate(";SIG", existing: [";sig"]) == .failure(.duplicate))
        #expect(TriggerValidator.validate(";SIG", existing: [";sig"], caseSensitive: true) == .success(";SIG"))
        #expect(TriggerValidator.validate(";sig", existing: [";sig"], caseSensitive: true) == .failure(.duplicate))
    }

    @Test func lengthLimitIsTheKeyBufferCapacity() {
        #expect(TriggerValidator.maxLength == KeyBuffer.defaultCapacity)
        let ok = String(repeating: "a", count: TriggerValidator.maxLength)
        #expect(TriggerValidator.validate(ok, existing: []) == .success(ok))
        let tooLong = ok + "a"
        #expect(TriggerValidator.validate(tooLong, existing: []) == .failure(.tooLong(length: 65)))
    }

    @Test func lengthCountsGraphemeClustersNotScalars() {
        // 64 family emoji are 64 characters (but many scalars): allowed.
        let family = String(repeating: "👨‍👩‍👧", count: TriggerValidator.maxLength)
        #expect(TriggerValidator.validate(family, existing: []) == .success(family))
    }

    @Test func messagesAreNonEmpty() {
        for issue in [TriggerIssue.empty, .whitespaceOnly, .duplicate, .tooLong(length: 70)] {
            #expect(!issue.message.isEmpty)
        }
        #expect(TriggerIssue.tooLong(length: 70).message.contains("70"))
    }

    @Test func tokensSplitOnAnyWhitespace() {
        #expect(TriggerValidator.tokens(from: " a  b\tc\nd ") == ["a", "b", "c", "d"])
        #expect(TriggerValidator.tokens(from: "   ").isEmpty)
    }
}
