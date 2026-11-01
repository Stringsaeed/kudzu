@testable import KudzuUI
import SwiftUI
import Testing

/// The one window with realistic content: sidebar items, a list of snippet rows and a detail pane with
/// fields and disclosures. Controls owned by other components are placeholders.
struct SampleAppShell: View {
    var selection = ":date"
    var body: some View {
        KzAppShell(mode: .embedded) {
            KzSidebarSection {
                KzSidebarItem("All snippets", icon: .stack, count: 18, isActive: true) {}
                KzSidebarItem("Favorites", icon: .star, count: 3) {}
                KzSidebarItem("Recently used", icon: .clock) {}
                KzSidebarItem("Stats", icon: .chart) {}
            }
            KzSidebarSection("Groups", action: { PlaceholderIconButton(icon: .plus, size: KzSize.controlSm) }) {
                KzSidebarItem("base", icon: .folder, count: 15) {}
                KzSidebarItem("Getting started", icon: .folder, count: 3) {}
            }
        } sidebarHeader: {
            PlaceholderIconButton(icon: .sidebar, size: KzSize.controlSm)
        } sidebarFooter: {
            KzSidebarItem("Settings", icon: .sliders, shortcut: "⌘,") {}
        } list: {
            KzListHeader("All snippets", subtitle: "18 snippets") {
                PlaceholderIconButton(icon: .sort)
                PlaceholderIconButton(icon: .compose)
            }
            PlaceholderSearch().padding(.horizontal, KzSpace.s3).padding(.bottom, KzSpace.s2)
            VStack(spacing: 1) {
                KzSnippetRow(name: ":an", triggers: [":an"], preview: "Muhammed Saeed", group: "base", uses: 1, isFavorite: true, isSelected: selection == ":an")
                KzSnippetRow(name: ":date", triggers: [":date"], preview: "2 Nov 2026", group: "base", isFavorite: true, isSelected: selection == ":date")
                KzSnippetRow(name: ":espanso", triggers: [":espanso"], preview: "Hi there!", group: "base", isFavorite: true)
                KzSnippetRow(name: ":fn", triggers: [":fn"], preview: "Muhammed Ahmed Saeed", group: "base")
                KzSnippetRow(name: ":gh", triggers: [":gh"], preview: "github.com/Stringsaeed", group: "base")
                KzSnippetRow(name: "Kudzu test", triggers: [";kudzu", ";kz"], preview: "Kudzu is working. Expanded at 00:57 in Notes.", group: "Getting started")
                KzSnippetRow(name: "Signature", triggers: [";sig"], preview: "Best, Muhammed", group: "Getting started")
            }
            .padding(.horizontal, KzSpace.s2)
            Spacer(minLength: 0)
        } detail: {
            KzPaneHeader {
                HStack(spacing: 6) {
                    KzIcon(.folder, size: 12)
                    Text("base").kzTextStyle(.caption)
                }
                .foregroundStyle(KzColor.inkMuted)
                Spacer(minLength: 0)
                HStack(spacing: 2) {
                    PlaceholderIconButton(icon: .starFill)
                    PlaceholderIconButton(icon: .bug)
                    PlaceholderIconButton(icon: .more)
                }
                .padding(3)
                .kzGlass(.strong, in: RoundedRectangle(cornerRadius: KzRadius.lg), shadow: .glass)
            }
            VStack(alignment: .leading, spacing: KzSpace.s5) {
                Text(":date")
                    .kzTextStyle(.titleLg)
                    .foregroundStyle(KzColor.ink)
                KzField("Triggers", aside: "Space or return to add") { PlaceholderField(text: ":date") }
                KzField("Content") {
                    KzEditorFrame(minHeight: 70, insert: { PlaceholderButton(title: "+ Insert  ⌄", small: true) }) {
                        KzEditorTextView([.tag("{date:%d %b %Y}")], isEditable: false)
                    }
                }
                KzField("Preview") {
                    Text("2 Nov 2026")
                        .kzTextStyle(.content)
                        .foregroundStyle(KzColor.ink)
                        .padding(.horizontal, 14).padding(.vertical, KzSpace.s3)
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .background(KzColor.surfaceSunken, in: RoundedRectangle(cornerRadius: KzRadius.lg))
                }
                KzDisclosure("Rules", isInitiallyExpanded: true) {
                    HStack(spacing: KzSpace.s3) {
                        PlaceholderSegmented(options: ["Inherit from group", "Custom"])
                        Text("Works in every app").kzTextStyle(.callout).foregroundStyle(KzColor.inkMuted)
                    }
                }
                KzDisclosure("Advanced", summary: "Word boundary, case, undo") { Text("") }
                Spacer(minLength: 0)
                KzHairline()
                Text("Created 1 Nov · Edited 2 Nov · 1 expansion")
                    .kzTextStyle(.caption)
                    .foregroundStyle(KzColor.inkMuted)
            }
            .padding(.horizontal, KzSpace.s6)
            .padding(.top, KzSpace.s2)
            .padding(.bottom, KzSpace.s6)
        }
    }
}

@MainActor
@Suite struct AppShellSnapshotTests {
    @Test func window() throws {
        try snapshot("app-shell", width: 1240 + 2 * KzSpace.s4, background: .canvas) {
            SampleAppShell().frame(width: 1240, height: 780)
        }
    }

    @Test func paneWidthsComeFromTokens() {
        #expect(KzAppShell<EmptyView, EmptyView, EmptyView, EmptyView, EmptyView>.minimumSize.width == KzSize.sidebar + KzSize.list + 360)
    }
}
