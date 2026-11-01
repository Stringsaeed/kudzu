import AppKit
import KudzuCore
import SwiftUI

/// Reusable editor for a snippet's or group's `AppRule` and `InsertionMethod` override.
struct RulesSection: View {
    @Binding var rule: AppRule?
    @Binding var insertion: InsertionMethod?
    var inheritedRuleDescription: String
    var hasInheritedRule: Bool
    var inheritedInsertionDescription: String
    var hasInheritedInsertion: Bool
    var inheritOptionTitle: String

    private enum RuleSource: Hashable { case inherit, custom }

    init(
        rule: Binding<AppRule?>,
        insertion: Binding<InsertionMethod?>,
        inheritedRuleDescription: String,
        hasInheritedRule: Bool = false,
        inheritedInsertionDescription: String,
        hasInheritedInsertion: Bool = false,
        inheritOptionTitle: String = "Inherit from group"
    ) {
        self._rule = rule
        self._insertion = insertion
        self.inheritedRuleDescription = inheritedRuleDescription
        self.hasInheritedRule = hasInheritedRule
        self.inheritedInsertionDescription = inheritedInsertionDescription
        self.hasInheritedInsertion = hasInheritedInsertion
        self.inheritOptionTitle = inheritOptionTitle
    }

    init(
        rule: Binding<AppRule?>,
        insertion: Binding<InsertionMethod?>,
        inherited: InheritedGroupSettings,
        defaultInsertion: InsertionMethod = .automatic,
        inheritOptionTitle: String = "Inherit from group"
    ) {
        self.init(
            rule: rule,
            insertion: insertion,
            inheritedRuleDescription: inherited.ruleDescription { AppCatalog.info(for: $0).name },
            hasInheritedRule: inherited.rule != nil,
            inheritedInsertionDescription: inherited.insertionDescription(defaultInsertion: defaultInsertion),
            hasInheritedInsertion: inherited.insertion != nil,
            inheritOptionTitle: inheritOptionTitle
        )
    }

    private var source: Binding<RuleSource> {
        Binding(
            get: { rule == nil ? .inherit : .custom },
            set: { newValue in
                rule = newValue == .inherit ? nil : (rule ?? AppRule(mode: .deny, bundleIDs: []))
            }
        )
    }

    private var ruleMode: Binding<AppRuleMode> {
        Binding(
            get: { rule?.mode ?? .deny },
            set: { mode in rule?.mode = mode }
        )
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            VStack(alignment: .leading, spacing: 8) {
                Picker("Apps", selection: source) {
                    Text(inheritOptionTitle).tag(RuleSource.inherit)
                    Text("Custom").tag(RuleSource.custom)
                }
                .pickerStyle(.segmented)
                .labelsHidden()

                if rule == nil {
                    inheritedRuleSummary
                } else {
                    customRule
                }
            }

            VStack(alignment: .leading, spacing: 6) {
                LabeledContent("Insertion") {
                    Picker("Insertion", selection: $insertion) {
                        Text(inheritedInsertionDescription).tag(InsertionMethod?.none)
                        Divider()
                        Text(InsertionMethod.automatic.title).tag(InsertionMethod?.some(.automatic))
                        Text(InsertionMethod.type.title).tag(InsertionMethod?.some(.type))
                        Text(InsertionMethod.paste.title).tag(InsertionMethod?.some(.paste))
                    }
                    .labelsHidden()
                    .fixedSize()
                }
                .help("Automatic types short plain text and pastes the rest. Choose Paste for apps that drop typed characters.")

                if insertion == nil {
                    inheritedInsertionSummary
                }
            }
        }
    }

    // MARK: - Inherited

    private var inheritedRuleSummary: some View {
        Label {
            Text(inheritedRuleDescription)
        } icon: {
            Image(systemName: hasInheritedRule ? "arrow.turn.down.right" : "checkmark.circle")
        }
        .font(.callout)
        .foregroundStyle(.secondary)
    }

    private var inheritedInsertionSummary: some View {
        Label {
            Text(inheritedInsertionDescription)
        } icon: {
            Image(systemName: hasInheritedInsertion ? "arrow.turn.down.right" : "checkmark.circle")
        }
        .font(.callout)
        .foregroundStyle(.secondary)
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

            let bundleIDs = rule?.bundleIDs ?? []
            if bundleIDs.isEmpty {
                Text(rule?.mode == .allow
                     ? "No apps chosen, so snippets won't expand anywhere."
                     : "No apps blocked yet.")
                    .font(.callout)
                    .foregroundStyle(rule?.mode == .allow ? Color.orange : Color.secondary)
            } else {
                VStack(spacing: 0) {
                    ForEach(bundleIDs, id: \.self) { id in
                        AppRow(bundleID: id) {
                            rule?.bundleIDs.removeAll { $0 == id }
                        }
                        if id != bundleIDs.last { Divider() }
                    }
                }
                .background(.fill.quaternary, in: RoundedRectangle(cornerRadius: 8))
            }

            AddAppMenu(existing: Set(bundleIDs)) { bundleID in
                add(bundleID)
            }
        }
    }

    private func add(_ bundleID: String) {
        guard var current = rule, !current.bundleIDs.contains(bundleID) else { return }
        current.bundleIDs.append(bundleID)
        rule = current
    }
}

/// Menu button that adds a running app or opens a file chooser for `/Applications`.
struct AddAppMenu: View {
    var existing: Set<String>
    var onAdd: (String) -> Void

    var body: some View {
        Menu {
            let running = AppCatalog.runningApps(excluding: existing)
            if !running.isEmpty {
                Section("Running Apps") {
                    ForEach(running) { app in
                        Button {
                            onAdd(app.bundleID)
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
                if let id = AppCatalog.chooseApp() { onAdd(id) }
            }
        } label: {
            Label("Add App", systemImage: "plus")
        }
        .menuStyle(.button)
        .fixedSize()
    }

    /// Menu images render at their natural size, so shrink app icons to menu size.
    private static func menuIcon(for bundleID: String) -> NSImage {
        let icon = AppCatalog.info(for: bundleID).icon.copy() as? NSImage ?? AppCatalog.info(for: bundleID).icon
        icon.size = NSSize(width: 16, height: 16)
        return icon
    }
}

/// A row displaying an app's icon, name, bundle ID, and a remove button.
struct AppRow: View {
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
