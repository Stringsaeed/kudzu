import AppKit
import KudzuCore
import SwiftUI

/// Where a snippet is allowed to expand, and how its text is inserted.
struct RulesSection: View {
    @Environment(LibraryModel.self) private var library
    var model: SnippetEditorModel

    private enum RuleSource: Hashable { case inherit, custom }

    private var snippet: SnippetRecord { model.draft }

    private var source: Binding<RuleSource> {
        Binding(
            get: { snippet.rule == nil ? .inherit : .custom },
            set: { newValue in
                model.edit { $0.rule = newValue == .inherit ? nil : ($0.rule ?? AppRule(mode: .deny, bundleIDs: [])) }
            }
        )
    }

    private var ruleMode: Binding<AppRuleMode> {
        Binding(
            get: { snippet.rule?.mode ?? .deny },
            set: { mode in model.edit { $0.rule?.mode = mode } }
        )
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            VStack(alignment: .leading, spacing: 8) {
                Picker("Apps", selection: source) {
                    Text("Inherit from group").tag(RuleSource.inherit)
                    Text("Custom").tag(RuleSource.custom)
                }
                .pickerStyle(.segmented)
                .labelsHidden()

                if snippet.rule == nil {
                    inheritedSummary
                } else {
                    customRule
                }
            }

            LabeledContent("Insertion") {
                Picker("Insertion", selection: model.binding(\.insertion)) {
                    Text(inheritedInsertionTitle).tag(InsertionMethod?.none)
                    Divider()
                    Text("Automatic").tag(InsertionMethod?.some(.automatic))
                    Text("Type").tag(InsertionMethod?.some(.type))
                    Text("Paste").tag(InsertionMethod?.some(.paste))
                }
                .labelsHidden()
                .fixedSize()
            }
            .help("Automatic types short plain text and pastes the rest. Choose Paste for apps that drop typed characters.")
        }
    }

    // MARK: - Inherited

    private var inheritedSummary: some View {
        Group {
            if let inherited = library.tree.inheritedRule(forGroup: snippet.groupID) {
                Label {
                    Text("\(describe(inherited.rule)) — set on “\(inherited.from.name)”.")
                } icon: {
                    Image(systemName: "arrow.turn.down.right")
                }
            } else {
                Label("Works in every app.", systemImage: "checkmark.circle")
            }
        }
        .font(.callout)
        .foregroundStyle(.secondary)
    }

    private var inheritedInsertionTitle: String {
        if let inherited = library.tree.inheritedInsertion(forGroup: snippet.groupID) {
            return "Inherit (\(inherited.method.title))"
        }
        return "Inherit"
    }

    private func describe(_ rule: AppRule) -> String {
        let names = rule.bundleIDs.map { AppCatalog.info(for: $0).name }.joined(separator: ", ")
        if rule.bundleIDs.isEmpty { return rule.mode == .allow ? "Disabled everywhere" : "Works in every app" }
        return (rule.mode == .allow ? "Only in " : "Everywhere except ") + names
    }

    // MARK: - Custom

    private var customRule: some View {
        VStack(alignment: .leading, spacing: 10) {
            Picker("Mode", selection: ruleMode) {
                Text("Allow only in").tag(AppRuleMode.allow)
                Text("Block in").tag(AppRuleMode.deny)
            }
            .pickerStyle(.segmented)
            .labelsHidden()

            let bundleIDs = snippet.rule?.bundleIDs ?? []
            if bundleIDs.isEmpty {
                Text(snippet.rule?.mode == .allow
                     ? "No apps chosen, so this snippet won't expand anywhere."
                     : "No apps blocked yet.")
                    .font(.callout)
                    .foregroundStyle(snippet.rule?.mode == .allow ? Color.orange : Color.secondary)
            } else {
                VStack(spacing: 0) {
                    ForEach(bundleIDs, id: \.self) { id in
                        AppRow(bundleID: id) {
                            model.edit { $0.rule?.bundleIDs.removeAll { $0 == id } }
                        }
                        if id != bundleIDs.last { Divider() }
                    }
                }
                .background(.fill.quaternary, in: RoundedRectangle(cornerRadius: 8))
            }

            addAppMenu(existing: Set(bundleIDs))
        }
    }

    private func addAppMenu(existing: Set<String>) -> some View {
        Menu {
            let running = AppCatalog.runningApps(excluding: existing)
            if !running.isEmpty {
                Section("Running Apps") {
                    ForEach(running) { app in
                        Button {
                            add(app.bundleID)
                        } label: {
                            Label {
                                Text(app.name)
                            } icon: {
                                Image(nsImage: Self.menuIcon(for: app.bundleID))
                            }
                        }
                    }
                }
            }
            Divider()
            Button("Choose App…") {
                if let id = AppCatalog.chooseApp() { add(id) }
            }
        } label: {
            Label("Add App", systemImage: "plus")
        }
        .menuStyle(.button)
        .fixedSize()
    }

    private func add(_ bundleID: String) {
        model.edit { snippet in
            guard var rule = snippet.rule, !rule.bundleIDs.contains(bundleID) else { return }
            rule.bundleIDs.append(bundleID)
            snippet.rule = rule
        }
    }

    /// Menu images render at their natural size, so shrink app icons to menu size.
    private static func menuIcon(for bundleID: String) -> NSImage {
        let icon = AppCatalog.info(for: bundleID).icon.copy() as? NSImage ?? AppCatalog.info(for: bundleID).icon
        icon.size = NSSize(width: 16, height: 16)
        return icon
    }
}

private struct AppRow: View {
    var bundleID: String
    var remove: () -> Void

    var body: some View {
        let info = AppCatalog.info(for: bundleID)
        HStack(spacing: 10) {
            Image(nsImage: info.icon)
                .resizable()
                .frame(width: 24, height: 24)
                .opacity(info.isInstalled ? 1 : 0.5)
            VStack(alignment: .leading, spacing: 0) {
                Text(info.name)
                if info.isInstalled {
                    Text(bundleID).font(.caption).foregroundStyle(.secondary)
                } else {
                    Text("Not installed").font(.caption).foregroundStyle(.secondary)
                }
            }
            Spacer()
            Button(action: remove) {
                Image(systemName: "minus.circle.fill")
                    .foregroundStyle(.secondary)
            }
            .buttonStyle(.plain)
            .help("Remove \(info.name)")
            .accessibilityLabel("Remove \(info.name)")
        }
        .padding(.horizontal, 10)
        .padding(.vertical, 6)
    }
}

extension InsertionMethod {
    /// Name shown in menus.
    var title: String {
        switch self {
        case .automatic: "Automatic"
        case .type: "Type"
        case .paste: "Paste"
        }
    }
}
