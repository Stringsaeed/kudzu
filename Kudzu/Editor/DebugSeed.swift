#if DEBUG
import KudzuCore
import SwiftUI

/// DEBUG-only sample data for trying the window. Compiled out of release builds.
enum DebugSeed {
    private static let idPrefix = "DEB06000-0000-4000-8000-"

    /// Demo records get recognizable IDs so `removeDemoData` never touches real snippets.
    static func id(_ n: Int) -> UUID {
        UUID(uuidString: idPrefix + String(format: "%012d", n))!
    }

    static func isDemo(_ id: UUID) -> Bool { id.uuidString.hasPrefix(idPrefix) }

    /// Seeds the demo library when launched with `-KudzuSeedDemo` and the library is empty.
    @MainActor
    static func seedIfRequested(library: LibraryModel) {
        let arguments = ProcessInfo.processInfo.arguments
        guard library.snippets.isEmpty, library.groups.isEmpty else { return }
        if arguments.contains("-KudzuSeedDemo") { demo(library: library) }
        if arguments.contains("-KudzuSeedBulk") { bulk(3000, library: library) }
    }

    /// A handful of groups and snippets covering templates, forms, nesting, usage and a conflict.
    @MainActor
    static func demo(library: LibraryModel) {
        let store = library.store
        let work = GroupRecord(id: id(1), name: "Work", sortIndex: 0)
        let email = GroupRecord(id: id(2), name: "Email", parentID: work.id, sortIndex: 0)
        let personal = GroupRecord(id: id(3), name: "Personal", sortIndex: 1)
        let signature = SnippetRecord(id: id(10),
            name: "Signature", triggers: [";sig"], content: "Best regards,\nSaeed{{cursor}}", groupID: email.id, isFavorite: true)
        let short = SnippetRecord(id: id(11), name: "Short sig", triggers: [";s"], content: "— S", groupID: email.id)
        let today = SnippetRecord(id: id(12),
            name: "Today", triggers: [";date", ";today"], content: "{{date format=\"EEEE, d MMMM yyyy\"}}", groupID: work.id)
        let meeting = SnippetRecord(id: id(13),
            name: "Meeting notes",
            triggers: [";mtg"],
            content: "# {{field name=\"title\" default=\"Weekly sync\"}}\nDate: {{date}}\nWith: {{choice name=\"who\" options=\"Team|Client|Board\"}}\n\n{{cursor}}",
            groupID: work.id, fireMode: .wholeWord)
        let addr = SnippetRecord(id: id(14),
            name: "Address", triggers: [";addr"], content: "1 Infinite Loop\nCupertino, CA", groupID: personal.id,
            rule: AppRule(mode: .allow, bundleIDs: ["com.apple.mail", "com.apple.Notes"]))
        let clip = SnippetRecord(id: id(15), name: "Quote clipboard", triggers: [";quote"], content: "> {{clipboard trim=true}}", insertion: .paste)
        let nested = SnippetRecord(id: id(16), name: "Thanks", triggers: [";ty"], content: "Thank you!\n\n{{snippet id=\"\(signature.id.uuidString)\"}}", groupID: email.id)
        let broken = SnippetRecord(id: id(17), name: "Broken tag", triggers: [";bad"], content: "Hello {{nope}} and {{date format=}}", isEnabled: false)
        do {
            for group in [work, email, personal] { try store.upsert(group) }
            for snippet in [signature, short, today, meeting, addr, clip, nested, broken] { try store.upsert(snippet) }
            for (snippet, uses) in [(signature, 42), (today, 17), (meeting, 9), (nested, 3), (clip, 1)] {
                for _ in 0..<uses { try store.recordUsage(snippetID: snippet.id, charsSaved: snippet.content.count) }
            }
        } catch {
            library.errorMessage = "Seeding failed: \(error.localizedDescription)"
        }
        library.reload()
    }

    /// Deletes everything `demo` and `bulk` created, leaving real data alone.
    @MainActor
    static func removeDemoData(library: LibraryModel) {
        let store = library.store
        do {
            for snippet in library.snippets where isDemo(snippet.id) { try store.deleteSnippet(id: snippet.id) }
            let doomed = library.groups.filter { isDemo($0.id) }
                .sorted { library.tree.ancestorIDs(of: $0.id).count > library.tree.ancestorIDs(of: $1.id).count }
            for group in doomed { try store.deleteGroup(id: group.id, moveChildrenTo: library.tree.parent(of: group.id)) }
        } catch {
            library.errorMessage = "Cleanup failed: \(error.localizedDescription)"
        }
        library.reload()
    }

    /// Many snippets across groups, for checking that scrolling, search and sorting stay fast.
    @MainActor
    static func bulk(_ count: Int, library: LibraryModel) {
        let store = library.store
        do {
            let groups = (0..<20).map { GroupRecord(id: id(100 + $0), name: "Group \($0 + 1)", sortIndex: $0) }
            for group in groups { try store.upsert(group) }
            for i in 0..<count {
                let snippet = SnippetRecord(
                    id: id(1000 + i),
                    name: "Bulk snippet \(i)",
                    triggers: [";b\(i)"],
                    content: "Body of bulk snippet number \(i). Café résumé naïve.",
                    groupID: groups[i % groups.count].id)
                try store.upsert(snippet)
                if i % 7 == 0 { try store.recordUsage(snippetID: snippet.id, charsSaved: 40) }
            }
        } catch {
            library.errorMessage = "Seeding failed: \(error.localizedDescription)"
        }
        library.reload()
    }
}

/// Toolbar menu for loading sample data. DEBUG builds only.
struct DebugMenu: View {
    @Environment(LibraryModel.self) private var library

    var body: some View {
        Menu {
            Button("Seed Demo Data") { DebugSeed.demo(library: library) }
            Button("Seed 3,000 Snippets") { DebugSeed.bulk(3000, library: library) }
            Divider()
            Button("Remove Demo Data") { DebugSeed.removeDemoData(library: library) }
        } label: {
            Label("Debug", systemImage: "ladybug")
        }
        .help("DEBUG build only: load sample data")
    }
}
#endif
