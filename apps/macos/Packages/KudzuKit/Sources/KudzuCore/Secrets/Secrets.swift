// Ported from kudzu-core/src/secrets.rs
// Finds passwords, API keys, tokens and private keys in text, so Kudzu can withhold them from
// AI agents.
//
// Detection is heuristic. It combines four kinds of evidence:
//
// - **Known formats**: PEM private keys, JWTs, and tokens with a vendor prefix (`ghp_`,
//   `sk-ant-`, `AKIA…`, `xoxb-`, …).
// - **Labels**: a value after `password=`, `"api_key": "…"`, `Secret: …`, or `Bearer …`.
// - **Credentials in URLs**: the password in `scheme://user:password@host`, and webhook URLs
//   whose path is the secret.
// - **Shape**: a lone word that looks generated (mixed character classes, high entropy), or a
//   long random-looking run inside other text.
//
// Short or dictionary-like passwords without a label (`hunter2`) can't be told apart from
// ordinary words, so a miss is always possible. When unsure, the rules lean towards
// withholding.
//
// The scanners work on the UTF-8 bytes and Unicode scalars exactly as the Rust ones do (byte
// searches, `char` classes), never on `Character`s or regexes: finding ranges are byte offsets.

import Foundation

/// Finds passwords, API keys, tokens and private keys before text reaches AI agents.
/// Detection combines known formats, labels, URL credentials and generated-looking shape.
public enum Secrets {
    /// Text scanned by `detect`; a secret is almost always near the start of a copy.
    public static let DETECT_LIMIT = 256 * 1024

    /// One secret: its UTF-8 byte range in the scanned text (the Rust contract).
    public struct Finding: Equatable, Sendable {
        public var range: Range<Int>
        public var kind: SecretKind
        public init(range: Range<Int>, kind: SecretKind) { self.range = range; self.kind = kind }
    }

    /// Text with its secrets replaced by `[redacted: …]` markers.
    public struct Redacted: Equatable, Sendable {
        public var text: String
        /// The kinds found, in order of first appearance, without repeats.
        public var kinds: [SecretKind]
        /// The text is a secret as a whole (nothing but secrets and whitespace).
        public var whole: Bool
        public var isClean: Bool { kinds.isEmpty }
        public init(text: String, kinds: [SecretKind], whole: Bool) {
            self.text = text; self.kinds = kinds; self.whole = whole
        }
    }

    /// Every secret in `text`, sorted and non-overlapping.
    public static func scan(_ text: String) -> [Finding] {
        let b = Array(text.utf8)
        var found: [Finding] = []
        privateKeys(b, &found)
        jwts(b, &found)
        sendgrid(b, &found)
        words(b, &found)
        urlCredentials(b, &found)
        authorization(b, &found)
        labelled(b, &found)
        loneWord(b, &found)
        return merge(found)
    }

    /// The first kind of secret in `text` (its first [`DETECT_LIMIT`] bytes), if any.
    public static func detect(_ text: String) -> SecretKind? {
        let bytes = Array(text.utf8)
        var end = min(bytes.count, DETECT_LIMIT)
        while end > 0 && end < bytes.count && bytes[end] & 0xC0 == 0x80 { end -= 1 }
        return scan(String(decoding: bytes.prefix(end), as: UTF8.self)).first?.kind
    }

    /// Whether `text` contains anything that looks like a secret.
    public static func containsSecret(_ text: String) -> Bool { !scan(text).isEmpty }

    /// The marker that replaces a secret of `kind`.
    public static func marker(_ kind: SecretKind) -> String { "[redacted: \(kind.label)]" }

    /// Replaces every secret in `text` with a marker.
    public static func redact(_ text: String) -> Redacted {
        let bytes = Array(text.utf8), findings = scan(text)
        func piece(_ r: Range<Int>) -> String { String(decoding: bytes[r], as: UTF8.self) }
        var out = "", kinds: [SecretKind] = [], at = 0
        for f in findings {
            out += piece(at..<f.range.lowerBound) + marker(f.kind)
            at = f.range.upperBound
            if !kinds.contains(f.kind) { kinds.append(f.kind) }
        }
        out += piece(at..<bytes.count)
        var covered = 0, whole = !findings.isEmpty
        for f in findings {
            if !RustText.trim(piece(covered..<f.range.lowerBound)).isEmpty { whole = false }
            covered = f.range.upperBound
        }
        whole = whole && RustText.trim(piece(covered..<bytes.count)).isEmpty
        return Redacted(text: out, kinds: kinds, whole: whole)
    }

    static func merge(_ input: [Finding]) -> [Finding] {
        // A stable sort: equal ranges keep rule order, as Rust's `sort_by`.
        let found = input.filter { !$0.range.isEmpty }.enumerated().sorted {
            if $0.element.range.lowerBound != $1.element.range.lowerBound { return $0.element.range.lowerBound < $1.element.range.lowerBound }
            if $0.element.range.upperBound != $1.element.range.upperBound { return $0.element.range.upperBound > $1.element.range.upperBound }
            return $0.offset < $1.offset
        }.map(\.element)
        var out: [Finding] = []
        for f in found {
            if let last = out.last, f.range.lowerBound < last.range.upperBound {
                out[out.count - 1].range = last.range.lowerBound..<max(last.range.upperBound, f.range.upperBound)
            } else {
                out.append(f)
            }
        }
        return out
    }

    // MARK: Known formats

    static func privateKeys(_ b: [UInt8], _ out: inout [Finding]) {
        let begin = Array("-----BEGIN ".utf8), dashes = Array("-----".utf8)
        var from = 0
        while let i = RustText.find(begin, in: b, from: from) {
            guard let j = RustText.find(dashes, in: b, from: i + begin.count) else { break }
            let headerEnd = j + 5
            let label = Array(b[(i + begin.count)..<j])
            if label.reversed().starts(with: "PRIVATE KEY".utf8.reversed()) || label.elementsEqual("PGP PRIVATE KEY BLOCK".utf8) {
                let endMarker = Array("-----END ".utf8) + label + dashes
                let end = RustText.find(endMarker, in: b, from: headerEnd).map { $0 + endMarker.count } ?? b.count
                out.append(Finding(range: i..<end, kind: .privateKey))
                from = end
            } else {
                from = headerEnd
            }
        }
    }

    static func isWordByte(_ b: UInt8) -> Bool { RustText.isASCIIAlphanumeric(b) || b == UInt8(ascii: "_") || b == UInt8(ascii: "-") }

    static func jwts(_ b: [UInt8], _ out: inout [Finding]) {
        let start = Array("eyJ".utf8)
        var from = 0
        while let i = RustText.find(start, in: b, from: from) {
            from = i + 3
            if i > 0 && isWordByte(b[i - 1]) { continue }
            var end = i
            while end < b.count && (isWordByte(b[end]) || b[end] == UInt8(ascii: ".") || b[end] == UInt8(ascii: "=")) { end += 1 }
            let parts = b[i..<end].split(separator: UInt8(ascii: "."), omittingEmptySubsequences: false)
            if parts.count >= 3 && parts[0].count >= 10 && parts[1].starts(with: start) && parts[1].count >= 4 {
                out.append(Finding(range: i..<end, kind: .token))
                from = end
            }
        }
    }

    static func sendgrid(_ b: [UInt8], _ out: inout [Finding]) {
        func wordRun(_ from: Int) -> Int { var n = 0; while from + n < b.count && isWordByte(b[from + n]) { n += 1 }; return n }
        var from = 0
        while let i = RustText.find(Array("SG.".utf8), in: b, from: from) {
            from = i + 3
            if i > 0 && isWordByte(b[i - 1]) { continue }
            let a = wordRun(i + 3), dot = i + 3 + a
            guard a == 22, dot < b.count, b[dot] == UInt8(ascii: ".") else { continue }
            let rest = wordRun(dot + 1)
            if rest == 43 { out.append(Finding(range: i..<(dot + 1 + rest), kind: .apiKey)) }
        }
    }

    /// Vendor prefixes and the shortest token (prefix included) that counts.
    static let PREFIXES: [(String, Int)] = [
        ("sk-ant-", 30), ("sk-proj-", 30), ("sk-svcacct-", 30), ("sk-or-v1-", 30),
        ("sk_live_", 20), ("sk_test_", 20), ("rk_live_", 20), ("rk_test_", 20), ("whsec_", 20),
        ("ghp_", 30), ("gho_", 30), ("ghu_", 30), ("ghs_", 30), ("ghr_", 30), ("github_pat_", 40),
        ("glpat-", 24), ("glptt-", 24), ("xoxb-", 20), ("xoxp-", 20), ("xoxa-", 20), ("xoxr-", 20), ("xoxs-", 20), ("xapp-", 20),
        ("npm_", 30), ("pypi-", 50), ("shpat_", 30), ("shpss_", 30), ("shpca_", 30), ("dop_v1_", 40), ("doo_v1_", 40),
        ("hf_", 30), ("lin_api_", 30), ("figd_", 30), ("dckr_pat_", 30), ("gsk_", 30), ("xai-", 30), ("sbp_", 30),
    ]

    /// Characters that make up base64 and similar encodings. Padding (`=`) is left out so
    /// `KEY=value` stays two runs; the padding of a real key doesn't change its verdict.
    static func isB64Byte(_ b: UInt8) -> Bool { isWordByte(b) || b == UInt8(ascii: "+") || b == UInt8(ascii: "/") }

    static func runs(_ b: [UInt8], _ keep: (UInt8) -> Bool) -> [Range<Int>] {
        var out: [Range<Int>] = [], i = 0
        while i < b.count {
            if !keep(b[i]) { i += 1; continue }
            let start = i
            while i < b.count && keep(b[i]) { i += 1 }
            out.append(start..<i)
        }
        return out
    }

    static func words(_ b: [UInt8], _ out: inout [Finding]) {
        for r in runs(b, isWordByte) where vendorKey(b[r]) { out.append(Finding(range: r, kind: .apiKey)) }
        for r in runs(b, isB64Byte) {
            // Trim padding and path separators that belong to the surroundings.
            let raw = b[r]
            let lead = raw.prefix { [UInt8(ascii: "/"), UInt8(ascii: "+"), UInt8(ascii: "-")].contains($0) }.count
            let trail = raw.reversed().prefix { [UInt8(ascii: "/"), UInt8(ascii: "-")].contains($0) }.count
            if lead + trail >= raw.count { continue }
            let range = (r.lowerBound + lead)..<(r.upperBound - trail)
            if looksRandom(String(decoding: b[range], as: UTF8.self)) { out.append(Finding(range: range, kind: .apiKey)) }
        }
    }

    static func vendorKey(_ word: ArraySlice<UInt8>) -> Bool {
        func starts(_ p: String) -> Bool { word.starts(with: p.utf8) }
        // AWS access key ids and Google API keys have fixed shapes.
        let aws = ["AKIA", "ASIA", "ABIA", "ACCA"].contains(where: starts) && word.count == 20
            && word.allSatisfy { (65...90).contains($0) || (48...57).contains($0) }
        let google = starts("AIza") && word.count == 39
        if aws || google { return true }
        return PREFIXES.contains { prefix, min in
            guard word.count >= min && starts(prefix) else { return false }
            let rest = word.dropFirst(prefix.utf8.count)
            return rest.contains { (48...57).contains($0) } && rest.contains { RustText.isASCIIAlphanumeric($0) && !(48...57).contains($0) }
        }
    }

    // MARK: Shape

    struct Classes {
        var lower = 0, upper = 0, digit = 0, symbol = 0
        /// Symbols other than the ones identifiers and file names use.
        var strongSymbol = 0

        init(_ s: String) {
            for c in s.unicodeScalars {
                if c.properties.isLowercase { lower += 1 }
                else if c.properties.isUppercase { upper += 1 }
                else if RustText.isNumeric(c) { digit += 1 }
                else {
                    symbol += 1
                    if !"._-/:".unicodeScalars.contains(c) { strongSymbol += 1 }
                }
            }
        }

        var count: Int { [lower, upper, digit, symbol].filter { $0 > 0 }.count }
    }

    /// Shannon entropy relative to the most a string of this length (over this alphabet) can have.
    static func relativeEntropy(_ s: String, _ alphabet: Double) -> Double {
        var counts: [Unicode.Scalar: Int] = [:], n = 0
        for c in s.unicodeScalars { counts[c, default: 0] += 1; n += 1 }
        guard n >= 2 else { return 0 }
        let len = Double(n)
        let h = counts.values.reduce(0.0) { sum, c in let p = Double(c) / len; return sum - p * log2(p) }
        return h / log2(min(len, alphabet))
    }

    static func isHex(_ s: String) -> Bool { s.utf8.allSatisfy { (48...57).contains($0) || (65...70).contains($0) || (97...102).contains($0) } }

    static func isUUID(_ s: String) -> Bool {
        let parts = s.utf8.split(separator: UInt8(ascii: "-"), omittingEmptySubsequences: false)
        return parts.map(\.count) == [8, 4, 4, 4, 12] && parts.allSatisfy { $0.allSatisfy { (48...57).contains($0) || (65...70).contains($0) || (97...102).contains($0) } }
    }

    /// A git commit id: 40 lowercase hex digits. Common on developers' clipboards and not secret.
    static func isCommitID(_ s: String) -> Bool { s.utf8.count == 40 && s.utf8.allSatisfy { (48...57).contains($0) || (97...102).contains($0) } }

    /// A long run of characters that looks generated rather than written.
    static func looksRandom(_ s: String) -> Bool {
        let len = s.utf8.count
        guard (20...512).contains(len), !isUUID(s), !isCommitID(s) else { return false }
        let c = Classes(s)
        guard c.digit >= 2 && c.lower + c.upper >= 2 else { return false }
        if isHex(s) { return len >= 32 && relativeEntropy(s, 16) >= 0.85 }
        return c.lower > 0 && c.upper > 0 && relativeEntropy(s, 64) >= 0.85
    }

    /// A lone word that looks like a password: the whole text is one word with mixed classes.
    static func loneWord(_ b: [UInt8], _ out: inout [Finding]) {
        // A labelled value or a known format already says which part is secret.
        guard out.isEmpty else { return }
        let text = String(decoding: b, as: UTF8.self)
        let trimmed = RustText.trim(text)
        if trimmed.unicodeScalars.contains(where: RustText.isWhitespace) { return }
        guard (8...128).contains(trimmed.unicodeScalars.count), looksLikePassword(trimmed) else { return }
        let start = b.count - RustText.trimStart(text).utf8.count
        out.append(Finding(range: start..<(start + trimmed.utf8.count), kind: .password))
    }

    static func looksLikePassword(_ s: String) -> Bool {
        let lower = RustText.asciiLowercased(s), bytes = Array(s.utf8)
        let isURL = RustText.contains(lower, "://") || RustText.hasPrefix(lower, "www.")
        let isPath = ["/", "~", "./", "../"].contains { RustText.hasPrefix(s, $0) } || bytes.contains(UInt8(ascii: "\\"))
        let isEmail: Bool = {
            guard let at = bytes.firstIndex(of: UInt8(ascii: "@")) else { return false }
            let domain = bytes[(at + 1)...]
            return at > 0 && domain.contains(UInt8(ascii: ".")) && domain.allSatisfy { RustText.isASCIIAlphanumeric($0) || $0 == UInt8(ascii: ".") || $0 == UInt8(ascii: "-") }
        }()
        let isNumber = s.unicodeScalars.allSatisfy { ($0.isASCII && (48...57).contains($0.value)) || ",.+-%$€£".unicodeScalars.contains($0) }
        if isURL || isPath || isEmail || isNumber || isUUID(s) || isCommitID(s) || RustText.hasPrefix(s, "#") { return false }
        let c = Classes(s)
        if c.strongSymbol > 0 && c.count >= 3 && c.lower + c.upper > 0 { return true }
        let len = s.unicodeScalars.count
        if c.lower > 0 && c.upper > 0 && c.digit > 0 && (len >= 12 || (c.digit >= 2 && c.upper >= 2)) { return true }
        // Lowercase letters and digits mixed throughout (not `release2024notes`).
        let alnum = bytes.filter(RustText.isASCIIAlphanumeric)
        let switches = zip(alnum, alnum.dropFirst()).filter { (48...57).contains($0) != (48...57).contains($1) }.count
        if len >= 12 && c.digit >= 3 && c.lower + c.upper > 0 && switches >= 6 && relativeEntropy(s, 36) >= 0.85 { return true }
        return looksRandom(s)
    }

    // MARK: Labels and URLs

    static func urlCredentials(_ b: [UInt8], _ out: inout [Finding]) {
        let lower = b.map { (65...90).contains($0) ? $0 + 32 : $0 }
        func lowered(_ r: Range<Int>) -> [UInt8] { Array(lower[r]) }
        var from = 0
        while let i = RustText.find(Array("://".utf8), in: b, from: from) {
            from = i + 3
            let schemeLen = b[..<i].reversed().prefix { RustText.isASCIIAlphanumeric($0) || $0 == UInt8(ascii: "+") || $0 == UInt8(ascii: ".") || $0 == UInt8(ascii: "-") }.count
            if schemeLen == 0 { continue }
            let rest = i + 3
            let urlEnd = RustText.firstScalar(in: b, from: rest) { RustText.isWhitespace($0) || "\"'<>`)]".unicodeScalars.contains($0) }
            let urlLen = urlEnd - rest
            let authorityLen = b[rest..<urlEnd].firstIndex { [UInt8(ascii: "/"), UInt8(ascii: "?"), UInt8(ascii: "#")].contains($0) }.map { $0 - rest } ?? urlLen
            let authority = b[rest..<(rest + authorityLen)]
            if let at = authority.lastIndex(of: UInt8(ascii: "@")), let colon = b[rest..<at].firstIndex(of: UInt8(ascii: ":")) {
                if at > colon + 1 { out.append(Finding(range: (colon + 1)..<at, kind: .credential)) }
            }
            // Webhook URLs carry their secret in the path.
            let host = lowered(rest..<(rest + authorityLen)), path = lowered((rest + authorityLen)..<urlEnd)
            func hostEnds(_ s: String) -> Bool { host.reversed().starts(with: s.utf8.reversed()) }
            let webhook = (hostEnds("hooks.slack.com") && path.starts(with: "/services/".utf8))
                || ((hostEnds("discord.com") || hostEnds("discordapp.com")) && path.starts(with: "/api/webhooks/".utf8))
            if webhook {
                let skip = path.dropFirst().firstIndex(of: UInt8(ascii: "/")).map { $0 + 1 } ?? 1
                out.append(Finding(range: (rest + authorityLen + skip)..<urlEnd, kind: .credential))
            }
            // Rust skips at least one byte here and panics when that lands inside a multi-byte
            // character (`"a://\u{3000}"`); a byte search simply continues.
            from = i + 3 + min(max(urlLen, 1), b.count - rest)
        }
    }

    static func authorization(_ b: [UInt8], _ out: inout [Finding]) {
        let lower = b.map { (65...90).contains($0) ? $0 + 32 : $0 }
        let header = Array("authorization:".utf8)
        for scheme in ["bearer ", "basic ", "token "] {
            let needle = Array(scheme.utf8)
            var from = 0
            while let i = RustText.find(needle, in: lower, from: from) {
                from = i + needle.count
                if i > 0 && RustText.isASCIIAlphanumeric(lower[i - 1]) { continue }
                // "token " is only a header scheme right after "Authorization:".
                if scheme == "token " {
                    var end = i
                    while let (c, start) = RustText.scalar(before: end, in: lower), RustText.isWhitespace(c) { end = start }
                    if !lower[..<end].reversed().starts(with: header.reversed()) { continue }
                }
                var start = from
                while start < b.count && b[start] == UInt8(ascii: " ") { start += 1 }
                var end = start
                while end < b.count && (isB64Byte(b[end]) || b[end] == UInt8(ascii: ".") || b[end] == UInt8(ascii: "~") || b[end] == UInt8(ascii: "=")) { end += 1 }
                if end - start >= 8 { out.append(Finding(range: start..<end, kind: .token)) }
            }
        }
    }

    /// Key names whose value is a secret, compared after lowercasing and dropping punctuation.
    static let SECRET_LABELS = ["password", "passwd", "passphrase", "secret", "token", "apikey", "accesskey", "privatekey", "credential", "authkey"]
    static let PASSWORD_LABELS = ["password", "passwd", "passphrase"]
    static let SHORT_LABELS = ["pwd", "pass", "pw", "pin"]

    /// Values that stand in for a secret rather than being one: types, placeholders, references.
    static func isPlaceholder(_ value: String) -> Bool {
        let v = RustText.asciiLowercased(value)
        let words = ["string", "str", "null", "none", "nil", "true", "false", "undefined", "required", "optional", "number", "boolean", "redacted", "hidden"]
        return words.contains { $0.isExactly(v) }
            || value.unicodeScalars.first.map { "$%{<([*&".unicodeScalars.contains($0) } == true
            || value.unicodeScalars.allSatisfy { "xX*.•".unicodeScalars.contains($0) }
            || ["process.env", "os.environ", "env."].contains { RustText.hasPrefix(v, $0) }
    }

    static func labelled(_ b: [UInt8], _ out: inout [Finding]) {
        for i in b.indices where b[i] == UInt8(ascii: "=") || b[i] == UInt8(ascii: ":") {
            let next: UInt8? = i + 1 < b.count ? b[i + 1] : nil
            let prev: UInt8 = i > 0 ? b[i - 1] : UInt8(ascii: " ")
            if b[i] == UInt8(ascii: ":") && (b[(i + 1)...].starts(with: "//".utf8) || next == UInt8(ascii: ":") || prev == UInt8(ascii: ":")) { continue }
            if b[i] == UInt8(ascii: "=") && (next == UInt8(ascii: "=") || "=!<>".utf8.contains(prev)) { continue }
            guard let key = keyBefore(b, i) else { continue }
            let normalized = String(decoding: b[key].filter(RustText.isASCIIAlphanumeric).map { (65...90).contains($0) ? $0 + 32 : $0 }, as: UTF8.self)
            let short = SHORT_LABELS.contains { $0.isExactly(normalized) }
            let secret = SECRET_LABELS.contains { RustText.contains(normalized, $0) } || short
            if !secret { continue }
            guard let value = valueAfter(b, i, arrow: next == UInt8(ascii: ">")) else { continue }
            let v = String(decoding: b[value], as: UTF8.self)
            if v.unicodeScalars.count < 4 || isPlaceholder(v) { continue }
            // `max_tokens=1024`, `token_count: 3`: counts, not secrets.
            if RustText.contains(normalized, "token") && v.utf8.allSatisfy({ (48...57).contains($0) }) { continue }
            let password = PASSWORD_LABELS.contains { RustText.contains(normalized, $0) } || short
            out.append(Finding(range: value, kind: password ? .password : .credential))
        }
    }

    /// The label in front of a separator at `sep`: up to three words, skipping a closing quote.
    static func keyBefore(_ b: [UInt8], _ sep: Int) -> Range<Int>? {
        var end = sep
        while end > 0 && (b[end - 1] == UInt8(ascii: " ") || b[end - 1] == UInt8(ascii: "\t")) { end -= 1 }
        if end > 0 && (b[end - 1] == UInt8(ascii: "\"") || b[end - 1] == UInt8(ascii: "'")) { end -= 1 }
        var start: Int?, at = end
        while let (c, s) = RustText.scalar(before: at, in: b), RustText.isAlphanumeric(c) || "_-. ".unicodeScalars.contains(c) {
            start = s; at = s
        }
        guard let start else { return nil }
        // `trim()`: the run holds no whitespace but spaces.
        var lo = start, hi = end
        while lo < hi && b[lo] == UInt8(ascii: " ") { lo += 1 }
        while hi > lo && b[hi - 1] == UInt8(ascii: " ") { hi -= 1 }
        let words = b[lo..<hi].split(separator: UInt8(ascii: " ")).count
        if lo == hi || words > 3 || hi - lo > 48 {
            // Take only the word nearest the separator.
            let last = (b[lo..<hi].lastIndex(of: UInt8(ascii: " ")).map { $0 + 1 } ?? lo)..<hi
            return last.isEmpty ? nil : last
        }
        return lo..<hi
    }

    static func valueAfter(_ b: [UInt8], _ sep: Int, arrow: Bool) -> Range<Int>? {
        var i = sep + 1 + (arrow ? 1 : 0)
        while i < b.count && (b[i] == UInt8(ascii: " ") || b[i] == UInt8(ascii: "\t")) { i += 1 }
        if i < b.count, case let q = b[i], q == UInt8(ascii: "\"") || q == UInt8(ascii: "'") || q == UInt8(ascii: "`") {
            let start = i + 1
            var j = start
            while j < b.count && b[j] != q && b[j] != UInt8(ascii: "\n") { j += b[j] == UInt8(ascii: "\\") ? 2 : 1 }
            let end = min(j, b.count)
            return end > start ? start..<end : nil
        }
        let start = min(i, b.count)
        let end = RustText.firstScalar(in: b, from: start) { RustText.isWhitespace($0) || ",;)}]\"'".unicodeScalars.contains($0) }
        return end > start ? start..<end : nil
    }
}
