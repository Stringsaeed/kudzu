import Foundation

extension Clipboard {
    public struct Filter: Equatable, Sendable { public var value: String; public var label: String; public var short: String }
    public static let FILTERS: [Filter] = [("all","All"),("pinned","Pinned"),("text","Text"),("image","Images"),("link","Links"),("file","Files"),("code","Code"),("color","Colors")].map { Filter(value:$0.0,label:$0.1,short:$0.1) }
    public static func kindLabel(_ kind: ClipKind) -> String { kind.rawValue.capitalized }
    public static func isSecret(_ e: ClipEntry) -> Bool { e.isSecret }
    public static func secretReason(_ e: ClipEntry) -> String {
        guard let kind = e.secret else { return "Marked as secret" }
        let article = kind == .apiKey || kind == .token ? "an" : "a"
        return "Looks like \(article) \(kind.label)"
    }
    public static func hasText(_ e: ClipEntry) -> Bool { [.text,.code,.link,.color].contains(e.kind) }
    /// Text also covers other app data, as in the web filter.
    public static func inFilter(_ e: ClipEntry, filter: String) -> Bool {
        if filter == "all" { return true }; if filter == "pinned" { return e.pinned }
        if filter == "text" { return e.kind == .text || e.kind == .other }; return e.kind.rawValue == filter
    }
    public struct ParsedQuery: Equatable, Sendable { public var terms: [String]; public var app: String?; public var kind: ClipKind? }
    public static func parseQuery(_ query: String) -> ParsedQuery {
        var out = ParsedQuery(terms:[],app:nil,kind:nil)
        let aliases: [String:ClipKind] = ["text":.text,"code":.code,"link":.link,"links":.link,"url":.link,"color":.color,"colour":.color,"image":.image,"images":.image,"img":.image,"file":.file,"files":.file,"other":.other]
        // `q.toLowerCase().split(/\s+/)`, `startsWith` and `slice` work on UTF-16 code units.
        for raw in ECMAScriptWhitespace.split(query.lowercased()) {
            let units = Array(raw.utf16)
            func rest(_ prefix: String) -> String? { units.starts(with: prefix.utf16) ? String(decoding: units.dropFirst(prefix.utf16.count), as: UTF16.self) : nil }
            if let app = rest("app:") { out.app = app }
            else if let alias = rest("is:"), let k = aliases.exactValue(for: alias) { out.kind = k }
            else { out.terms.append(raw) }
        }; return out
    }
    public static func searchClips(_ entries: [ClipEntry], filter: String = "all", query: String) -> [ClipEntry] {
        let q = parseQuery(query)
        return entries.filter { e in
            guard inFilter(e,filter:filter), q.kind == nil || q.kind == e.kind else { return false }
            if let app = q.app, !app.isEmpty, !(e.source.map { includes($0.name.lowercased(),app) || includes($0.bundleId.lowercased(),app) } ?? false) { return false }
            let h = [e.text ?? "",e.files.map(basename).joined(separator:" "),e.color ?? "",kindLabel(e.kind)].joined(separator:"\n").lowercased()
            return q.terms.allSatisfy { includes(h,$0) }
        }
    }
    /// JavaScript `String.prototype.includes`: a UTF-16 code-unit search, not canonical equivalence.
    static func includes(_ haystack: String, _ needle: String) -> Bool {
        needle.isEmpty || (haystack as NSString).range(of: needle, options: .literal).location != NSNotFound
    }
    /// UTF-16 match ranges, merged for highlighting, bounded at 64 matches.
    public static func matchRanges(_ text: String, terms: [String]) -> [Range<Int>] {
        let lower = text.lowercased() as NSString; var ranges: [Range<Int>] = []
        for term in terms where !term.isEmpty {
            var at = 0
            while at <= lower.length && ranges.count < 64 {
                let r = lower.range(of:term,options:.literal,range:NSRange(location:at,length:lower.length-at))
                if r.location == NSNotFound { break }; ranges.append(r.location..<(r.location+r.length)); at = r.location+r.length
            }
        }
        ranges.sort { $0.lowerBound < $1.lowerBound }; var merged: [Range<Int>] = []
        for r in ranges { if let last = merged.last, r.lowerBound <= last.upperBound { merged[merged.count-1] = last.lowerBound..<max(last.upperBound,r.upperBound) } else { merged.append(r) } }
        return merged
    }
    public enum ClipRow: Equatable, Sendable {
        case header(key: String, label: String, count: Int)
        case entry(key: String, entry: ClipEntry, index: Int, dated: Bool)
    }
    static func dayKey(_ date: Date, _ calendar: Calendar) -> String {
        let c = calendar.dateComponents([.year,.month,.day],from:date)
        return "\(c.year ?? 0)-\((c.month ?? 1)-1)-\(c.day ?? 0)"
    }
    static func formatted(_ date: Date, _ template: String, calendar: Calendar = .current) -> String {
        let f = DateFormatter(); f.calendar = calendar; f.timeZone = calendar.timeZone; f.locale = .current; f.setLocalizedDateFormatFromTemplate(template); return f.string(from:date)
    }
    public static func dayLabel(_ date: Date, now: Date = Date(), calendar: Calendar = .current) -> String {
        if calendar.isDate(date,inSameDayAs:now) { return "Today" }
        if let y = calendar.date(byAdding:.day,value:-1,to:now), calendar.isDate(date,inSameDayAs:y) { return "Yesterday" }
        if calendar.startOfDay(for:now).timeIntervalSince(date)/86400 < 6 { return formatted(date,"EEEE",calendar:calendar) }
        let sameYear = calendar.component(.year,from:date) == calendar.component(.year,from:now)
        return formatted(date,sameYear ? "EEE MMM d" : "EEE MMM d y",calendar:calendar)
    }
    public static func buildRows(_ entries: [ClipEntry], pinnedFirst: Bool, sections: Bool, now: Date = Date(), calendar: Calendar = .current) -> (rows: [ClipRow], order: [ClipEntry]) {
        let pinned = pinnedFirst ? entries.filter(\.pinned) : [], rest = pinnedFirst ? entries.filter { !$0.pinned } : entries
        var rows: [ClipRow] = [], order: [ClipEntry] = []
        func push(_ e: ClipEntry, _ dated: Bool) { rows.append(.entry(key:e.id.lowercased,entry:e,index:order.count,dated:dated)); order.append(e) }
        if !pinned.isEmpty { if sections { rows.append(.header(key:"h-pinned",label:"Pinned",count:pinned.count)) }; for e in pinned { push(e,true) } }
        var current = "", header: Int?
        for e in rest {
            if sections {
                let key = dayKey(e.copiedAt,calendar)
                if key != current { current = key; header = rows.count; rows.append(.header(key:"h-"+key,label:dayLabel(e.copiedAt,now:now,calendar:calendar),count:0)) }
                if let i = header, case .header(let key,let label,let count) = rows[i] { rows[i] = .header(key:key,label:label,count:count+1) }
            }; push(e,!sections)
        }; return (rows,order)
    }
    /// `path.replace(/\/+$/, "")`: only at the very end (an ICU `$` would also match before a final newline).
    static func trimTrailingSlashes(_ path: String) -> [UInt16] {
        var units = Array(path.utf16)
        while units.last == UInt16(UInt8(ascii: "/")) { units.removeLast() }
        return units
    }
    public static func basename(_ path: String) -> String {
        let p = trimTrailingSlashes(path), name = p[((p.lastIndex(of: UInt16(UInt8(ascii: "/"))) ?? -1) + 1)...]
        return String(decoding: name.isEmpty ? p[...] : name, as: UTF16.self)
    }
    public static func dirname(_ path: String) -> String {
        let p = trimTrailingSlashes(path)
        guard let i = p.lastIndex(of: UInt16(UInt8(ascii: "/"))), i > 0 else { return "/" }
        return String(decoding: p[..<i], as: UTF16.self)
    }
    public static func tildify(_ path: String) -> String { path.replacingOccurrences(of:"^/Users/[^/]+",with:"~",options:.regularExpression) }
    public static func formatBytes(_ n: Int) -> String {
        if n < 1024 { return "\(n) B" }; var v = Double(n)/1024, u = 0; let units = ["KB","MB","GB"]
        while v >= 1024 && u < 2 { v /= 1024; u += 1 }
        let digits = v < 10 ? 1 : 0, scale = v < 10 ? 10.0 : 1.0
        let rounded = floor(v*scale+0.5)/scale
        return String(format:digits == 1 ? "%.1f %@" : "%.0f %@",rounded,units[u])
    }
    public static func imageFormat(_ e: ClipEntry) -> String {
        guard let t = e.types.first(where: { $0.range(of:"png|jpeg|tiff|gif|heic|webp",options:.regularExpression) != nil }) else { return "Image" }
        return [("png","PNG"),("jpeg","JPEG"),("tiff","TIFF"),("gif","GIF"),("heic","HEIC"),("webp","WebP")].first { t.contains($0.0) }?.1 ?? "Image"
    }
    public static func describe(_ e: ClipEntry) -> (title: String, subtitle: String) {
        let text = e.text ?? ""
        switch e.kind {
        case .link:
            // `new URL(…)` (WHATWG) is approximated with `URLComponents`; a parse or `decodeURI` failure
            // lands in the TS `catch`: the raw text twice. Known gaps: no IDNA (punycode) hosts and no
            // dot-segment normalization of the path.
            let raw = ECMAScriptWhitespace.trim(text)
            let absolute = RustText.hasPrefix(RustText.asciiLowercased(raw),"www.") ? "https://"+raw : raw
            guard let u = URLComponents(string:absolute), let scheme = u.scheme.map(RustText.asciiLowercased) else { return (raw,raw) }
            if scheme == "mailto" { return (u.percentEncodedPath,"Email address") }
            // `URL.host` leaves out the scheme's default port.
            let defaultPort = ["http":80,"https":443,"ftp":21,"ws":80,"wss":443][scheme]
            var host = (u.percentEncodedHost ?? "").lowercased()
            if RustText.hasPrefix(host,"www.") { host = String(host.dropFirst(4)) }
            if let port = u.port, port != defaultPort { host += ":\(port)" }
            // `URLComponents` rewrites a `%` that is not two hex digits into `%25`. `pathname + search`
            // keeps that escape, and `decodeURI` throws.
            guard let rest = decodeURI(pathAndSearch(absolute)) else { return (raw,raw) }
            let subtitle = RustText.hasSuffix(rest,"/") ? String(rest.dropLast()) : rest
            return (host,subtitle.isEmpty ? raw : subtitle)
        case .color: return ((e.color ?? text).uppercased(),"Color")
        case .image: return (e.image.map { "Image \($0.width) × \($0.height)" } ?? "Image","\(imageFormat(e)) · \(formatBytes(Int(clamping:e.size)))")
        case .file:
            let first = e.files.first ?? ""
            return (e.files.count > 1 ? "\(basename(first)) and \(e.files.count-1) more" : basename(first),tildify(dirname(first)))
        case .other: return (e.source.map { "Content from \($0.name)" } ?? "Data","\(e.types.count) format\(e.types.count == 1 ? "" : "s") · \(formatBytes(Int(clamping:e.size)))")
        default:
            // `line.trim()` and `/\s+/g` are the ECMAScript whitespace set.
            let lines = text.components(separatedBy:"\n").map(ECMAScriptWhitespace.trim).filter { !$0.isEmpty }.map(ECMAScriptWhitespace.collapse)
            let count = e.textLen.formatted()
            return (lines.first ?? ECMAScriptWhitespace.trim(text),lines.count > 1 ? lines[1] : "\(count) character\(e.textLen == 1 ? "" : "s")")
        }
    }
    /// `pathname + search` from the original URL text, before `#`. Not `percentEncodedPath`.
    static func pathAndSearch(_ url: String) -> String {
        let s = Array(url.unicodeScalars)
        var i = 0
        while i < s.count, s[i] != ":" { i += 1 }
        if i + 2 < s.count, s[i] == ":", s[i + 1] == "/", s[i + 2] == "/" {
            i += 3
            if i < s.count, s[i] == "[" { while i < s.count, s[i] != "]" { i += 1 }; if i < s.count { i += 1 } }
            while i < s.count, s[i] != "/", s[i] != "?", s[i] != "#" { i += 1 }
        } else if i < s.count, s[i] == ":" { i += 1 }
        let start = i
        while i < s.count, s[i] != "#" { i += 1 }
        return String(String.UnicodeScalarView(s[start..<i]))
    }
    /// JavaScript `decodeURI`: keeps escapes of `;/?:@&=+$,#`; `nil` where JS throws `URIError`.
    static func decodeURI(_ source: String) -> String? {
        let s = Array(source.unicodeScalars)
        let reserved = Set<Unicode.Scalar>(";/?:@&=+$,#".unicodeScalars)
        var out = "", k = 0
        func hex(_ scalar: Unicode.Scalar) -> UInt8? {
            switch scalar.value {
            case 48...57: return UInt8(scalar.value - 48)
            case 65...70: return UInt8(scalar.value - 55)
            case 97...102: return UInt8(scalar.value - 87)
            default: return nil
            }
        }
        func percentByte() -> UInt8? {
            guard k + 2 < s.count, s[k] == "%", let hi = hex(s[k + 1]), let lo = hex(s[k + 2]) else { return nil }
            k += 3
            return hi << 4 | lo
        }
        while k < s.count {
            if s[k] != "%" { out.unicodeScalars.append(s[k]); k += 1; continue }
            let start = k
            guard let lead = percentByte() else { return nil }
            if lead < 0x80 {
                let ch = Unicode.Scalar(lead)
                if reserved.contains(ch) { for i in start..<k { out.unicodeScalars.append(s[i]) } } else { out.unicodeScalars.append(ch) }
                continue
            }
            let extra: Int
            switch lead {
            case 0xC2...0xDF: extra = 1
            case 0xE0...0xEF: extra = 2
            case 0xF0...0xF4: extra = 3
            default: return nil
            }
            var bytes = [lead]
            for _ in 0..<extra {
                guard let b = percentByte(), (0x80...0xBF).contains(b) else { return nil }
                bytes.append(b)
            }
            guard let text = String(bytes: bytes, encoding: .utf8), let scalar = text.unicodeScalars.first, text.unicodeScalars.count == 1, scalar.value >= 0x80 else { return nil }
            out.unicodeScalars.append(scalar)
        }
        return out
    }
    /// `text.match(/\S+/g).length`.
    public static func wordCount(_ text: String) -> Int { ECMAScriptWhitespace.split(text).count }
    public static func clipTime(_ date: Date, now: Date = Date(), dated: Bool = false) -> String {
        let diff = max(0,now.timeIntervalSince(date))
        if diff < 45 { return "Just now" }; if diff < 3600 { return "\(Int((diff/60).rounded()))m ago" }
        let time = formatted(date,"jm")
        if !dated { return time }; let day = dayLabel(date,now:now)
        if day == "Today" { return time }; if day == "Yesterday" { return day }
        return formatted(date,diff < 6*86400 ? "EEE" : "MMM d")
    }
    public static func exactTime(_ date: Date, now: Date = Date()) -> String {
        let day = dayLabel(date,now:now)
        if day == "Today" || day == "Yesterday" { return "\(day) at \(formatted(date,"jms"))" }
        let f = DateFormatter(); f.dateStyle = .medium; f.timeStyle = .short; return f.string(from:date)
    }
    public static func expiresIn(_ e: ClipEntry, retentionDays: Int, now: Date = Date()) -> String {
        if e.pinned { return "Never (pinned)" }; if retentionDays == 0 { return "When history is full" }
        let hours = (e.copiedAt.addingTimeInterval(Double(retentionDays)*86400).timeIntervalSince(now))/3600
        if hours <= 1 { return "Within the hour" }; return hours < 36 ? "In \(Int(hours.rounded())) hours" : "In \(Int((hours/24).rounded())) days"
    }
    public static let RETENTION_OPTIONS = [(days:1,label:"1 day"),(7,"1 week"),(30,"1 month"),(90,"3 months"),(365,"1 year"),(0,"Forever")]
    public static func retentionLabel(_ days: Int) -> String {
        if days == 0 { return "until you clear it" }; if days == 1 { return "for 1 day" }
        for (unit,size) in [("year",365),("month",30),("week",7)] where days >= size && days % size == 0 { let n = days/size; return "for \(n) \(unit)\(n == 1 ? "" : "s")" }
        return "for \(days) days"
    }
    /// `text.slice(0, n)` in UTF-16 code units; a cut surrogate pair is dropped (Swift can't hold half).
    static func jsSlice(_ text: String, _ n: Int) -> String {
        var units = Array(text.utf16.prefix(max(0, n)))
        if let last = units.last, UTF16.isLeadSurrogate(last) { units.removeLast() }
        return String(decoding: units, as: UTF16.self)
    }
    public static func spokenLabel(_ e: ClipEntry, concealed: Bool = false) -> String {
        var parts = concealed ? ["Hidden secret",secretReason(e).lowercased(),"copied \(exactTime(e.copiedAt))"] : [kindLabel(e.kind),jsSlice(describe(e).title,120),"copied \(exactTime(e.copiedAt))"]
        if let app = e.source { parts.append("from \(app.name)") }; if e.copyCount > 1 { parts.append("\(e.copyCount) times") }; if e.pinned { parts.append("pinned") }; return parts.joined(separator:", ")
    }
}
