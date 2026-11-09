// Ported from kudzu-core/src/clipboard.rs
// Clipboard history: what a copy *is*, when it happened, and how long it's kept.
//
// Pure logic only. The engine reads the pasteboard as raw `(type, bytes)` pairs per item; this
// module classifies them ([`classify`]), folds repeats of the same content into one entry with
// its copy timings ([`History::ingest`]), applies the retention window ([`History::prune`]) and
// encodes the raw data for exact restoration ([`encode_items`] / [`decode_items`]).
//
// Entries hold metadata and a bounded text excerpt; full data lives in a blob next to the index.

import Foundation

extension Clipboard {
    /// Serializes pasteboard items: KZC1, counts, and type/data pairs with LE u32/u64 lengths.
    public static func encodeItems(_ items: [PasteItem]) -> Data {
        var out = Data("KZC1".utf8)
        func integer<T: FixedWidthInteger>(_ n: T) { var le = n.littleEndian; withUnsafeBytes(of:&le) { out.append(contentsOf:$0) } }
        integer(UInt32(truncatingIfNeeded:items.count))
        for item in items {
            integer(UInt32(truncatingIfNeeded:item.count))
            for (type,data) in item { integer(UInt32(truncatingIfNeeded:type.utf8.count)); out.append(contentsOf:type.utf8); integer(UInt64(data.count)); out.append(data) }
        }
        return out
    }
    /// Inverse of `encodeItems`; nil on every malformed input, including trailing bytes.
    public static func decodeItems(_ bytes: Data) -> [PasteItem]? {
        let b = Array(bytes); var at = 0
        func take(_ n: Int) -> Data? {
            guard n >= 0 && n <= b.count-at else { return nil }
            defer { at += n }; return Data(b[at..<(at+n)])
        }
        func integer(_ n: Int) -> UInt64? {
            guard let d = take(n) else { return nil }
            return d.enumerated().reduce(UInt64(0)) { $0 | (UInt64($1.element) << ($1.offset*8)) }
        }
        guard take(4) == Data("KZC1".utf8), let count = integer(4), count <= UInt64((b.count-at)/4) else { return nil }
        var items: [PasteItem] = []
        for _ in 0..<count {
            guard let n = integer(4), n <= UInt64((b.count-at)/12) else { return nil }
            var item: PasteItem = []
            for _ in 0..<n {
                guard let tl = integer(4), tl <= UInt64(b.count-at), let td = take(Int(tl)), let type = RustText.utf8String(td), let dl = integer(8), dl <= UInt64(b.count-at), let data = take(Int(dl)) else { return nil }
                item.append((type,data))
            }
            items.append(item)
        }
        return at == b.count ? items : nil
    }
    /// Full plain text of decoded items, if any.
    public static func textOf(_ items: [PasteItem]) -> String? { (find(items,uti.PLAIN_TEXT) ?? find(items,uti.LEGACY_TEXT)).flatMap { RustText.utf8String($0) } }
    /// Bytes and MIME type of the best image in decoded items.
    public static func imageOf(_ items: [PasteItem]) -> (data: Data, mime: String)? {
        for (type,mime) in uti.IMAGES { if let data = find(items,type) { return (data,mime) } }
        return nil
    }
    /// MIME type for an image file path a webview can display, by extension.
    public static func imageMimeForPath(_ path: String) -> String? {
        let b = Array(path.utf8)
        guard let dot = b.lastIndex(of: UInt8(ascii: ".")) else { return nil }
        switch RustText.asciiLowercased(String(decoding: b[(dot + 1)...], as: UTF8.self)) {
        case "png": return "image/png"
        case "jpg","jpeg": return "image/jpeg"
        case "gif": return "image/gif"
        case "webp": return "image/webp"
        case "heic","heif": return "image/heic"
        case "tif","tiff": return "image/tiff"
        case "bmp": return "image/bmp"
        case "avif": return "image/avif"
        default: return nil
        }
    }
}
