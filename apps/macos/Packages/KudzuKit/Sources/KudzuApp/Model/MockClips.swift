import Foundation
import KudzuCore

// The clipboard history `lib/mock.ts` seeds (`seedClips`).

enum MockClips {
    /// Where the sample copies came from.
    static let sources: [String: ClipSource] = [
        "safari": ClipSource(bundleId: "com.apple.Safari", name: "Safari", path: "/Applications/Safari.app"),
        "code": ClipSource(bundleId: "com.microsoft.VSCode", name: "Visual Studio Code", path: "/Applications/Visual Studio Code.app"),
        "slack": ClipSource(bundleId: "com.tinyspeck.slackmacgap", name: "Slack", path: "/Applications/Slack.app"),
        "finder": ClipSource(bundleId: "com.apple.finder", name: "Finder", path: "/System/Library/CoreServices/Finder.app"),
        "figma": ClipSource(bundleId: "com.figma.Desktop", name: "Figma", path: "/Applications/Figma.app"),
        "terminal": ClipSource(bundleId: "com.apple.Terminal", name: "Terminal", path: "/System/Applications/Utilities/Terminal.app"),
        "mail": ClipSource(bundleId: "com.apple.mail", name: "Mail", path: "/System/Applications/Mail.app"),
        "notes": ClipSource(bundleId: "com.apple.Notes", name: "Notes", path: "/System/Applications/Notes.app"),
        "preview": ClipSource(bundleId: "com.apple.Preview", name: "Preview", path: "/System/Applications/Preview.app"),
    ]

    private static func source(_ key: String) -> ClipSource? { sources[key] }

    /// The image entries' artwork: `(width, height, hue, screenshot)` by entry id.
    typealias Artwork = [UUID: MockMedia.Artwork]

    /// The seeded history, newest first. `extra` adds that many generated copies (`?clips=5000`).
    static func seed(now: Date = Date(), extra: Int = 0) -> (entries: [ClipEntry], artwork: Artwork) {
        var artwork: Artwork = [:]
        func hours(_ h: Double) -> Date { now.addingTimeInterval(-h * 3600) }
        func mk(_ kind: ClipKind, _ at: Double, repeats: [Double] = [], _ extra: (inout ClipEntry) -> Void = { _ in }) -> ClipEntry {
            let times = (repeats + [at]).sorted(by: >).map(hours)
            var e = ClipEntry(
                kind: kind,
                types: ["public.utf8-plain-text"],
                hash: UUID().lowercased,
                firstCopiedAt: times[0],
                copiedAt: times[times.count - 1],
                copyCount: UInt32(times.count),
                timings: times
            )
            extra(&e)
            if let text = e.text {
                e.textLen = text.utf16.count
                e.lineCount = text.split(separator: "\n", omittingEmptySubsequences: false).count
            }
            if e.size == 0 { e.size = UInt64(e.text?.utf16.count ?? 0) }
            return e
        }
        func text(_ kind: ClipKind, _ at: Double, _ value: String, _ src: String, repeats: [Double] = [], _ extra: (inout ClipEntry) -> Void = { _ in }) -> ClipEntry {
            mk(kind, at, repeats: repeats) {
                $0.text = value
                $0.source = source(src)
                extra(&$0)
            }
        }
        func image(_ at: Double, _ w: Int, _ h: Int, _ hue: Double, _ shot: Bool, _ src: String = "preview", types: [String] = ["public.png", "public.tiff"]) -> ClipEntry {
            let e = mk(.image, at) {
                $0.image = ImageSize(width: UInt32(w), height: UInt32(h))
                $0.types = types
                $0.size = UInt64((Double(w * h) * 0.9).rounded())
                $0.source = source(src)
            }
            artwork[e.id] = MockMedia.Artwork(width: w, height: h, hue: hue, screenshot: shot)
            return e
        }

        var entries: [ClipEntry] = [
            text(.text, 0.01, "Can we move the design review to Thursday at 3? Happy to send a new invite.", "slack") {
                $0.rich = true
                $0.types = ["public.utf8-plain-text", "public.html"]
            },
            text(.link, 0.2, "https://github.com/stringsaeed/kudzu/pull/128", "safari", repeats: [5, 26]) { $0.types = ["public.utf8-plain-text", "public.url"] },
            text(.code, 0.6, """
            export function debounce<T extends (...args: never[]) => void>(fn: T, ms = 120) {
              let t: ReturnType<typeof setTimeout> | undefined
              return (...args: Parameters<T>) => {
                clearTimeout(t)
                t = setTimeout(() => fn(...args), ms)
              }
            }
            """, "code"),
            text(.text, 0.4, "kezwy4-dyxbeR-mosmac", "safari") { $0.secret = .password },
            text(.code, 0.9, "export OPENAI_API_KEY=sk-proj-" + "Abc123Def456Ghi789Jkl012Mno345\nexport REGION=eu-west-1", "terminal") { $0.secret = .apiKey },
            text(.color, 1.1, "#5B7CFA", "figma", repeats: [2, 30]) { $0.color = "#5b7cfa" },
            image(1.6, 2880, 1800, 230, true),
            text(.code, 2.3, "pnpm tauri build --config src-tauri/tauri.dev.conf.json --bundles app", "terminal", repeats: [3, 50, 51]),
            text(.file, 3.4, "/Users/saeed/Documents/Contracts/Kudzu — Services Agreement.pdf", "finder") {
                $0.files = ["/Users/saeed/Documents/Contracts/Kudzu — Services Agreement.pdf"]
                $0.types = ["public.file-url", "public.utf8-plain-text"]
                $0.size = 412
            },
            text(.text, 5.2, """
            Hi Maya,

            Thanks for the quick turnaround on the contract. I've signed and attached it — let me know if you need anything else from our side before kickoff on Monday.

            Best,
            Saeed
            """, "mail") {
                $0.rich = true
                $0.types = ["public.utf8-plain-text", "public.rtf", "public.html"]
                $0.pinned = true
            },
            text(.color, 7, "oklch(0.72 0.16 65)", "code") { $0.color = "oklch(0.72 0.16 65)" },
            text(.link, 9, "https://www.figma.com/design/x7Gq/Kudzu-Clipboard?node-id=12-2048", "figma"),
            image(20, 1600, 1067, 18, false, "safari", types: ["public.jpeg", "public.tiff"]),
            text(.text, 23, "221B Baker Street, London NW1 6XE", "notes", repeats: [80, 120]) { $0.pinned = true },
            text(.file, 26, "hero@2x.png\nhero-dark@2x.png\nicon.svg", "finder") {
                $0.files = ["/Users/saeed/Desktop/hero@2x.png", "/Users/saeed/Desktop/hero-dark@2x.png", "/Users/saeed/Desktop/icon.svg"]
                $0.types = ["public.file-url"]
                $0.size = 220
            },
            mk(.other, 29) {
                $0.types = ["com.figma.clipboard", "public.html", "public.png"]
                $0.size = 182_311
                $0.source = source("figma")
            },
            text(.code, 31, "SELECT id, email, created_at\nFROM users\nWHERE created_at > now() - interval '7 days'\nORDER BY created_at DESC;", "code"),
            text(.text, 47, "INV-2026-0412", "mail", repeats: [48, 60]),
            text(.link, 52, "mailto:hello@kudzu.app", "mail"),
            image(70, 1170, 2532, 280, true, "slack"),
            text(.text, 75, "Ship it! 🚀 Great work everyone — the new onboarding flow cut drop-off by 18%.", "slack"),
            text(.color, 98, "rgb(34, 197, 94)", "safari") { $0.color = "rgb(34, 197, 94)" },
            text(.code, 120, "{\n  \"name\": \"kudzu-desktop\",\n  \"private\": true,\n  \"packageManager\": \"pnpm@12.16.1\"\n}", "code"),
            text(.text, 140, "The quick brown fox jumps over the lazy dog.", "notes"),
            text(.link, 150, "https://developer.apple.com/documentation/appkit/nspasteboard", "safari"),
        ]
        // Lots of older plain copies, to exercise virtualization and day sections.
        let filler = ["Looks good to me 👍", "On my way", "+44 20 7946 0958", "Meeting notes — Q4 planning", "TODO: follow up with legal", "ref: KZ-482", "4111 1111 1111 1111"]
        let fillerSources = ["slack", "notes", "mail"]
        for i in 0..<60 {
            entries.append(text(.text, 26 + Double(i) * 2.3, filler[i % filler.count] + (i > 6 ? " (\(i))" : ""), fillerSources[i % 3]))
        }
        // `extra` fills the history to its cap, for performance checks.
        let words = "alpha beta gamma delta invoice meeting design review contract notes kudzu snippet paste copy deploy release".split(separator: " ").map(String.init)
        let extraSources = ["slack", "notes", "mail", "code"]
        for i in 0..<extra {
            let body = (0..<(6 + i % 40)).map { j in words[(i * 7 + j * 3) % words.count] }.joined(separator: " ") + " #\(i)"
            entries.append(text(i % 9 == 0 ? .code : .text, 0.5 + Double(i) / Double(max(1, extra)) * 160, body, extraSources[i % 4]))
        }
        entries.sort { $0.copiedAt > $1.copiedAt }
        return (entries, artwork)
    }
}
