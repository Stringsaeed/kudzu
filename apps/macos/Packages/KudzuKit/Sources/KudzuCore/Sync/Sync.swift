// Ported from kudzu-core/src/sync.rs
// iCloud sync model: how the library maps to CloudKit records and how changes merge.
//
// Pure logic with no CloudKit types, so every rule here is unit-tested. The desktop app's
// CloudKit layer moves [`RemoteRecord`]s in and out and calls these functions.
//
// # Records
//
// | Record type     | Name                | `payload`                                   |
// |-----------------|---------------------|---------------------------------------------|
// | `KudzuSnippet`  | `snippet-<uuid>`    | the snippet's JSON (same shape as exports)  |
// | `KudzuGroup`    | `group-<uuid>`      | the group's JSON                            |
// | `KudzuUsage`    | `usage-<device id>` | that Mac's usage rows (only it writes them) |
// | `KudzuMeta`     | `meta`              | `{ "syncVersion": n }`                      |
//
// Types are prefixed because the container also holds the Swift app's SwiftData (`CD_…`) types.
//
// Every record has a single string field, `payload`, holding JSON. The CloudKit schema never
// changes when Kudzu adds a field: new fields are new JSON keys.
//
// # Rules that keep mixed versions safe
//
// * **Unknown keys survive.** Encoding merges known fields over the last payload received from
//   the server, so an older Kudzu editing a snippet never erases fields a newer one added.
// * **Sync version gate.** `meta.syncVersion` names the record format. A Mac that finds a
//   version above [`SYNC_VERSION`] stops sending (it keeps receiving nothing it can't read) and
//   asks the user to update.
// * **Usage never conflicts.** Each Mac owns exactly one `Usage` record.
// * **Conflicts:** last writer wins per record, by `updatedAt` for snippets; deletions win.

import Foundation

/// Pure iCloud record translation, unknown-key preservation and conflict merging.
public enum Sync {
    public static let SYNC_VERSION = 1
    public static let ZONE_NAME = "Kudzu", PAYLOAD_FIELD = "payload", META_RECORD = "meta"
    public enum RecordKind: String, Codable, Hashable, Sendable, CaseIterable {
        case snippet = "Snippet", group = "Group", usage = "Usage", meta = "Meta"
        public var recordType: String { "Kudzu" + rawValue }
        /// Parses a CloudKit record type name.
        public static func fromRecordType(_ type: String) -> Self? { allCases.first { $0.recordType == type } }
    }
    /// Record name for a snippet.
    public static func snippetName(_ id: UUID) -> String { "snippet-" + id.lowercased }
    /// Record name for a group.
    public static func groupName(_ id: UUID) -> String { "group-" + id.lowercased }
    /// Record name for one Mac's usage rows.
    public static func usageName(_ deviceID: String) -> String { "usage-" + deviceID }
    public struct RemoteRecord: Equatable, Sendable, Codable {
        public var kind: RecordKind
        public var name: String
        public var payload: String
        public init(kind: RecordKind, name: String, payload: String) { self.kind = kind; self.name = name; self.payload = payload }
    }
    public struct IndexedRecord: Equatable, Sendable, Codable {
        public var kind: RecordKind
        public var payload: String
        public init(kind: RecordKind, payload: String) { self.kind = kind; self.payload = payload }
    }
    /// Last server payload per record. Unknown keys live here and survive local edits.
    public struct SyncIndex: Equatable, Sendable, Codable {
        public var records = ExactMap<IndexedRecord>()
        public var serverSyncVersion = 0
        public var initialMergeDone = false
        public var blockedByNewerFormat: Bool { serverSyncVersion > SYNC_VERSION }
        public init() {}
        enum CodingKeys: CodingKey { case records, serverSyncVersion, initialMergeDone }
        public init(from decoder: any Decoder) throws {
            let c = try decoder.container(keyedBy:CodingKeys.self)
            records = try c.decode(ExactMap<IndexedRecord>.self,forKey:.records)
            serverSyncVersion = try c.decodeIfPresent(Int.self,forKey:.serverSyncVersion) ?? 0
            initialMergeDone = try c.decodeIfPresent(Bool.self,forKey:.initialMergeDone) ?? false
        }
        public func encode(to encoder: any Encoder) throws {
            var c = encoder.container(keyedBy:CodingKeys.self)
            try c.encode(records,forKey:.records)
            try c.encode(serverSyncVersion,forKey:.serverSyncVersion)
            try c.encode(initialMergeDone,forKey:.initialMergeDone)
        }
    }
    public struct LocalChanges: Equatable, Sendable, Codable {
        public var saves: [String] = [], deletes: [String] = []
        public var isEmpty: Bool { saves.isEmpty && deletes.isEmpty }
        public init() {}
    }
    public struct Applied: Equatable, Sendable, Codable {
        public var libraryChanged = false, needsUpdate = false
        public init() {}
    }
    static let snippetKeys = ["id","name","triggers","content","formatting","groupID","fireMode","caseMode","insertion","rule","isFavorite","isEnabled","createdAt","updatedAt"]
    static let groupKeys = ["id","name","parentID","sortIndex","isEnabled","insertion","rule"]
    static func object<T: Encodable>(_ value: T) -> [String:Any] { ((try? KudzuJSON.encode(value,pretty:false)).flatMap { try? JSONSerialization.jsonObject(with:$0) } as? [String:Any]) ?? [:] }
    static func json(_ value: Any) -> String? { (try? JSONSerialization.data(withJSONObject:value,options:[.sortedKeys,.withoutEscapingSlashes])).flatMap { String(data:$0,encoding:.utf8) } }
    static func parsed(_ text: String) -> Any? { try? JSONSerialization.jsonObject(with:Data(text.utf8),options:.fragmentsAllowed) }
    static func sameModel<T: Equatable & Encodable>(_ a: T, _ b: T) -> Bool {
        // Swift String equality folds canonically equivalent Unicode; Rust compares UTF-8.
        a == b && (try? KudzuJSON.encode(a,pretty:false)) == (try? KudzuJSON.encode(b,pretty:false))
    }
    static func sameJSON(_ a: String, _ b: String) -> Bool {
        if let a = parsed(a) as? NSObject, let b = parsed(b) as? NSObject { return a == b }
        return a.isExactly(b)
    }
    /// `Uuid::parse_str`: simple, hyphenated, `{hyphenated}`, and lowercase `urn:uuid:`.
    static func rustUUID(_ text: String) -> UUID? {
        let bytes = Array(text.utf8)
        let hex: [UInt8]
        switch bytes.count {
        case 32: hex = bytes
        case 36: hex = unhyphenated(bytes)
        case 38 where bytes.first == UInt8(ascii: "{") && bytes.last == UInt8(ascii: "}"): hex = unhyphenated(Array(bytes[1..<37]))
        case 45 where bytes.starts(with: Array("urn:uuid:".utf8)): hex = unhyphenated(Array(bytes[9...]))
        default: return nil
        }
        guard hex.count == 32 else { return nil }
        var raw = [UInt8](repeating: 0, count: 16)
        for i in 0..<16 {
            guard let hi = hexDigit(hex[i * 2]), let lo = hexDigit(hex[i * 2 + 1]) else { return nil }
            raw[i] = hi << 4 | lo
        }
        return UUID(uuid: (raw[0], raw[1], raw[2], raw[3], raw[4], raw[5], raw[6], raw[7], raw[8], raw[9], raw[10], raw[11], raw[12], raw[13], raw[14], raw[15]))
    }
    static func unhyphenated(_ body: [UInt8]) -> [UInt8] {
        guard body.count == 36 else { return [] }
        var out: [UInt8] = []
        for (i, byte) in body.enumerated() {
            if i == 8 || i == 13 || i == 18 || i == 23 { if byte != UInt8(ascii: "-") { return [] } }
            else { out.append(byte) }
        }
        return out
    }
    static func hexDigit(_ byte: UInt8) -> UInt8? {
        switch byte {
        case 48...57: byte - 48
        case 65...70: byte - 55
        case 97...102: byte - 87
        default: nil
        }
    }
    /// `strip_prefix` plus `Uuid::parse_str`. The prefix is bytes, so a combining mark after `-` still strips.
    static func recordID(_ name: String, prefix: String) -> UUID? {
        guard RustText.hasPrefix(name, prefix) else { return nil }
        let bytes = Array(name.utf8)
        return rustUUID(String(decoding: bytes.dropFirst(prefix.utf8.count), as: UTF8.self))
    }
    /// Known omitted keys are removed; keys from newer versions are preserved.
    public static func encode(_ name: String, library: Library, index: SyncIndex, deviceID: String) -> RemoteRecord? {
        var fields: [String:Any], kind: RecordKind, known: [String]
        if name.isExactly(META_RECORD) { fields = ["syncVersion":max(index.serverSyncVersion,SYNC_VERSION)]; kind = .meta; known = ["syncVersion"] }
        else if name.isExactly(usageName(deviceID)) {
            let rows = library.usage.filter { $0.deviceID.isExactly(deviceID) }.sorted { $0.snippetID.lowercased < $1.snippetID.lowercased }
            fields = ["deviceID":deviceID,"rows":rows.map(object)]; kind = .usage; known = ["deviceID","rows"]
        } else if let id = recordID(name, prefix: "snippet-"), let s = library.snippets.first(where: { $0.id == id }) { fields = object(s); kind = .snippet; known = snippetKeys }
        else if let id = recordID(name, prefix: "group-"), let g = library.groups.first(where: { $0.id == id }) { fields = object(g); kind = .group; known = groupKeys }
        else { return nil }
        // Keep an `NSDictionary`: bridging to `[String:Any]` drops keys that differ only by normalization.
        let out = NSMutableDictionary(dictionary: index.records[name].flatMap { parsed($0.payload) as? NSDictionary } ?? NSDictionary())
        for k in known { out.removeObject(forKey:k) }
        for (k,v) in fields { out.setObject(v,forKey:k as NSString) }
        guard let payload = json(out) else { return nil }
        return RemoteRecord(kind:kind,name:name,payload:payload)
    }
    /// Local edits and deletions. Other devices' usage and meta are never deleted.
    public static func localChanges(library: Library, index: SyncIndex, deviceID: String) -> LocalChanges {
        var out = LocalChanges()
        if index.blockedByNewerFormat { return out }
        var names = library.groups.map { groupName($0.id) } + library.snippets.map { snippetName($0.id) }
        if library.usage.contains(where: { $0.deviceID.isExactly(deviceID) }) || index.records[usageName(deviceID)] != nil { names.append(usageName(deviceID)) }
        if index.serverSyncVersion < SYNC_VERSION { names.append(META_RECORD) }
        for name in names {
            guard let rec = encode(name,library:library,index:index,deviceID:deviceID) else { continue }
            if index.records[name].map({ sameJSON($0.payload,rec.payload) }) != true { out.saves.append(name) }
        }
        let present = presentNames(library)
        out.deletes = index.records.keys.filter { name in
            !present.contains(Data(name.utf8)) && [.snippet, .group].contains(index.records[name]?.kind)
        }.sorted { $0.utf8.lexicographicallyPrecedes($1.utf8) }
        return out
    }
    /// UTF-8 names of the snippets and groups in `library`.
    static func presentNames(_ library: Library) -> Set<Data> {
        var present = Set<Data>()
        for group in library.groups { present.insert(Data(groupName(group.id).utf8)) }
        for snippet in library.snippets { present.insert(Data(snippetName(snippet.id).utf8)) }
        return present
    }
    /// Names from a signed-out snapshot that this library no longer contains.
    /// A name still present was kept, so a later fetch may update it.
    public static func namesAbsent(from library: Library, names: [String]) -> Set<String> {
        let present = presentNames(library)
        return Set(names.filter { !present.contains(Data($0.utf8)) })
    }
    /// `snippet-` / `group-` plus a Rust UUID, the only names a signed-out deletion can tombstone.
    public static func tombstoneKind(_ name: String) -> RecordKind? {
        if recordID(name, prefix: "snippet-") != nil { return .snippet }
        if recordID(name, prefix: "group-") != nil { return .group }
        return nil
    }
    /// Records that the server accepted `record` (a save finished).
    public static func recordSaved(index: inout SyncIndex, record: RemoteRecord) {
        index.records[record.name] = IndexedRecord(kind:record.kind,payload:record.payload)
        if record.kind == .meta, let v = (parsed(record.payload) as? [String:Any])?["syncVersion"] as? Int { index.serverSyncVersion = max(index.serverSyncVersion,v) }
    }
    /// Records that the server confirmed a deletion.
    public static func recordDeleted(index: inout SyncIndex, name: String) { index.records.removeValue(forKey:name) }
    static func unsent(_ name: String, _ library: Library, _ index: SyncIndex, _ device: String) -> Bool {
        guard let rec = encode(name,library:library,index:index,deviceID:device) else { return false }
        return index.records[name].map { !sameJSON($0.payload,rec.payload) } ?? true
    }
    /// A local edit the server has a different copy of. A missing index entry is not an edit:
    /// sign-out drops the index, and treating that as unsent would ignore a remote deletion.
    static func hasDivergedLocalEdit(_ name: String, _ library: Library, _ index: SyncIndex, _ device: String) -> Bool {
        guard let rec = encode(name, library: library, index: index, deviceID: device), let known = index.records[name] else { return false }
        return !sameJSON(known.payload, rec.payload)
    }
    struct UsagePayload: Decodable { var deviceID: String; var rows: [UsageRecord] }
    /// Applies fetched records; last timestamp wins for snippets, unsent local groups win,
    /// usage is owned per device, and deletions win while retaining a group's children.
    ///
    /// `locallyDeleted` holds record names removed on this Mac while signed out. A fetched
    /// copy of one of those names is remembered and not recreated. Names the library still
    /// has are updates, not tombstones. A remote deletion of a snippet or group whose local
    /// payload differs from the indexed server copy keeps the local item and drops the index
    /// entry so the edit uploads again. A missing index entry is not that case.
    public static func applyRemote(library: inout Library, index: inout SyncIndex, records: [RemoteRecord], deleted: [String], deviceID: String, locallyDeleted: Set<String> = []) -> Applied {
        var out = Applied()
        let tombstones = Set(locallyDeleted.map { Data($0.utf8) })
        for rec in records {
            let data = Data(rec.payload.utf8)
            switch rec.kind {
            case .meta: recordSaved(index:&index,record:rec)
            case .snippet:
                guard let remote = try? KudzuJSON.decode(Snippet.self,from:data) else { continue }
                let pos = library.snippets.firstIndex { $0.id == remote.id }
                if tombstones.contains(Data(rec.name.utf8)), pos == nil {
                    recordSaved(index:&index,record:rec)
                    continue
                }
                let edit = unsent(rec.name,library,index,deviceID)
                let keep = edit && pos.map { library.snippets[$0].updatedAt > remote.updatedAt } == true
                recordSaved(index:&index,record:rec)
                if keep { continue }
                if let pos { if !sameModel(library.snippets[pos],remote) { library.snippets[pos] = remote; out.libraryChanged = true } }
                else { library.snippets.append(remote); out.libraryChanged = true }
            case .group:
                guard let remote = try? KudzuJSON.decode(Group.self,from:data) else { continue }
                let pos = library.groups.firstIndex { $0.id == remote.id }
                if tombstones.contains(Data(rec.name.utf8)), pos == nil {
                    recordSaved(index:&index,record:rec)
                    continue
                }
                let edit = unsent(rec.name,library,index,deviceID)
                recordSaved(index:&index,record:rec)
                if edit && pos != nil { continue }
                if let pos { if !sameModel(library.groups[pos],remote) { library.groups[pos] = remote; out.libraryChanged = true } }
                else { library.groups.append(remote); out.libraryChanged = true }
            case .usage:
                guard var remote = try? KudzuJSON.decode(UsagePayload.self,from:data) else { continue }
                recordSaved(index:&index,record:rec)
                if remote.deviceID.isExactly(deviceID) {
                    for row in remote.rows {
                        if let i = library.usage.firstIndex(where: { $0.deviceID.isExactly(deviceID) && $0.snippetID == row.snippetID }) {
                            if library.usage[i].count < row.count { library.usage[i] = row; out.libraryChanged = true }
                        } else { library.usage.append(row); out.libraryChanged = true }
                    }
                } else {
                    for i in remote.rows.indices { remote.rows[i].deviceID = remote.deviceID }
                    if library.usage.filter({ $0.deviceID.isExactly(remote.deviceID) }) != remote.rows {
                        library.usage.removeAll { $0.deviceID.isExactly(remote.deviceID) }; library.usage += remote.rows; out.libraryChanged = true
                    }
                }
            }
        }
        for name in deleted {
            if hasDivergedLocalEdit(name, library, index, deviceID) {
                index.records.removeValue(forKey: name)
                continue
            }
            index.records.removeValue(forKey:name)
            if let id = recordID(name, prefix: "snippet-") {
                let before = library.snippets.count; library.snippets.removeAll { $0.id == id }; out.libraryChanged = out.libraryChanged || before != library.snippets.count
            } else if let id = recordID(name, prefix: "group-") {
                let before = library.groups.count; library.groups.removeAll { $0.id == id }
                if before != library.groups.count {
                    out.libraryChanged = true
                    for i in library.groups.indices where library.groups[i].parentID == id { library.groups[i].parentID = nil }
                    for i in library.snippets.indices where library.snippets[i].groupID == id { library.snippets[i].groupID = nil }
                }
            }
        }
        out.needsUpdate = index.blockedByNewerFormat; return out
    }
    /// Folds duplicate starter libraries once, after the first complete fetch.
    public static func initialMerge(library: inout Library, index: inout SyncIndex) -> Bool {
        if index.initialMergeDone { return false }; index.initialMergeDone = true
        let merged = Dedupe.dedupe(library).library
        if merged == library { return false }; library = merged; return true
    }
}
