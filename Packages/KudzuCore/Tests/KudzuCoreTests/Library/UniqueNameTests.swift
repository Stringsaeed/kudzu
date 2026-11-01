import Testing
@testable import KudzuCore

@Suite("UniqueName")
struct UniqueNameTests {
    @Test func returnsBaseWhenFree() {
        #expect(UniqueName.make(base: "New Snippet", existing: ["Other"]) == "New Snippet")
    }

    @Test func numbersFromTwo() {
        #expect(UniqueName.make(base: "New Snippet", existing: ["New Snippet"]) == "New Snippet 2")
        #expect(UniqueName.make(base: "New Snippet", existing: ["New Snippet", "New Snippet 2", "new snippet 3"]) == "New Snippet 4")
    }

    @Test func comparisonIgnoresCase() {
        #expect(UniqueName.make(base: "Foo", existing: ["FOO"]) == "Foo 2")
    }

    @Test func fillsFirstGap() {
        #expect(UniqueName.make(base: "A", existing: ["A", "A 3"]) == "A 2")
    }

    @Test func triggersNumberWithoutSpace() {
        #expect(UniqueName.makeTrigger(base: ";snippet", existing: [";snippet"]) == ";snippet2")
        #expect(UniqueName.makeTrigger(base: ";snippet", existing: [";SNIPPET", ";snippet2"]) == ";snippet3")
        #expect(UniqueName.makeTrigger(base: ";snippet", existing: []) == ";snippet")
    }

    @Test func nameFromContentUsesFirstNonEmptyLine() {
        #expect(UniqueName.name(fromContent: "\n\n  Hello world \nsecond", fallback: "X") == "Hello world")
    }

    @Test func nameFromContentTruncatesAndFallsBack() {
        let long = String(repeating: "a", count: 100)
        let name = UniqueName.name(fromContent: long, fallback: "X", limit: 10)
        #expect(name == String(repeating: "a", count: 10) + "…")
        #expect(UniqueName.name(fromContent: "  \n ", fallback: "Fallback") == "Fallback")
    }
}
