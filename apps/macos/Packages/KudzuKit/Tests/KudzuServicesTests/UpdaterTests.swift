import CryptoKit
import Foundation
import KudzuCore
@testable import KudzuServices
import Testing

// MARK: - BLAKE2b

@Suite struct Blake2bTests {
    /// RFC 7693 Appendix A: BLAKE2b-512("abc").
    @Test func rfc7693AppendixA() {
        #expect(Blake2b.hash512(Data("abc".utf8)).hex
            == "ba80a53f981c4d0d6a2797b69f12f6e94c212f14685ac4b74b12bb6fdbffa2d17d87c5392aab792dc252d5de4533cc9518d38aa8dbf1925ab92386edd4009923")
    }

    @Test func emptyInput() {
        #expect(Blake2b.hash512(Data()).hex
            == "786a02f742015903c6c6fd852552d272912f4740e15847618a86e217f71f5419d25e1031afee585313896444934eb04b903a685b1448b755d56f701afe9be2ce")
    }

    /// Block boundaries (127/128/129, 255/256 bytes) against Python's `hashlib.blake2b`.
    @Test(arguments: [
        (127, "71546bbf9110ad184cc60f2eb120fcfd9b4dbbca7a7f1270045b8a23a6a4f4330f65c1f030dd2f5fabc6c57617242c37cf427bd90407fac5b9deffd3ae888c39"),
        (128, "2d9e329f42afa3601d646692b81c13e87fcaff5bf15972e9813d7373cb6d181f9599f4d513d4af4fd6ebd37497aceb29aba5ee23ed764d8510b552bd088814fb"),
        (129, "47889df9eb4d717afc5019df5c6a83df00a0b8677395e078cd5778ace0f338a618e68b7d9afb065d9e6a01ccd31d109447e7fae771c3ee3e105709194122ba2b"),
        (255, "1a5199ac66a00e8a87ad1c7fbad30b33137dd8312bf6d98602dacf8f40ea2cb623a7fbc63e5a6bfa434d337ae7da5ca1a52502a215a3fe0297a151be85d88789"),
        (256, "91019c558584980249ca43eceed27e19f1c3c24161b93eed1eee2a6a774f60bf8a81b43750870bee1698feac9c5336ae4d5c842e7ead159bf3916387e8ded9ae"),
        (1000, "4bdd2c9cf31d797a81d245c989ffb7515143ca345c66f73087dd5c58bf642bf083ba16894eab79e3b08d5126404d833e7510271b50be36a7b7cbbb46f5c89fac"),
    ])
    func blockBoundaries(length: Int, expected: String) {
        let data = Data((0 ..< length).map { UInt8(($0 * 7 + 3) % 256) })
        #expect(Blake2b.hash512(data).hex == expected)
        // Fed in uneven pieces, the digest is the same.
        var b = Blake2b()
        var offset = 0
        for size in [1, 63, 64, 129, 300, 1000] where offset < length {
            let end = min(length, offset + size)
            b.update(data[offset ..< end])
            offset = end
        }
        if offset < length { b.update(data[offset...]) }
        #expect(b.finalize().hex == expected)
    }
}

// MARK: - minisign

/// Builds minisign files the way `minisign`/`tauri signer` write them, with a CryptoKit key.
struct MinisignFixture {
    let key = Curve25519.Signing.PrivateKey()
    let keyID: [UInt8] = [0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88]

    var publicFile: String {
        let raw = Data("Ed".utf8) + Data(keyID) + key.publicKey.rawRepresentation
        return "untrusted comment: minisign public key: \(keyID.reversed().map { String(format: "%02X", $0) }.joined())\n\(raw.base64EncodedString())\n"
    }

    func signatureFile(for data: Data, prehashed: Bool = true, trustedComment: String = "timestamp:1762600000\tfile:Kudzu.app.tar.gz") throws -> String {
        let message = prehashed ? Data(Blake2b.hash512(data)) : data
        let signature = try key.signature(for: message)
        let raw = Data((prehashed ? "ED" : "Ed").utf8) + Data(keyID) + signature
        let global = try key.signature(for: signature + Data(trustedComment.utf8))
        return "untrusted comment: signature from tauri secret key\n\(raw.base64EncodedString())\ntrusted comment: \(trustedComment)\n\(global.base64EncodedString())\n"
    }

    var tauriPublicKey: String { Data(publicFile.utf8).base64EncodedString() }
    func tauriSignature(for data: Data, prehashed: Bool = true, trustedComment: String = "timestamp:1762600000\tfile:Kudzu.app.tar.gz") throws -> String {
        Data(try signatureFile(for: data, prehashed: prehashed, trustedComment: trustedComment).utf8).base64EncodedString()
    }
}

@Suite struct MinisignTests {
    let archive = Data((0 ..< 5000).map { UInt8($0 % 251) })

    @Test func parsesTheEmbeddedPublicKey() throws {
        let key = try Minisign.PublicKey(tauriBase64: Updater.publicKeyBase64)
        #expect(key.keyIDHex == "6E52269F4241F007")
        #expect(key.key.count == 32)
    }

    @Test func verifiesPrehashedAndLegacySignatures() throws {
        let f = MinisignFixture()
        let key = try Minisign.PublicKey(fileContents: f.publicFile)
        for prehashed in [true, false] {
            let sig = try Minisign.Signature(fileContents: try f.signatureFile(for: archive, prehashed: prehashed))
            #expect(sig.prehashed == prehashed)
            try Minisign.verify(archive, signature: sig, publicKey: key)
        }
        // The Tauri encoding (each file base64-encoded once more).
        try Updater.verify(archive, signature: try f.tauriSignature(for: archive), publicKey: f.tauriPublicKey)
    }

    @Test func aVersionedCheckRequiresTheArchiveNameInTheTrustedComment() throws {
        let f = MinisignFixture()
        let plain = try f.tauriSignature(for: archive)
        #expect(throws: Minisign.Failure.invalidTrustedComment) {
            try Updater.verify(archive, signature: plain, publicKey: f.tauriPublicKey, fileVersion: "0.7.0")
        }
        let named = try f.tauriSignature(for: archive, trustedComment: "timestamp:1762600000\tfile:Kudzu_0.7.0_universal.app.tar.gz")
        try Updater.verify(archive, signature: named, publicKey: f.tauriPublicKey, fileVersion: "0.7.0")
        try Updater.verify(archive, signature: plain, publicKey: f.tauriPublicKey)
    }

    @Test func rejectsTamperedDataWrongKeysAndForgedComments() throws {
        let f = MinisignFixture()
        let key = try Minisign.PublicKey(fileContents: f.publicFile)
        let sigText = try f.signatureFile(for: archive)
        let sig = try Minisign.Signature(fileContents: sigText)

        var tampered = archive
        tampered[10] ^= 1
        #expect(throws: Minisign.Failure.invalidSignature) { try Minisign.verify(tampered, signature: sig, publicKey: key) }

        let other = MinisignFixture()
        let otherKey = try Minisign.PublicKey(fileContents: other.publicFile)
        #expect(throws: Minisign.Failure.invalidSignature) { try Minisign.verify(archive, signature: sig, publicKey: otherKey) }

        var wrongID = key
        wrongID.keyID[0] ^= 0xFF
        #expect(throws: Minisign.Failure.keyMismatch) { try Minisign.verify(archive, signature: sig, publicKey: wrongID) }

        var forged = sig
        forged.trustedComment = "timestamp:0\tfile:evil.tar.gz"
        #expect(throws: Minisign.Failure.invalidTrustedComment) { try Minisign.verify(archive, signature: forged, publicKey: key) }

        // The real release key never verifies a signature from another key.
        #expect(throws: Minisign.Failure.keyMismatch) {
            try Updater.verify(archive, signature: try f.tauriSignature(for: archive), publicKey: Updater.publicKeyBase64)
        }
    }

    @Test func rejectsMalformedFiles() {
        #expect(throws: Minisign.Failure.malformedPublicKey) { try Minisign.PublicKey(fileContents: "untrusted comment: x\nAAAA\n") }
        #expect(throws: Minisign.Failure.malformedSignature) { try Minisign.Signature(fileContents: "untrusted comment: x\n") }
        let bogus = Data("Xy".utf8) + Data(count: 72)
        let text = "untrusted comment: x\n\(bogus.base64EncodedString())\ntrusted comment: t\n\(Data(count: 64).base64EncodedString())\n"
        #expect(throws: Minisign.Failure.unsupportedAlgorithm) { try Minisign.Signature(fileContents: text) }
    }

    /// End-to-end against a real release, when the archive and its feed signature are on disk:
    /// `KUDZU_UPDATE_ARCHIVE=…/Kudzu_x_universal.app.tar.gz KUDZU_UPDATE_SIG=…/latest.json swift test`.
    @Test(.enabled(if: ProcessInfo.processInfo.environment["KUDZU_UPDATE_ARCHIVE"] != nil))
    func verifiesARealReleaseArchive() throws {
        let env = ProcessInfo.processInfo.environment
        let archive = try Data(contentsOf: URL(fileURLWithPath: try #require(env["KUDZU_UPDATE_ARCHIVE"])))
        let feed = try UpdateFeed.parse(try Data(contentsOf: URL(fileURLWithPath: try #require(env["KUDZU_UPDATE_SIG"]))))
        let platform = try #require(feed.platform(arch: "aarch64"))
        try Updater.verify(archive, signature: platform.signature, publicKey: Updater.publicKeyBase64)

        var tampered = archive
        tampered[tampered.count / 2] ^= 1
        #expect(throws: Minisign.Failure.invalidSignature) {
            try Updater.verify(tampered, signature: platform.signature, publicKey: Updater.publicKeyBase64)
        }

        // And it swaps in over an installed bundle.
        let dir = tempDir("install")
        defer { try? FileManager.default.removeItem(at: dir) }
        let installed = dir.appendingPathComponent("Kudzu.app")
        // Use a signed installation: the installer now verifies its designated requirement.
        try Installer.run("/usr/bin/tar", ["-xzf", try #require(env["KUDZU_UPDATE_ARCHIVE"]), "-C", dir.path])
        try Installer.install(archive: archive, over: installed)
        #expect(FileManager.default.fileExists(atPath: installed.appendingPathComponent("Contents/Info.plist").path))

        #expect(try FileManager.default.contentsOfDirectory(atPath: dir.path) == ["Kudzu.app"], "no leftovers next to the bundle")

        // An unsigned candidate must fail even when the installed app has a valid designated
        // requirement. Keep this separate from the unsigned/unsigned unit fixture below.
        let info = installed.appendingPathComponent("Contents/Info.plist")
        let originalInfo = try Data(contentsOf: info)
        let unsigned = dir.appendingPathComponent("unsigned/Kudzu.app")
        write("unsigned", to: unsigned.appendingPathComponent("Contents/marker"))
        let badTar = dir.appendingPathComponent("unsigned.tar.gz")
        try Installer.run("/usr/bin/tar", ["-czf", badTar.path, "-C", unsigned.deletingLastPathComponent().path, "Kudzu.app"])
        #expect(throws: ServiceError.self) { try Installer.install(archive: Data(contentsOf: badTar), over: installed) }
        #expect(try Data(contentsOf: info) == originalInfo)
        #expect(!FileManager.default.fileExists(atPath: installed.appendingPathComponent("Contents/marker").path))
    }
}

// MARK: - Feed and flow

@Suite struct UpdateFeedTests {
    static let feedJSON = """
    {
      "version": "0.7.0",
      "notes": "Faster palette.",
      "pub_date": "2026-11-01T10:00:00.000Z",
      "platforms": {
        "darwin-aarch64": { "signature": "c2ln", "url": "https://github.com/Stringsaeed/kudzu/releases/download/v0.7.0/Kudzu_0.7.0_universal.app.tar.gz" },
        "darwin-x86_64": { "signature": "c2ln", "url": "https://github.com/Stringsaeed/kudzu/releases/download/v0.7.0/Kudzu_0.7.0_universal.app.tar.gz" }
      }
    }
    """

    @Test func parsesTheTauriLatestJSON() throws {
        let feed = try UpdateFeed.parse(Data(Self.feedJSON.utf8))
        #expect(feed.version == "0.7.0")
        #expect(feed.notes == "Faster palette.")
        #expect(feed.pubDate == "2026-11-01T10:00:00.000Z")
        #expect(feed.platform(arch: "aarch64")?.url.lastPathComponent == "Kudzu_0.7.0_universal.app.tar.gz")
        #expect(feed.platform(arch: "x86_64") != nil)
        #expect(feed.isNewer(than: "0.6.0"))
        #expect(!feed.isNewer(than: "0.7.0"))
        #expect(!feed.isNewer(than: "0.8.0-beta.1"))
        #expect(throws: ServiceError.self) { try UpdateFeed.parse(Data("<html>504</html>".utf8)) }
    }

    @Test func fallsBackToTheUniversalKey() throws {
        let json = #"{"version":"1.0.0","platforms":{"darwin-universal":{"signature":"s","url":"https://e.x/a.tar.gz"}}}"#
        let feed = try UpdateFeed.parse(Data(json.utf8))
        #expect(feed.platform(arch: "aarch64")?.url.absoluteString == "https://e.x/a.tar.gz")
        #expect(feed.notes == nil)
    }

    @Test func channelsPickTheirFeeds() {
        #expect(UpdateFeed.url(for: .stable).absoluteString == "https://github.com/Stringsaeed/kudzu/releases/latest/download/latest.json")
        #expect(UpdateFeed.url(for: .beta).absoluteString == "https://github.com/Stringsaeed/kudzu/releases/download/update-feed/beta.json")
    }

    @Test func comparesSemanticVersions() throws {
        let order = ["0.2.0", "0.3.0-beta.1", "0.3.0-beta.2", "0.3.0-beta.10", "0.3.0", "0.3.1", "1.0.0-alpha", "1.0.0"]
        let parsed = try order.map { try #require(SemanticVersion($0)) }
        for (a, b) in zip(parsed, parsed.dropFirst()) { #expect(a < b) }
        #expect(SemanticVersion("v1.2.3") == SemanticVersion("1.2.3+build.5"))
        #expect(SemanticVersion("1.2") == nil)
    }

    @Test func developmentVariantCannotUseProductionUpdates() async {
        if case .failure = Updater.updateAllowed(AppIdentity.productionID) { Issue.record("production must update") }
        guard case let .failure(e) = Updater.updateAllowed(AppIdentity.devID) else { Issue.record("dev must refuse"); return }
        #expect(e.message == "Updates are unavailable in Kudzu Dev.")

        let published = Published()
        let dev = Updater(identity: AppIdentity(bundleID: AppIdentity.devID, version: "0.6.0", productName: "Kudzu Dev"), bundle: nil,
                          channel: { .stable }, autoCheck: { true }, publish: published.add, fetch: { _, _ in Issue.record("no network in dev"); return Data() })
        let state = await dev.check(quiet: false)
        #expect(state == .error(message: "Updates are unavailable in Kudzu Dev.", needsMove: false))
        #expect(throws: ServiceError.self) { try dev.installPending() }
    }

    @Test func downloadsVerifiesAndHoldsTheUpdate() async throws {
        let f = MinisignFixture()
        let archive = Data("pretend tarball".utf8)
        let signature = try f.tauriSignature(for: archive, trustedComment: "timestamp:1762600000\tfile:Kudzu_0.7.0_universal.app.tar.gz")
        let feed = Self.feedJSON.replacingOccurrences(of: "c2ln", with: signature)
        let published = Published()
        let channel = ChannelBox()
        let updater = Updater(
            identity: AppIdentity(bundleID: AppIdentity.productionID, version: "0.6.0", productName: "Kudzu"),
            publicKey: f.tauriPublicKey, bundle: nil,
            channel: { channel.value }, autoCheck: { true }, publish: published.add,
            fetch: { url, progress in
                if url.pathExtension == "json" {
                    channel.requested.append(url)
                    return Data(feed.utf8)
                }
                progress(UInt64(archive.count), UInt64(archive.count))
                return archive
            }
        )
        let state = await updater.check(quiet: false)
        #expect(state == .ready(version: "0.7.0", notes: "Faster palette.", date: "2026-11-01T10:00:00.000Z"))
        #expect(published.states.first == .checking)
        #expect(published.states.contains(.downloading(version: "0.7.0", downloaded: 15, total: 15)))
        #expect(updater.state == state)
        // Holding an update: later checks don't download again.
        channel.value = .beta
        #expect(await updater.check(quiet: false) == state)
        #expect(channel.requested == [UpdateFeed.stableURL])
    }

    @Test func aBadSignatureIsNeverHeld() async throws {
        let f = MinisignFixture()
        let signature = try f.tauriSignature(for: Data("the real one".utf8), trustedComment: "timestamp:1762600000\tfile:Kudzu_0.7.0_universal.app.tar.gz")
        let feed = Self.feedJSON.replacingOccurrences(of: "c2ln", with: signature)
        let updater = Updater(
            identity: AppIdentity(bundleID: AppIdentity.productionID, version: "0.6.0", productName: "Kudzu"),
            publicKey: f.tauriPublicKey, bundle: nil, channel: { .stable }, autoCheck: { true }, publish: { _ in },
            fetch: { url, _ in url.pathExtension == "json" ? Data(feed.utf8) : Data("tampered".utf8) }
        )
        #expect(await updater.check(quiet: false) == .error(message: Minisign.Failure.invalidSignature.description, needsMove: false))
        #expect(await updater.check(quiet: true) == .idle)
        #expect(try !updater.installPending())
    }

    @Test func upToDateWhenTheFeedIsNotNewer() async {
        let updater = Updater(
            identity: AppIdentity(bundleID: AppIdentity.productionID, version: "0.7.0", productName: "Kudzu"),
            bundle: nil, channel: { .stable }, autoCheck: { true }, publish: { _ in },
            fetch: { _, _ in Data(Self.feedJSON.utf8) }
        )
        guard case .upToDate = await updater.check(quiet: false) else { Issue.record("expected up to date"); return }
    }

    @Test func locationProbeDoesNotTruncateAnExistingFile() throws {
        let dir = tempDir("probe")
        defer { try? FileManager.default.removeItem(at: dir) }
        let existing = dir.appendingPathComponent(".kudzu-write-test")
        try Data("keep me".utf8).write(to: existing)
        #expect(Installer.locationProblem(bundle: dir.appendingPathComponent("Kudzu.app")) == nil)
        #expect(read(existing) == "keep me")
    }

    @Test func unsignedUpdateCannotReplaceTheInstalledBundle() throws {
        let dir = tempDir("unsigned-update")
        defer { try? FileManager.default.removeItem(at: dir) }
        let candidate = dir.appendingPathComponent("source/Kudzu.app")
        write("unsigned", to: candidate.appendingPathComponent("Contents/marker"))
        let tar = dir.appendingPathComponent("update.tar.gz")
        try Installer.run("/usr/bin/tar", ["-czf", tar.path, "-C", candidate.deletingLastPathComponent().path, "Kudzu.app"])
        let installed = dir.appendingPathComponent("Kudzu.app")
        write("original", to: installed.appendingPathComponent("Contents/marker"))
        #expect(throws: ServiceError.self) { try Installer.install(archive: Data(contentsOf: tar), over: installed) }
        #expect(read(installed.appendingPathComponent("Contents/marker")) == "original")
    }

    @Test func runningFromADiskImageNeedsAMove() {
        #expect(Installer.locationProblem(bundle: URL(fileURLWithPath: "/Volumes/Kudzu/Kudzu.app")) == "Move Kudzu to your Applications folder so it can update itself.")
        let dir = tempDir("loc")
        defer { try? FileManager.default.removeItem(at: dir) }
        #expect(Installer.locationProblem(bundle: dir.appendingPathComponent("Kudzu.app")) == nil)
    }
}

/// Records published states (thread-safe).
final class Published: @unchecked Sendable {
    private let lock = NSLock()
    private var list: [UpdateState] = []
    var states: [UpdateState] { lock.withLock { list } }
    var add: @Sendable (UpdateState) -> Void { { [self] s in lock.withLock { list.append(s) } } }
}

final class ChannelBox: @unchecked Sendable {
    private let lock = NSLock()
    private var _value: UpdateChannel = .stable
    private var _requested: [URL] = []
    var value: UpdateChannel {
        get { lock.withLock { _value } }
        set { lock.withLock { _value = newValue } }
    }
    var requested: [URL] {
        get { lock.withLock { _requested } }
        set { lock.withLock { _requested = newValue } }
    }
}
