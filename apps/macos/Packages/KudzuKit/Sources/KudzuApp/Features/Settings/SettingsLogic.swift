// The decisions behind the Settings pages, as data: section names and keyboard order, the
// clipboard retention picker, the iCloud and update rows, the AI agents descriptions and the
// backup labels. Port of the inline logic of `components/app/main/settings.tsx`; the views only
// draw what these return, so the tests can pin every string and state.

import Foundation
import KudzuCore

// MARK: Sections

extension SettingsSection {
    /// The label in the section list and the page header (`SETTINGS_SECTIONS`).
    public var label: String {
        switch self {
        case .general: "General"
        case .expansion: "Expansion"
        case .clipboard: "Clipboard"
        case .agents: "AI agents"
        case .apps: "Apps"
        case .data: "Backups & data"
        case .shortcuts: "Shortcuts"
        case .about: "About"
        }
    }
}

enum SettingsNav {
    /// The order of the section list. The web list stops at seven (`shortcuts` is a valid route
    /// there but had no page); the native app gives the cheat sheet its own page before About.
    static let sections: [SettingsSection] = [.general, .expansion, .clipboard, .agents, .apps, .data, .shortcuts, .about]

    /// The page header: the section label alone, as on the web (`<h2>{label}</h2>`).
    static func header(for section: SettingsSection) -> String { section.label }

    /// The section ↑/↓ moves to from `section`, wrapping at both ends.
    static func neighbour(of section: SettingsSection, step: Int) -> SettingsSection {
        let i = sections.firstIndex(of: section) ?? 0
        let n = sections.count
        return sections[((i + step) % n + n) % n]
    }
}

// MARK: Clipboard retention

/// What the "Keep history" picker shows: one of the presets (`0` = Forever) or "Custom…".
enum RetentionChoice: Hashable {
    case preset(UInt32)
    case custom

    /// `RETENTION_OPTIONS` (days, label), Forever last.
    static let presets: [(days: UInt32, label: String)] = Clipboard.RETENTION_OPTIONS.map { (days: UInt32($0.0), label: $0.1) }

    static func isPreset(_ days: UInt32) -> Bool { presets.contains { $0.days == days } }

    /// The picker's value: custom while the user is typing a custom number or when the stored
    /// value isn't a preset (`custom ? "custom" : String(days)`).
    static func selection(days: UInt32, customMode: Bool) -> RetentionChoice {
        customMode || !isPreset(days) ? .custom : .preset(days)
    }

    /// The draft a custom field starts from (`String(days || 7)`).
    static func draft(for days: UInt32) -> String { String(days == 0 ? 7 : days) }

    /// `commitCustom`: a whole number of days from 1 to 3650 (rounded like `Math.round`), or `nil`
    /// to reset the draft.
    static func parseCustom(_ draft: String) -> UInt32? {
        let text = draft.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !text.isEmpty, let value = Double(text), value.isFinite else { return nil }
        let n = (value + 0.5).rounded(.down)
        guard n >= 1, n <= 3650 else { return nil }
        return UInt32(n)
    }

    /// The row description under "Keep history".
    static func description(days: UInt32) -> String {
        guard days > 0 else { return "Items stay until you delete them (up to 5,000). Pinned items always stay." }
        let label = Clipboard.retentionLabel(Int(days))
        let after = label.hasPrefix("for ") ? "after " + label.dropFirst(4) : label
        return "Unpinned items are deleted \(after). Pinned items stay until you unpin them."
    }
}

enum ClipboardStorageCopy {
    /// "1,284 items · 12.4 MB", the count in the user's locale (`toLocaleString()`).
    static func title(count: Int, diskUsage: UInt64, locale: Locale = .current) -> String {
        let items = "\(count.formatted(.number.locale(locale))) item\(count == 1 ? "" : "s")"
        return diskUsage > 0 ? "\(items) · \(Clipboard.formatBytes(Int(clamping: diskUsage)))" : items
    }

    static func description(pinned: Int) -> String {
        pinned > 0
            ? "\(pinned) pinned. Up to 5,000 unpinned items are kept; the oldest go first."
            : "Up to 5,000 unpinned items are kept; the oldest go first."
    }

    /// The clear-history alert's message.
    static func clearMessage(pinned: Int) -> String {
        let what = pinned > 0 ? "Deletes everything except your \(pinned) pinned item\(pinned == 1 ? "" : "s")." : "Deletes everything Kudzu remembered."
        return "\(what) This can’t be undone."
    }

    static func cleared(_ n: Int) -> String { n == 1 ? "Deleted 1 item" : "Deleted \(n) items" }
}

// MARK: iCloud

/// The "Sync with iCloud" row's status line and trailing action.
struct SyncLine: Equatable {
    enum Tone: Equatable { case plain, muted, warning, destructive }
    enum Action: Equatable {
        /// "Update" → Settings › About.
        case update
        /// "Sync now".
        case syncNow
    }

    var text: String
    var tone: Tone
    /// A spinner before the text (`starting`, `syncing`).
    var busy = false
    /// The success check (`upToDate`).
    var check = false
    var action: Action?

    static func of(_ status: SyncStatus, now: Date = Date()) -> SyncLine {
        switch status {
        case .off: SyncLine(text: "Off. Your snippets stay on this Mac.", tone: .plain)
        case let .unavailable(reason): SyncLine(text: reason, tone: .muted)
        case let .noAccount(reason): SyncLine(text: reason, tone: .warning)
        case .starting, .syncing: SyncLine(text: "Syncing…", tone: .plain, busy: true)
        case let .upToDate(lastSync):
            SyncLine(text: "Up to date · synced \(relative(lastSync, now: now).lowercased())", tone: .plain, check: true, action: .syncNow)
        case .needsUpdate:
            SyncLine(text: "Another Mac uses a newer Kudzu. Update this one to keep syncing; nothing here is lost meanwhile.", tone: .warning, action: .update)
        case let .error(message): SyncLine(text: message, tone: .destructive, action: .syncNow)
        }
    }
}

/// `relativeTime(Date.parse(s))` for the RFC 3339 strings Rust sends.
func relative(_ timestamp: String, now: Date = Date()) -> String {
    guard let date = KudzuDate.parse(timestamp) else { return "Never" }
    return Stats.relativeTime(date.timeIntervalSince1970 * 1000, now: now)
}

// MARK: Updates

/// What `UpdateRow` shows for an `UpdateState`.
enum UpdateRowContent: Equatable {
    /// "Kudzu x is ready" with release notes and Restart (or the installing spinner).
    case ready(version: String, notes: [ReleaseNotes.Section], restarting: Bool)
    /// "Downloading Kudzu x" with a progress bar; `percent` is nil when the size is unknown
    /// (the bar then sits at 30 %).
    case downloading(version: String, percent: Int?)
    /// The plain "Software update" row with Check now.
    case row(description: String, tone: SyncLine.Tone, button: String, checking: Bool)

    static func of(_ state: UpdateState, now: Date = Date()) -> UpdateRowContent {
        switch state {
        case let .ready(version, notes, _): .ready(version: version, notes: ReleaseNotes.parse(notes), restarting: false)
        case let .restarting(version): .ready(version: version, notes: [], restarting: true)
        case let .downloading(version, downloaded, total):
            .downloading(version: version, percent: total.flatMap { $0 > 0 ? min(100, Int((Double(downloaded) / Double($0) * 100).rounded())) : nil })
        case .checking: .row(description: "Checking…", tone: .plain, button: "Check now", checking: true)
        case let .upToDate(checkedAt):
            .row(description: "You’re on the latest version · checked \(relative(checkedAt, now: now).lowercased())", tone: .plain, button: "Check now", checking: false)
        case let .error(message, needsMove):
            // A location problem ("Move Kudzu to your Applications folder…") is advice, not a failure.
            needsMove
                ? .row(description: message, tone: .warning, button: "Check now", checking: false)
                : .row(description: "Couldn’t check: \(message)", tone: .destructive, button: "Try again", checking: false)
        case .idle: .row(description: "New versions download in the background and install when you quit.", tone: .plain, button: "Check now", checking: false)
        }
    }

    /// The download line: "In the background · 42%. You’ll be asked before anything restarts."
    static func downloadingDetail(percent: Int?) -> String {
        "In the background\(percent.map { " · \($0)%" } ?? ""). You’ll be asked before anything restarts."
    }
}

// MARK: AI agents

enum AgentCopy {
    /// "Allow AI agents" description.
    static func access(on: Bool, running: Bool?) -> String {
        guard on else { return "Off. Connected agents are told Kudzu is unavailable." }
        return running == true ? "Agents you’ve connected can use Kudzu while it’s running." : "Couldn’t start. Turn it off and on again, or restart Kudzu."
    }

    /// The install button is shown while setup loads but stays disabled until it arrives, and
    /// while an install runs (`disabled={installing || !setup}`).
    static func skillButtonDisabled(setup: AgentSetup?, installing: Bool) -> Bool {
        installing || setup == nil
    }

    static func clipboard(enabled: Bool) -> String {
        enabled ? "Agents can search what you’ve copied, such as a link or an error message." : "Clipboard history is turned off."
    }

    /// The skill row's description and button for a setup (nil while it loads).
    static func skill(_ setup: AgentSetup?) -> (description: String, button: String?) {
        switch setup?.skill {
        case .current: ("Installed in \(Clipboard.tildify(setup?.skillPath ?? ""))", nil)
        case .outdated: ("This version of Kudzu has a newer skill.", "Update skill")
        default: ("Teaches Claude when to use your snippets and how to write Kudzu templates.", "Install skill")
        }
    }
}

// MARK: Backups & data

enum DataCopy {
    static func reason(_ reason: BackupReason) -> String {
        switch reason {
        case .daily: "Daily"
        case .preImport: "Before import"
        case .preRestore: "Before restore"
        case .manual: "Manual"
        case .preUpdate: "Before update"
        }
    }

    /// "Nov 3, 2026 at 9:41 AM" (`dateStyle: medium, timeStyle: short`).
    static func backupDate(_ date: Date, locale: Locale = .current) -> String {
        let f = DateFormatter()
        f.locale = locale
        f.dateStyle = .medium
        f.timeStyle = .short
        return f.string(from: date)
    }

    /// "Daily · Yesterday".
    static func backupDetail(_ b: BackupInfo, now: Date = Date()) -> String {
        "\(reason(b.reason)) · \(Stats.relativeTime(b.createdAt.timeIntervalSince1970 * 1000, now: now))"
    }

    static func espansoFound(_ d: EspansoDetection) -> String {
        "\(d.snippetCount) snippets in \(d.groupCount) files · \(d.path)"
    }
}

// MARK: Release notes (`updates.ts` `parseNotes`)

enum ReleaseNotes {
    static let releasesURL = URL(string: "https://github.com/Stringsaeed/kudzu/releases")!

    struct Section: Equatable, Sendable {
        var heading: String?
        var items: [String]
    }

    /// Reduces `release-please` Markdown notes to readable sections (`updates.ts` `parseNotes`).
    static func parse(_ md: String?) -> [Section] {
        guard let md, !md.isEmpty else { return [] }
        var sections: [Section] = []
        var cur = Section(heading: nil, items: [])

        for raw in md.components(separatedBy: "\n") {
            let line = raw.trimmingCharacters(in: .whitespacesAndNewlines)
            if line.isEmpty { continue }

            if let heading = matchHeading(line) {
                if isVersionTitleHeading(heading) { continue }
                if !cur.items.isEmpty || cur.heading != nil {
                    sections.append(cur)
                }
                cur = Section(heading: heading, items: [])
                continue
            }

            let cleaned = cleanNoteItem(line)
            if !cleaned.isEmpty {
                cur.items.append(cleaned)
            }
        }

        if !cur.items.isEmpty || cur.heading != nil {
            sections.append(cur)
        }
        return sections
    }

    private static func matchHeading(_ line: String) -> String? {
        var hashes = 0
        var idx = line.startIndex
        while idx < line.endIndex, line[idx] == "#" {
            hashes += 1
            idx = line.index(after: idx)
        }
        guard (1...6).contains(hashes), idx < line.endIndex, line[idx].isWhitespace else {
            return nil
        }
        while idx < line.endIndex, line[idx].isWhitespace {
            idx = line.index(after: idx)
        }
        return String(line[idx...])
    }

    private static func isVersionTitleHeading(_ heading: String) -> Bool {
        heading.range(of: #"^\[?\d+\.\d+\.\d+"#, options: .regularExpression) != nil
    }

    private static func cleanNoteItem(_ line: String) -> String {
        var s = line.replacingOccurrences(of: #"^[*-]\s+"#, with: "", options: .regularExpression)
        s = s.replacingOccurrences(of: #"\s*\(\[[0-9a-f]{7,}\]\([^)]*\)\)"#, with: "", options: .regularExpression)
        s = s.replacingOccurrences(of: #"\[([^\]]+)\]\([^)]*\)"#, with: "$1", options: .regularExpression)
        s = s.replacingOccurrences(of: #"\*\*([^*]+)\*\*"#, with: "$1", options: .regularExpression)
        return s.trimmingCharacters(in: .whitespacesAndNewlines)
    }
}

// MARK: App insertion overrides (`settings.tsx` `AppsSettings`)

enum AppInsertionsLogic {
    struct Entry: Identifiable, Equatable, Sendable {
        var bundleId: String
        var method: InsertionMethod
        var id: String { bundleId }
    }

    /// Entries sorted alphabetically by resolved display name.
    static func sortedEntries(
        _ map: [String: InsertionMethod],
        nameForBundleId: (String) -> String
    ) -> [Entry] {
        map
            .map { Entry(bundleId: $0.key, method: $0.value) }
            .sorted {
                nameForBundleId($0.bundleId)
                    .localizedCaseInsensitiveCompare(nameForBundleId($1.bundleId)) == .orderedAscending
            }
    }

    /// Adds new bundle IDs with `defaultMethod` without overwriting existing overrides.
    static func adding(
        _ bundleIds: [String],
        to map: [String: InsertionMethod],
        defaultMethod: InsertionMethod = .paste
    ) -> [String: InsertionMethod] {
        var next = map
        for id in bundleIds where next[id] == nil {
            next[id] = defaultMethod
        }
        return next
    }

    /// Sets the insertion method for `bundleId`.
    static func setting(
        _ bundleId: String,
        method: InsertionMethod,
        in map: [String: InsertionMethod]
    ) -> [String: InsertionMethod] {
        var next = map
        next[bundleId] = method
        return next
    }

    /// Removes `bundleId` from the overrides map.
    static func removing(
        _ bundleId: String,
        from map: [String: InsertionMethod]
    ) -> [String: InsertionMethod] {
        var next = map
        next.removeValue(forKey: bundleId)
        return next
    }
}

// MARK: Brand & Privacy copy (`brand.json` + `settings.tsx` About)

enum BrandCopy {
    static let name = "Kudzu"
    /// `design/brand.json`.
    static let tagline = "For things worth repeating"
    static let description = "Clipboard history and text shortcuts for your Mac."
    /// The About privacy card (`settings.tsx` `About`).
    static let privacyBody = "No accounts, no analytics, no Kudzu servers. Your snippets live on this Mac and, if you turn on iCloud sync, in your private iCloud database, which Kudzu’s developer can’t see. Apart from that, Kudzu only goes online to check for updates, and that sends nothing about you."
}

// MARK: Shortcuts catalog (`shortcuts.tsx`)

enum ShortcutsCatalog {
    enum Binding: Equatable, Sendable {
        /// A single shortcut chord (`Shortcut("mod+f")`).
        case key(String)
        /// Multiple alternative chords separated by `/` (`up` / `down`, `mod+b` / `mod+i` / `mod+u`).
        case alternatives([String])
        /// A range of chords separated by `…` (`mod+1` … `mod+6`).
        case range(String, String)
        /// Modifier + click (`mod` + click, `shift` + click).
        case modifierClicks([String])
        /// Dynamic global accelerator from `KudzuCore.Settings` (`Keys.acceleratorLabels(raw)`).
        case globalAccelerator(String)
    }

    struct Row: Equatable, Sendable, Identifiable {
        var label: String
        var binding: Binding
        var id: String { label }
    }

    struct Section: Equatable, Sendable, Identifiable {
        var title: String
        var rows: [Row]
        var id: String { title }
    }

    /// `shortcuts.tsx` `SECTIONS`. Global accelerators are the dialog header, not rows.
    static func sections(settings: KudzuCore.Settings) -> [Section] {
        _ = settings
        return [
            Section(title: "Everywhere", rows: [
                Row(label: "Search & commands", binding: .key("mod+k")),
                Row(label: "New snippet", binding: .key("mod+n")),
                Row(label: "New group", binding: .key("mod+shift+n")),
                Row(label: "Filter snippets", binding: .key("mod+f")),
                Row(label: "Pause / resume expanding", binding: .key("mod+shift+p")),
                Row(label: "Settings", binding: .key("mod+,")),
                Row(label: "This cheat sheet", binding: .key("mod+/")),
            ]),
            Section(title: "Navigate", rows: [
                Row(label: "All snippets", binding: .key("mod+1")),
                Row(label: "Favorites", binding: .key("mod+2")),
                Row(label: "Recently used", binding: .key("mod+3")),
                Row(label: "Insights", binding: .key("mod+4")),
                Row(label: "Clipboard history", binding: .key("mod+5")),
                Row(label: "Focus the list", binding: .key("mod+l")),
                Row(label: "Edit content", binding: .key("mod+e")),
                Row(label: "Back to the list", binding: .key("escape")),
            ]),
            Section(title: "Snippet list", rows: [
                Row(label: "Previous (or K)", binding: .key("up")),
                Row(label: "Next (or J)", binding: .key("down")),
                Row(label: "Extend selection", binding: .key("shift+down")),
                Row(label: "Select all", binding: .key("mod+a")),
                Row(label: "Open in editor", binding: .key("enter")),
                Row(label: "Filter", binding: .key("/")),
                Row(label: "Delete (undoable)", binding: .key("mod+backspace")),
            ]),
            Section(title: "Clipboard history", rows: [
                Row(label: "Copy", binding: .key("enter")),
                Row(label: "Copy as plain text", binding: .key("shift+enter")),
                Row(label: "Large preview", binding: .key("space")),
                Row(label: "Pin / unpin", binding: .key("mod+p")),
                Row(label: "Save as snippet", binding: .key("mod+s")),
                Row(label: "Open link or file", binding: .key("mod+o")),
                Row(label: "Delete (undoable)", binding: .key("mod+backspace")),
            ]),
            Section(title: "Editor", rows: [
                Row(label: "Insert a variable", binding: .key("/")),
                Row(label: "Duplicate snippet", binding: .key("mod+d")),
                Row(label: "Toggle favorite", binding: .key("mod+shift+f")),
                Row(label: "Turn on / off", binding: .key("mod+shift+e")),
                Row(label: "Undo", binding: .key("mod+z")),
                Row(label: "Save now (auto-saves anyway)", binding: .key("mod+s")),
            ]),
        ]
    }
}
