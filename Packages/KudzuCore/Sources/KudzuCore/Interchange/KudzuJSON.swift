import Foundation

/// Errors thrown when encoding or decoding a Kudzu JSON library.
public enum KudzuJSONError: Error, LocalizedError, Equatable, Sendable {
    case unsupportedFormatVersion(found: Int, maxSupported: Int)
    case decodingFailed(String)

    public var errorDescription: String? {
        switch self {
        case .unsupportedFormatVersion(let found, let maxSupported):
            return "Unsupported Kudzu library format version \(found). Maximum supported version is \(maxSupported)."
        case .decodingFailed(let detail):
            return "Failed to decode Kudzu library: \(detail)"
        }
    }
}

/// Encodes and decodes `KudzuLibrary` values as deterministic, human-readable JSON.
public enum KudzuJSON {
    /// Encodes a library as pretty-printed JSON with sorted keys and ISO-8601 dates.
    public static func encode(_ library: KudzuLibrary) throws -> Data {
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.prettyPrinted, .sortedKeys]
        encoder.dateEncodingStrategy = .iso8601
        return try encoder.encode(library)
    }

    /// Decodes a library from JSON data, rejecting format versions newer than `KudzuLibrary.currentFormatVersion`.
    public static func decode(_ data: Data) throws -> KudzuLibrary {
        struct VersionProbe: Decodable {
            var formatVersion: Int
        }
        let decoder = JSONDecoder()
        decoder.dateDecodingStrategy = .iso8601

        let probe = try decoder.decode(VersionProbe.self, from: data)
        if probe.formatVersion > KudzuLibrary.currentFormatVersion {
            throw KudzuJSONError.unsupportedFormatVersion(
                found: probe.formatVersion,
                maxSupported: KudzuLibrary.currentFormatVersion
            )
        }
        return try decoder.decode(KudzuLibrary.self, from: data)
    }
}
