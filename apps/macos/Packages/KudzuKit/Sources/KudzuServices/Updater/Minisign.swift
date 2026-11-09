import CryptoKit
import Foundation

/// minisign signature verification (what tauri-plugin-updater does with `minisign-verify`).
///
/// File formats (https://jedisct1.github.io/minisign/):
///
/// ```text
/// public key:  untrusted comment: <text>
///              base64( "Ed" | key id (8) | Ed25519 public key (32) )
///
/// signature:   untrusted comment: <text>
///              base64( "Ed" or "ED" | key id (8) | Ed25519 signature (64) )
///              trusted comment: <text>
///              base64( Ed25519 signature (64) of: signature (64) | trusted comment bytes )
/// ```
///
/// `ED` (prehashed, minisign ≥ 0.8 and Tauri's signer) signs the BLAKE2b-512 digest of the file;
/// legacy `Ed` signs the file itself. The global signature binds the trusted comment, so it is
/// checked too. Tauri ships both files base64-encoded once more (`tauri.conf.json`'s `pubkey` and
/// the feed's `signature`); ``PublicKey/init(tauriBase64:)`` and ``Signature/init(tauriBase64:)``
/// undo that layer.
public enum Minisign {
    public enum Failure: Error, Equatable, Sendable, CustomStringConvertible {
        case malformedPublicKey
        case malformedSignature
        case unsupportedAlgorithm
        case keyMismatch
        case invalidSignature
        case invalidTrustedComment

        public var description: String {
            switch self {
            case .malformedPublicKey: "The update public key is malformed."
            case .malformedSignature: "The update signature is malformed."
            case .unsupportedAlgorithm: "The update signature uses an unsupported algorithm."
            case .keyMismatch: "The update was signed with a different key."
            case .invalidSignature: "The update's signature doesn't match. It was not installed."
            case .invalidTrustedComment: "The update signature's trusted comment doesn't match."
            }
        }
    }

    public struct PublicKey: Sendable, Equatable {
        public var keyID: [UInt8]
        public var key: [UInt8]

        public init(keyID: [UInt8], key: [UInt8]) {
            self.keyID = keyID
            self.key = key
        }

        /// The text of a `.pub` file (comment line optional).
        public init(fileContents: String) throws(Failure) {
            guard let line = Minisign.payloadLines(fileContents).first,
                  let raw = Data(base64Encoded: line), raw.count == 42,
                  raw.prefix(2) == Data("Ed".utf8) else { throw .malformedPublicKey }
            let bytes = [UInt8](raw)
            keyID = Array(bytes[2 ..< 10])
            key = Array(bytes[10 ..< 42])
        }

        /// `plugins.updater.pubkey` in tauri.conf.json: the `.pub` file, base64-encoded.
        public init(tauriBase64: String) throws(Failure) {
            guard let data = Data(base64Encoded: tauriBase64.trimmingCharacters(in: .whitespacesAndNewlines)),
                  let text = String(data: data, encoding: .utf8) else { throw .malformedPublicKey }
            try self.init(fileContents: text)
        }

        /// The key id as minisign prints it (`6E52269F4241F007`): little-endian hex.
        public var keyIDHex: String { keyID.reversed().map { String(format: "%02X", $0) }.joined() }
    }

    public struct Signature: Sendable, Equatable {
        public var prehashed: Bool
        public var keyID: [UInt8]
        public var signature: [UInt8]
        public var trustedComment: String
        public var globalSignature: [UInt8]

        /// The text of a `.minisig` / `.sig` file.
        public init(fileContents: String) throws(Failure) {
            let lines = fileContents.split(omittingEmptySubsequences: true, whereSeparator: \.isNewline).map(String.init)
            guard lines.count >= 4, lines[0].hasPrefix("untrusted comment:"),
                  let raw = Data(base64Encoded: lines[1].trimmingCharacters(in: .whitespaces)), raw.count == 74,
                  lines[2].hasPrefix("trusted comment: "),
                  let global = Data(base64Encoded: lines[3].trimmingCharacters(in: .whitespaces)), global.count == 64
            else { throw .malformedSignature }
            let bytes = [UInt8](raw)
            switch String(decoding: bytes[0 ..< 2], as: UTF8.self) {
            case "ED": prehashed = true
            case "Ed": prehashed = false
            default: throw .unsupportedAlgorithm
            }
            keyID = Array(bytes[2 ..< 10])
            signature = Array(bytes[10 ..< 74])
            trustedComment = String(lines[2].dropFirst("trusted comment: ".count))
            globalSignature = [UInt8](global)
        }

        /// The feed's `signature` field: the `.sig` file, base64-encoded.
        public init(tauriBase64: String) throws(Failure) {
            guard let data = Data(base64Encoded: tauriBase64.trimmingCharacters(in: .whitespacesAndNewlines)),
                  let text = String(data: data, encoding: .utf8) else { throw .malformedSignature }
            try self.init(fileContents: text)
        }
    }

    /// Verifies `data` against `signature` made by `publicKey`. Throws unless both the file
    /// signature and the trusted-comment signature are valid.
    public static func verify(_ data: Data, signature: Signature, publicKey: PublicKey) throws(Failure) {
        guard signature.keyID == publicKey.keyID else { throw .keyMismatch }
        guard let key = try? Curve25519.Signing.PublicKey(rawRepresentation: publicKey.key) else { throw .malformedPublicKey }
        let message = signature.prehashed ? Data(Blake2b.hash512(data)) : data
        guard key.isValidSignature(Data(signature.signature), for: message) else { throw .invalidSignature }
        let global = Data(signature.signature) + Data(signature.trustedComment.utf8)
        guard key.isValidSignature(Data(signature.globalSignature), for: global) else { throw .invalidTrustedComment }
    }

    /// Non-comment lines of a minisign file.
    static func payloadLines(_ text: String) -> [String] {
        text.split(whereSeparator: \.isNewline)
            .map { $0.trimmingCharacters(in: .whitespaces) }
            .filter { !$0.isEmpty && !$0.hasPrefix("untrusted comment:") }
    }
}
