import Foundation
import KudzuCore

/// The updater feed: Tauri's `latest.json` shape, written by `scripts/release/latest-json.mjs`.
///
/// ```json
/// { "version": "0.6.0", "notes": "…", "pub_date": "2026-…Z",
///   "platforms": { "darwin-aarch64": { "signature": "<base64 .sig>", "url": "…app.tar.gz" },
///                  "darwin-x86_64": { … } } }
/// ```
public struct UpdateFeed: Decodable, Sendable, Equatable {
    public struct Platform: Decodable, Sendable, Equatable {
        public var signature: String
        public var url: URL
    }

    public var version: String
    public var notes: String?
    public var pubDate: String?
    public var platforms: [String: Platform]

    enum CodingKeys: String, CodingKey {
        case version, notes, platforms
        case pubDate = "pub_date"
    }

    /// Feeds: stable is `latest.json` on the newest published release. Beta users read
    /// `beta.json` from the rolling `update-feed` release, which CI keeps pointing at whichever is
    /// newer, the latest beta or the latest stable, so beta users never fall behind stable.
    public static let stableURL = URL(string: "https://github.com/Stringsaeed/kudzu/releases/latest/download/latest.json")!
    public static let betaURL = URL(string: "https://github.com/Stringsaeed/kudzu/releases/download/update-feed/beta.json")!

    public static func url(for channel: UpdateChannel) -> URL {
        switch channel {
        case .stable: stableURL
        case .beta: betaURL
        }
    }

    public static func parse(_ data: Data) throws -> UpdateFeed {
        do {
            return try JSONDecoder().decode(UpdateFeed.self, from: data)
        } catch {
            throw ServiceError("The update feed couldn't be read.")
        }
    }

    /// This Mac's entry: the architecture's own key (`darwin-aarch64` / `darwin-x86_64`, as
    /// Tauri looks up), then `darwin-universal`.
    public func platform(arch: String = UpdateFeed.currentArch) -> Platform? {
        platforms["darwin-\(arch)"] ?? platforms["darwin-universal"]
    }

    public static var currentArch: String {
        #if arch(arm64)
            "aarch64"
        #else
            "x86_64"
        #endif
    }

    /// Whether the feed offers something newer than `current` (the updater never downgrades).
    public func isNewer(than current: String) -> Bool {
        guard let remote = SemanticVersion(version), let mine = SemanticVersion(current) else { return false }
        return remote > mine
    }
}

/// Semantic versions as the updater compares them (`0.3.0-beta.1` < `0.3.0`).
public struct SemanticVersion: Comparable, Sendable, Equatable {
    public var major: Int
    public var minor: Int
    public var patch: Int
    /// Dot-separated pre-release identifiers; empty for a release.
    public var prerelease: [String]

    public init?(_ string: String) {
        var s = string.trimmingCharacters(in: .whitespaces)
        if s.hasPrefix("v") { s.removeFirst() }
        if let plus = s.firstIndex(of: "+") { s = String(s[..<plus]) } // build metadata never counts
        let parts = s.split(separator: "-", maxSplits: 1)
        guard let core = parts.first else { return nil }
        let numbers = core.split(separator: ".").map { Int($0) }
        guard numbers.count == 3, let major = numbers[0], let minor = numbers[1], let patch = numbers[2] else { return nil }
        self.major = major
        self.minor = minor
        self.patch = patch
        prerelease = parts.count > 1 ? parts[1].split(separator: ".").map(String.init) : []
    }

    public static func < (a: SemanticVersion, b: SemanticVersion) -> Bool {
        if (a.major, a.minor, a.patch) != (b.major, b.minor, b.patch) {
            return (a.major, a.minor, a.patch) < (b.major, b.minor, b.patch)
        }
        // A pre-release sorts before its release.
        switch (a.prerelease.isEmpty, b.prerelease.isEmpty) {
        case (true, true), (true, false): return false
        case (false, true): return true
        case (false, false): break
        }
        for (x, y) in zip(a.prerelease, b.prerelease) where x != y {
            switch (Int(x), Int(y)) {
            case let (i?, j?): return i < j
            case (_?, nil): return true // numeric identifiers sort first
            case (nil, _?): return false
            case (nil, nil): return x < y
            }
        }
        return a.prerelease.count < b.prerelease.count
    }
}
