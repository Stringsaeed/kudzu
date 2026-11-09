// Ported from kudzu-core/src/interchange/coordinator.rs
// Import orchestration: previews, safety backups, merge + dedupe, restore, export and starter
// snippets. Works on plain [`KudzuLibrary`] values; the caller persists the result.

import Foundation

/// Import previews, safety backups, merge/dedupe, restore, exports and onboarding snippets.
public enum Coordinator {
    public static let SAMPLE_TRIGGER = ";kudzu", SAMPLE_EXPANSION = "Kudzu is working!", GETTING_STARTED_GROUP = "Getting Started"
    public enum ImportSource: Equatable, Sendable { case kudzu, espanso, raycast, textExpander }
    public struct EspansoDirectoryPreview: Equatable, Sendable {
        public var path: URL; public var snippetCount: Int; public var groupCount: Int; public var groupedWarnings: [WarningGroup]; public var result: ImportResult
    }
    /// Groups warnings by message, most frequent first, then by message.
    public static func groupWarnings(_ warnings: [ImportWarning]) -> [WarningGroup] {
        // Group by message bytes (`BTreeMap<&str>`), then count descending, then `message.cmp`.
        var order: [Data] = [], buckets: [Data:[ImportWarning]] = [:]
        for warning in warnings {
            let key = Data(warning.message.utf8)
            if buckets[key] == nil { order.append(key) }
            buckets[key,default:[]].append(warning)
        }
        return order.map { key in let items = buckets[key] ?? []; return WarningGroup(message:items.first?.message ?? "",count:items.count,warnings:items) }.sorted { a, b in
            a.count != b.count ? a.count > b.count : a.message.utf8.lexicographicallyPrecedes(b.message.utf8)
        }
    }
    /// Imports `dir` without touching any library, for a confirmation screen.
    public static func previewEspansoDir(_ dir: URL) throws -> EspansoDirectoryPreview {
        let r = try Espanso.importDir(dir); return EspansoDirectoryPreview(path:dir,snippetCount:r.library.snippets.count,groupCount:r.library.groups.count,groupedWarnings:groupWarnings(r.warnings),result:r)
    }
    /// Previews every default Espanso config directory that exists under `home`.
    public static func detectEspansoDirs(home: URL) -> [EspansoDirectoryPreview] { Espanso.defaultConfigDirs(home:home).filter(Espanso.isDirectory).compactMap { try? previewEspansoDir($0) } }
    /// Same-ID snapshots use newer edits and max counters. Newly inserted parent cycles are cut.
    public static func mergeLibrary(existing: Library, incoming: Library) -> (library: Library, report: DedupeReport) {
        var merged = existing, parents: [UUID:UUID] = [:], known = Set(existing.groups.map(\.id)), inserted: [Int] = []
        for g in existing.groups { if let parent = g.parentID { parents[g.id] = parent } }
        for var g in incoming.groups where !known.contains(g.id) {
            known.insert(g.id); g.parentID = nil; inserted.append(merged.groups.count); merged.groups.append(g)
        }
        for i in inserted {
            let id = merged.groups[i].id
            guard let wanted = incoming.groups.first(where: { $0.id == id })?.parentID, known.contains(wanted) else { continue }
            var current: UUID? = wanted, seen: Set<UUID> = [], cycle = false
            while let n = current, seen.insert(n).inserted { if n == id { cycle = true; break }; current = parents[n] }
            if !cycle { parents[id] = wanted; merged.groups[i].parentID = wanted }
        }
        for var s in incoming.snippets {
            if s.groupID.map(known.contains) != true { s.groupID = nil }
            if let i = merged.snippets.firstIndex(where: { $0.id == s.id }) { if s.updatedAt > merged.snippets[i].updatedAt { merged.snippets[i] = s } }
            else { merged.snippets.append(s) }
        }
        for row in incoming.usage {
            if let i = merged.usage.firstIndex(where: { $0.snippetID == row.snippetID && $0.deviceID.isExactly(row.deviceID) }) {
                merged.usage[i].count = max(merged.usage[i].count,row.count); merged.usage[i].charsSaved = max(merged.usage[i].charsSaved,row.charsSaved)
                merged.usage[i].lastUsedAt = [merged.usage[i].lastUsedAt,row.lastUsedAt].compactMap { $0 }.max()
            } else { merged.usage.append(row) }
        }; return Dedupe.dedupe(merged)
    }
    /// Writes a `preImport` backup of `existing`, then merges `incoming` into it.
    public static func importLibrary(existing: Library, incoming: Library, warnings: [ImportWarning] = [], backups: BackupStore, now: Date, safetyLibrary: Library? = nil) throws -> (library: Library, summary: ImportSummary) {
        let backup = try backups.write(safetyLibrary ?? existing,reason:.preImport,now:now), result = mergeLibrary(existing:existing,incoming:incoming)
        return (result.library,ImportSummary(importedSnippets:incoming.snippets.count,importedGroups:incoming.groups.count,backup:backup,dedupeReport:result.report,warnings:warnings,groupedWarnings:groupWarnings(warnings)))
    }
    /// Merges any converted import (Espanso, Raycast, TextExpander, Kudzu JSON) into `existing`
    /// after a `preImport` backup.
    public static func importResult(existing: Library, result: ImportResult, backups: BackupStore, now: Date, safetyLibrary: Library? = nil) throws -> (library: Library, summary: ImportSummary) {
        try importLibrary(existing:existing,incoming:result.library,warnings:result.warnings,backups:backups,now:now,safetyLibrary:safetyLibrary)
    }
    /// Merges an Espanso import result into `existing` after a `preImport` backup.
    public static func importEspanso(existing: Library, result: ImportResult, backups: BackupStore, now: Date) throws -> (library: Library, summary: ImportSummary) { try importResult(existing:existing,result:result,backups:backups,now:now) }
    public static func importKudzuJSON(existing: Library, json: String, backups: BackupStore, now: Date) throws -> (library: Library, summary: ImportSummary) { try importLibrary(existing:existing,incoming:KudzuJSON.decodeLibrary(json),backups:backups,now:now) }
    /// Works out what `path` is from its extension and, for JSON, its shape: a Raycast export is
    /// an array of snippets, a Kudzu library an object. Folders are Espanso config directories.
    public static func detectImportSource(_ path: URL) throws -> ImportSource {
        if Espanso.isDirectory(path) { return .espanso }
        switch path.pathExtension.lowercased() {
        case "yml","yaml": return .espanso
        case "textexpander","csv","plist","xml": return .textExpander
        case "json":
            let text = try String(contentsOf:path,encoding:.utf8)
            return text.drop(while: { $0 == "\u{feff}" }).drop(while: \.isWhitespace).first == "[" ? .raycast : .kudzu
        default: throw InterchangeError.invalidSource("Kudzu can import .json (Kudzu or Raycast), .yml (Espanso), and .textexpander or .csv (TextExpander) files.")
        }
    }
    /// Reads and converts the import file at `path`, whichever app wrote it. Nothing is merged yet.
    public static func loadImport(_ path: URL) throws -> ImportResult {
        switch try detectImportSource(path) {
        case .kudzu: return ImportResult(library:try KudzuJSON.decodeLibrary(String(contentsOf:path,encoding:.utf8)))
        case .espanso: return try Espanso.isDirectory(path) ? Espanso.importDir(path) : Espanso.importFiles([path])
        case .raycast: return try Raycast.importFile(path)
        case .textExpander: return try TextExpander.importFile(path)
        }
    }
    /// Decode first, then take a safety copy before the caller replaces its current library.
    public static func restoreBackup(current: Library, path: URL, backups: BackupStore, now: Date, safetyLibrary: Library? = nil) throws -> (library: Library, safety: BackupInfo) {
        let restored = try backups.load(path), safety = try backups.write(safetyLibrary ?? current,reason:.preRestore,now:now); return (restored,safety)
    }
    /// Writes `library` as Espanso match files under `dir`; returns the written paths, sorted.
    public static func exportEspansoToDir(_ library: Library, dir: URL) throws -> [URL] {
        let files = Espanso.export(library); var written: [URL] = []
        for relative in files.keys.sorted(by: { $0.utf8.lexicographicallyPrecedes($1.utf8) }) {
            guard let text = files[relative] else { continue }; let path = dir.appendingPathComponent(relative)
            try FileManager.default.createDirectory(at:path.deletingLastPathComponent(),withIntermediateDirectories:true); try Data(text.utf8).write(to:path); written.append(path)
        }; return written
    }
    /// Adds the "Getting Started" group and starter snippets (`;kudzu`, `;date`, `;sig`) unless a
    /// snippet already uses `;kudzu`. Returns whether anything was added.
    @discardableResult public static func ensureGettingStartedSnippets(_ library: inout Library, now: Date) -> Bool {
        if library.snippets.contains(where: { $0.triggers.contains { $0.lowercased() == SAMPLE_TRIGGER } }) { return false }
        let groupID: UUID
        if let g = library.groups.first(where: { $0.parentID == nil && $0.name.lowercased() == GETTING_STARTED_GROUP.lowercased() }) { groupID = g.id }
        else { let sort = library.groups.filter { $0.parentID == nil }.map(\.sortIndex).max().map { $0+1 } ?? 0; let g = Group(name:GETTING_STARTED_GROUP,sortIndex:sort); library.groups.append(g); groupID = g.id }
        for (name,trigger,content) in [("Kudzu Test",SAMPLE_TRIGGER,SAMPLE_EXPANSION),("Today's Date",";date","{{date format=\"EEEE, d MMMM yyyy\"}}"),("Signature",";sig","Best regards,\nYour Name")] {
            library.snippets.append(Snippet(name:name,triggers:[trigger],content:content,groupID:groupID,createdAt:now,updatedAt:now))
        }; return true
    }
}
