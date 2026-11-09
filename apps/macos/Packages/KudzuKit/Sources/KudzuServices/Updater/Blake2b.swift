import Foundation

/// BLAKE2b (RFC 7693), unkeyed, in pure Swift. minisign's prehashed signatures (`ED`) sign the
/// BLAKE2b-512 digest of the file, so the updater needs it to verify release archives.
public struct Blake2b: Sendable {
    private static let iv: [UInt64] = [
        0x6A09_E667_F3BC_C908, 0xBB67_AE85_84CA_A73B, 0x3C6E_F372_FE94_F82B, 0xA54F_F53A_5F1D_36F1,
        0x510E_527F_ADE6_82D1, 0x9B05_688C_2B3E_6C1F, 0x1F83_D9AB_FB41_BD6B, 0x5BE0_CD19_137E_2179,
    ]

    private static let sigma: [[Int]] = [
        [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15],
        [14, 10, 4, 8, 9, 15, 13, 6, 1, 12, 0, 2, 11, 7, 5, 3],
        [11, 8, 12, 0, 5, 2, 15, 13, 10, 14, 3, 6, 7, 1, 9, 4],
        [7, 9, 3, 1, 13, 12, 11, 14, 2, 6, 5, 10, 4, 0, 15, 8],
        [9, 0, 5, 7, 2, 4, 10, 15, 14, 1, 11, 12, 6, 8, 3, 13],
        [2, 12, 6, 10, 0, 11, 8, 3, 4, 13, 7, 5, 15, 14, 1, 9],
        [12, 5, 1, 15, 14, 13, 4, 10, 0, 7, 6, 3, 9, 2, 8, 11],
        [13, 11, 7, 14, 12, 1, 3, 9, 5, 0, 15, 4, 8, 6, 2, 10],
        [6, 15, 14, 9, 11, 3, 0, 8, 12, 2, 13, 7, 1, 4, 10, 5],
        [10, 2, 8, 4, 7, 6, 1, 5, 15, 11, 9, 14, 3, 12, 13, 0],
    ]

    private var h: [UInt64]
    private var buffer = [UInt8]()
    private var counter: (low: UInt64, high: UInt64) = (0, 0)
    public let outputLength: Int

    /// A hasher producing `outputLength` bytes (1…64; minisign uses 64).
    public init(outputLength: Int = 64) {
        precondition((1 ... 64).contains(outputLength), "BLAKE2b digests are 1…64 bytes")
        self.outputLength = outputLength
        h = Self.iv
        // Parameter block: digest length, key length 0, fanout 1, depth 1.
        h[0] ^= 0x0101_0000 ^ UInt64(outputLength)
        buffer.reserveCapacity(128)
    }

    /// The BLAKE2b-512 digest of `data`.
    public static func hash512(_ data: Data) -> [UInt8] {
        var b = Blake2b()
        b.update(data)
        return b.finalize()
    }

    public mutating func update(_ data: Data) {
        data.withUnsafeBytes { update(UnsafeRawBufferPointer($0)) }
    }

    public mutating func update(_ bytes: [UInt8]) {
        bytes.withUnsafeBytes { update($0) }
    }

    public mutating func update(_ bytes: UnsafeRawBufferPointer) {
        var offset = 0
        let count = bytes.count
        while offset < count {
            // The last block is compressed in `finalize`, so only flush a full buffer when more follows.
            if buffer.count == 128 {
                increment(128)
                compress(buffer, last: false)
                buffer.removeAll(keepingCapacity: true)
            }
            let take = min(128 - buffer.count, count - offset)
            buffer.append(contentsOf: bytes[offset ..< offset + take])
            offset += take
        }
    }

    public mutating func finalize() -> [UInt8] {
        increment(UInt64(buffer.count))
        let block = buffer + [UInt8](repeating: 0, count: 128 - buffer.count)
        compress(block, last: true)
        var out = [UInt8]()
        out.reserveCapacity(64)
        for word in h {
            withUnsafeBytes(of: word.littleEndian) { out.append(contentsOf: $0) }
        }
        return Array(out.prefix(outputLength))
    }

    private mutating func increment(_ n: UInt64) {
        let (low, overflow) = counter.low.addingReportingOverflow(n)
        counter = (low, counter.high &+ (overflow ? 1 : 0))
    }

    private mutating func compress(_ block: [UInt8], last: Bool) {
        var m = [UInt64](repeating: 0, count: 16)
        block.withUnsafeBytes { raw in
            for i in 0 ..< 16 {
                m[i] = UInt64(littleEndian: raw.loadUnaligned(fromByteOffset: i * 8, as: UInt64.self))
            }
        }
        var v = h + Self.iv
        v[12] ^= counter.low
        v[13] ^= counter.high
        if last { v[14] = ~v[14] }

        @inline(__always) func g(_ a: Int, _ b: Int, _ c: Int, _ d: Int, _ x: UInt64, _ y: UInt64) {
            v[a] = v[a] &+ v[b] &+ x
            v[d] = (v[d] ^ v[a]).rotatedRight(32)
            v[c] = v[c] &+ v[d]
            v[b] = (v[b] ^ v[c]).rotatedRight(24)
            v[a] = v[a] &+ v[b] &+ y
            v[d] = (v[d] ^ v[a]).rotatedRight(16)
            v[c] = v[c] &+ v[d]
            v[b] = (v[b] ^ v[c]).rotatedRight(63)
        }

        for round in 0 ..< 12 {
            let s = Self.sigma[round % 10]
            g(0, 4, 8, 12, m[s[0]], m[s[1]])
            g(1, 5, 9, 13, m[s[2]], m[s[3]])
            g(2, 6, 10, 14, m[s[4]], m[s[5]])
            g(3, 7, 11, 15, m[s[6]], m[s[7]])
            g(0, 5, 10, 15, m[s[8]], m[s[9]])
            g(1, 6, 11, 12, m[s[10]], m[s[11]])
            g(2, 7, 8, 13, m[s[12]], m[s[13]])
            g(3, 4, 9, 14, m[s[14]], m[s[15]])
        }
        for i in 0 ..< 8 { h[i] ^= v[i] ^ v[i + 8] }
    }
}

extension UInt64 {
    @inline(__always) fileprivate func rotatedRight(_ n: UInt64) -> UInt64 {
        (self >> n) | (self << (64 - n))
    }
}
