import Foundation
import Testing
@testable import KudzuCore

@Test func libraryRoundTripsThroughJSON() throws {
    let group = GroupRecord(name: "Work")
    let snippet = SnippetRecord(name: "Signature", triggers: [";sig"], content: "Best regards", groupID: group.id)
    let library = KudzuLibrary(groups: [group], snippets: [snippet])

    let data = try JSONEncoder().encode(library)
    let decoded = try JSONDecoder().decode(KudzuLibrary.self, from: data)

    #expect(decoded == library)
}
