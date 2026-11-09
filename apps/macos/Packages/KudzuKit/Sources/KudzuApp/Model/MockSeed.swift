import Foundation
import KudzuCore

// Sample data of `lib/mock.ts`: the library, settings and installed apps the preview starts with.

enum MockSeed {
    /// `iso()`: now, whole seconds (the mock strips milliseconds like the Rust writer).
    static func iso(_ date: Date = Date()) -> Date {
        Date(timeIntervalSince1970: date.timeIntervalSince1970.rounded(.down))
    }

    /// `daysAgo(n)`.
    static func daysAgo(_ n: Double, now: Date = Date()) -> Date {
        iso(now.addingTimeInterval(-n * 86_400))
    }

    // MARK: Library

    static func library(now: Date = Date()) -> Library {
        var random = SeededRandom(seed: 0x4B55_445A)
        let work = Group(name: "Work", sortIndex: 0)
        let email = Group(name: "Email", parentID: work.id, sortIndex: 0)
        let dev = Group(name: "Developer", sortIndex: 1)
        let personal = Group(name: "Personal", sortIndex: 2)
        func mk(_ name: String, _ triggers: [String], _ content: String, _ extra: (inout Snippet) -> Void = { _ in }) -> Snippet {
            var s = Snippet(
                name: name,
                triggers: triggers,
                content: content,
                createdAt: daysAgo(40, now: now),
                updatedAt: daysAgo(Double(random.next(below: 20)), now: now)
            )
            extra(&s)
            return s
        }
        let sig = mk("Email signature", [";sig"], "Best regards,\nSaeed\n{{date format=\"MMMM d, yyyy\"}}") {
            $0.groupID = email.id
            $0.isFavorite = true
        }
        let snippets: [Snippet] = [
            sig,
            mk("Meeting follow-up", [";fu", ";followup"], "Hi {{field name=\"Name\"}},\n\nThanks for the time today. As discussed, {{cursor}}\n\n{{snippet id=\"\(sig.id.lowercased)\"}}") { $0.groupID = email.id },
            mk("Out of office", [";ooo"], "I'm out of the office until {{date format=\"EEEE, MMMM d\" offset=\"+1w\"}} with limited access to email.") { $0.groupID = email.id },
            mk("Today", [";date"], "{{date}}") { $0.isFavorite = true },
            mk("Current time", [";now"], "{{time format=\"h:mm a\"}}"),
            mk("Shrug", [";shrug"], "¯\\_(ツ)_/¯") { $0.isFavorite = true },
            mk("Paste as quote", [";quote"], "> {{clipboard trim=true}}"),
            mk("Bug report", [";bug"], "**Environment:** {{choice name=\"Platform\" options=\"macOS|Windows|Linux\"}}\n**Steps:**\n1. {{cursor}}\n**Expected:**\n**Actual:**") { $0.groupID = dev.id },
            mk("Console log", [";cl"], "console.log({{clipboard}})") {
                $0.groupID = dev.id
                $0.fireMode = .wholeWord
                $0.caseMode = .exact
            },
            mk("Git commit co-author", [";coa"], "Co-authored-by: {{field name=\"Author\"}} <{{field name=\"Email\"}}>") { $0.groupID = dev.id },
            mk("Home address", [";addr"], "221B Baker Street\nLondon NW1 6XE\nUnited Kingdom") { $0.groupID = personal.id },
            mk("Phone number", [";ph"], "+44 20 7946 0958") { $0.groupID = personal.id },
            mk("Thanks", [";ty"], "Thank you so much — really appreciate it!"),
            mk("Calendar link", [";cal"], "You can grab a time that works here: https://cal.com/saeed/30min") { $0.groupID = work.id },
            mk("Lorem ipsum", [";lorem"], "Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua.") { $0.isEnabled = false },
        ]
        let usage = snippets.prefix(12).enumerated().map { i, s in
            let weight = 180 / (Double(i) + 1.4)
            return UsageRecord(
                snippetID: s.id,
                deviceID: "mock",
                count: max(1, Int(weight.rounded())),
                charsSaved: Int((Double(s.content.utf16.count - (s.triggers.first?.utf16.count ?? 0)) * weight).rounded()),
                lastUsedAt: daysAgo(Double(i) * 1.7, now: now)
            )
        }
        return Library(groups: [work, email, dev, personal], snippets: snippets, usage: usage)
    }

    // MARK: Settings

    /// `defaultSettings` of the mock (differs from `Settings.default`: onboarding done, launch at
    /// login on, sample denied apps and app insertions).
    static let settings: Settings = {
        var s = Settings.default
        s.deniedBundleIDs = ["com.1password.1password", "com.apple.keychainaccess"]
        s.appInsertions = ["com.apple.Terminal": .paste, "com.googlecode.iterm2": .paste]
        s.launchAtLogin = true
        s.onboardingCompleted = true
        s.lastRunVersion = "0.1.0"
        s.clipboardIgnoredApps = ["com.1password.1password", "com.apple.keychainaccess", "com.apple.Passwords"]
        return s
    }()

    // MARK: Apps

    static let apps: [AppInfo] = [
        ("com.apple.mail", "Mail"),
        ("com.apple.Safari", "Safari"),
        ("com.google.Chrome", "Google Chrome"),
        ("com.tinyspeck.slackmacgap", "Slack"),
        ("com.microsoft.VSCode", "Visual Studio Code"),
        ("com.apple.Terminal", "Terminal"),
        ("com.googlecode.iterm2", "iTerm"),
        ("notion.id", "Notion"),
        ("com.linear", "Linear"),
        ("com.1password.1password", "1Password"),
        ("com.apple.keychainaccess", "Keychain Access"),
        ("com.apple.Notes", "Notes"),
    ].map { AppInfo(bundleId: $0.0, name: $0.1, path: "/Applications/\($0.1).app") }

    // MARK: Backups

    static func backups(now: Date = Date()) -> [BackupInfo] {
        [
            BackupInfo(path: "/backups/a.json", filename: "kudzu-2026-11-03-daily.json", createdAt: daysAgo(0, now: now), reason: .daily),
            BackupInfo(path: "/backups/b.json", filename: "kudzu-2026-11-02-daily.json", createdAt: daysAgo(1, now: now), reason: .daily),
            BackupInfo(path: "/backups/c.json", filename: "kudzu-2026-11-01-preImport.json", createdAt: daysAgo(2, now: now), reason: .preImport),
        ]
    }

    /// Release notes the `?update` preview shows.
    static let updateNotes = "## [0.2.0](https://example.com) (2026-11-04)\n\n### Features\n\n* **palette:** faster search tray ([4143658](https://example.com))\n* new keycap shortcuts ([c818cef](https://example.com))\n\n### Fixes\n\n* keep the titlebar transparent"
}

/// A small deterministic generator, so previews and tests see the same sample data every run
/// (the TypeScript mock used `Math.random()` for `updatedAt`).
struct SeededRandom {
    private var state: UInt64
    init(seed: UInt64) { state = seed }

    mutating func next(below bound: Int) -> Int {
        state = state &* 6_364_136_223_846_793_005 &+ 1_442_695_040_888_963_407
        return Int((state >> 33) % UInt64(bound))
    }
}
