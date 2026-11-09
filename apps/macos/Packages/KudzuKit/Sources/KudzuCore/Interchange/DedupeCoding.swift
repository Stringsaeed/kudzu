import Foundation

extension Dedupe.UsageMerge: Codable {
    enum CodingKeys: String, CodingKey { case snippetID = "snippetId", deviceID = "deviceId", sourceSnippetIDs = "sourceSnippetIds", count, charsSaved, lastUsedAt }
    public init(from decoder: any Decoder) throws {
        let c = try decoder.container(keyedBy:CodingKeys.self)
        snippetID = try c.decode(UUID.self,forKey:.snippetID); deviceID = try c.decode(String.self,forKey:.deviceID)
        sourceSnippetIDs = try c.decode([UUID].self,forKey:.sourceSnippetIDs); count = try c.decode(Int.self,forKey:.count); charsSaved = try c.decode(Int.self,forKey:.charsSaved)
        lastUsedAt = try c.decodeIfPresent(Date.self,forKey:.lastUsedAt)
    }
    public func encode(to encoder: any Encoder) throws {
        var c = encoder.container(keyedBy:CodingKeys.self)
        try c.encodeUUID(snippetID,forKey:.snippetID); try c.encode(deviceID,forKey:.deviceID); try c.encodeUUIDs(sourceSnippetIDs,forKey:.sourceSnippetIDs)
        try c.encode(count,forKey:.count); try c.encode(charsSaved,forKey:.charsSaved)
        if let lastUsedAt { try c.encodeWholeSeconds(lastUsedAt,forKey:.lastUsedAt) } else { try c.encodeNil(forKey:.lastUsedAt) }
    }
}
extension Dedupe.Plan: Codable {
    enum CodingKeys: CodingKey { case groupMerges, snippetMerges, updatedGroups, updatedSnippets, usageMerges, conflicts }
    public init(from decoder: any Decoder) throws {
        let c = try decoder.container(keyedBy:CodingKeys.self)
        func map(_ key: CodingKeys) throws -> [UUID:UUID] {
            let values = try c.decode([String:String].self,forKey:key); var out: [UUID:UUID] = [:]
            for (a,b) in values { guard let a = UUID(uuidString:a), let b = UUID(uuidString:b) else { throw DecodingError.dataCorruptedError(forKey:key,in:c,debugDescription:"invalid UUID") }; out[a] = b }; return out
        }
        groupMerges = try map(.groupMerges); snippetMerges = try map(.snippetMerges)
        updatedGroups = try c.decode([Group].self,forKey:.updatedGroups); updatedSnippets = try c.decode([Snippet].self,forKey:.updatedSnippets)
        usageMerges = try c.decode([Dedupe.UsageMerge].self,forKey:.usageMerges); conflicts = try c.decode([DedupeConflict].self,forKey:.conflicts)
    }
    public func encode(to encoder: any Encoder) throws {
        var c = encoder.container(keyedBy:CodingKeys.self)
        func map(_ values: [UUID:UUID]) -> [String:String] { Dictionary(uniqueKeysWithValues:values.map { ($0.key.lowercased,$0.value.lowercased) }) }
        try c.encode(map(groupMerges),forKey:.groupMerges); try c.encode(map(snippetMerges),forKey:.snippetMerges)
        try c.encode(updatedGroups,forKey:.updatedGroups); try c.encode(updatedSnippets,forKey:.updatedSnippets); try c.encode(usageMerges,forKey:.usageMerges); try c.encode(conflicts,forKey:.conflicts)
    }
}
extension InterchangeStats.SnippetStats: Codable {
    enum CodingKeys: String, CodingKey { case snippetID = "snippetId", count, charsSaved, lastUsedAt }
    public init(from decoder: any Decoder) throws {
        let c = try decoder.container(keyedBy:CodingKeys.self); snippetID = try c.decode(UUID.self,forKey:.snippetID); count = try c.decode(Int.self,forKey:.count); charsSaved = try c.decode(Int.self,forKey:.charsSaved); lastUsedAt = try c.decodeIfPresent(Date.self,forKey:.lastUsedAt)
    }
    public func encode(to encoder: any Encoder) throws {
        var c = encoder.container(keyedBy:CodingKeys.self); try c.encodeUUID(snippetID,forKey:.snippetID); try c.encode(count,forKey:.count); try c.encode(charsSaved,forKey:.charsSaved)
        if let lastUsedAt { try c.encodeWholeSeconds(lastUsedAt,forKey:.lastUsedAt) } else { try c.encodeNil(forKey:.lastUsedAt) }
    }
}
