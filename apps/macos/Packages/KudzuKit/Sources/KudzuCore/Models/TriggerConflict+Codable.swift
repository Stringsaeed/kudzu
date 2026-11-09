import Foundation

// `TriggerConflict` itself is declared with the conflict finder in `Matching/TriggerConflicts.swift`
// (T1). This adds the JSON contract of `kudzu-core/src/matching/mod.rs`: `kind` is
// `"duplicate"`/`"shadowing"`, ids are `snippetId`/`otherSnippetId`, lowercase.

extension TriggerConflict.Kind: Codable {
    public init(from decoder: any Decoder) throws {
        let c = try decoder.singleValueContainer()
        switch try c.decode(String.self) {
        case "duplicate": self = .duplicate
        case "shadowing": self = .shadowing
        case let other: throw DecodingError.dataCorruptedError(in: c, debugDescription: "unknown conflict kind \(other)")
        }
    }

    public func encode(to encoder: any Encoder) throws {
        var c = encoder.singleValueContainer()
        switch self {
        case .duplicate: try c.encode("duplicate")
        case .shadowing: try c.encode("shadowing")
        }
    }
}

extension TriggerConflict: Codable {
    private enum CodingKeys: String, CodingKey { case kind, snippetId, trigger, otherSnippetId, otherTrigger }

    public init(from decoder: any Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        self.init(
            kind: try c.decode(Kind.self, forKey: .kind),
            snippetID: try c.decode(UUID.self, forKey: .snippetId),
            trigger: try c.decode(String.self, forKey: .trigger),
            otherSnippetID: try c.decode(UUID.self, forKey: .otherSnippetId),
            otherTrigger: try c.decode(String.self, forKey: .otherTrigger)
        )
    }

    public func encode(to encoder: any Encoder) throws {
        var c = encoder.container(keyedBy: CodingKeys.self)
        try c.encode(kind, forKey: .kind)
        try c.encodeUUID(snippetID, forKey: .snippetId)
        try c.encode(trigger, forKey: .trigger)
        try c.encodeUUID(otherSnippetID, forKey: .otherSnippetId)
        try c.encode(otherTrigger, forKey: .otherTrigger)
    }
}
