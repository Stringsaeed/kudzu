// Ported from kudzu-core/src/agent/tools.rs
// The tools an agent can call, their JSON schemas, and what they do.

import Foundation

extension Agent {
    /// Drops the last partial word of a truncated excerpt before detection.
    static func entryRedaction(_ e: ClipEntry, _ text: String, _ complete: Bool) -> Secrets.Redacted {
        if e.secretOverride == false { return .init(text:text,kinds:[],whole:false) }
        if e.secretOverride == true { return .init(text:"",kinds:[e.secret ?? .credential],whole:true) }
        if complete { return Secrets.redact(text) }
        let cut = text.lastIndex(where: \.isWhitespace) ?? text.startIndex
        var r = Secrets.redact(String(text[..<cut])); r.whole = false; return r
    }
    static func clipSummary(_ e: ClipEntry, _ text: String?, _ complete: Bool, _ max: Int) -> [String:Any] {
        var v: [String:Any] = ["id":e.id.lowercased,"kind":e.kind.rawValue,"copiedAt":KudzuDate.formatWholeSeconds(e.copiedAt),"copyCount":e.copyCount,"pinned":e.pinned,"length":e.textLen,"lines":e.lineCount]
        if let app = e.source { v["app"] = app.name }; if !e.files.isEmpty { v["files"] = e.files }
        if let image = e.image { v["image"] = ["width":image.width,"height":image.height] }; if let color = e.color { v["color"] = color }
        if let text {
            let r = entryRedaction(e,text,complete)
            if r.whole {
                v["text"] = NSNull(); v.removeValue(forKey:"length"); v.removeValue(forKey:"lines")
                v["withheld"] = e.secretOverride == true ? "The user marked this as a secret, so Kudzu doesn't share it." : "This looks like a \(r.kinds.first?.label ?? "secret"), so Kudzu doesn't share it."
            } else { let (shown,cut) = truncate(r.text,max); v["text"] = shown; if cut || !complete { v["truncated"] = true } }
            if !r.isClean { v["redacted"] = r.kinds.map(\.label) }
        }; return v
    }
    static func listClipboard(_ entries: [ClipEntry], _ args: [String:Any]) throws -> [String:Any] {
        let limit = max(1,try count(args,"limit",20,100)), raw = try string(args,"kind"), pinned = try boolean(args,"pinnedOnly") ?? false
        var kind: ClipKind?
        if let raw { guard let parsed = ClipKind(rawValue:raw) else { throw Failure("Unknown kind “\(raw)”: use text, code, link, color, image, file or other.") }; kind = parsed }
        let query = try string(args,"query")?.trimmingCharacters(in:.whitespacesAndNewlines).lowercased() ?? "", terms = query.split(whereSeparator: \.isWhitespace).map(String.init)
        var total = 0, items: [[String:Any]] = []
        for e in entries.sorted(by: { $0.copiedAt > $1.copiedAt }) {
            if (kind != nil && kind != e.kind) || (pinned && !e.pinned) { continue }
            let complete = e.textLen <= Clipboard.TEXT_EXCERPT_LIMIT
            if !terms.isEmpty {
                let r = e.text.map { entryRedaction(e,$0,complete) }
                let text = r.flatMap { $0.whole ? nil : $0.text } ?? "", hay = [text,e.files.joined(separator:"\n"),e.source?.name ?? ""].joined(separator:"\n").lowercased()
                if !terms.allSatisfy(hay.contains) { continue }
            }
            total += 1; if items.count < limit { items.append(clipSummary(e,e.text,complete,320)) }
        }; return ["total":total,"returned":items.count,"items":items]
    }
    static func getClipboardItem(_ host: any Host, _ args: [String:Any]) throws -> [String:Any] {
        guard let raw = try string(args,"id") else { throw Failure("Pass the `id` of a clipboard item from list_clipboard.") }
        let id = try parseID(raw,"clipboard item")
        guard let e = try host.clipboard().first(where: { $0.id == id }) else { throw Failure("That item is no longer in the clipboard history.") }
        let full = try host.clipboardText(id)
        return clipSummary(e,full ?? e.text,full != nil || e.textLen <= Clipboard.TEXT_EXCERPT_LIMIT,50000)
    }
}
