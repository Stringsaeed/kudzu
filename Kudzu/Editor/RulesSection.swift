import AppKit
import KudzuCore
import KudzuUI
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
        VStack(alignment: .leading, spacing: KzSpace.s3) {
            ViewThatFits(in: .horizontal) {
                HStack(alignment: .top, spacing: KzSpace.s3) {
                    ruleSourceControl.frame(maxWidth: .infinity, alignment: .leading)
                    if rule == nil {
                        summary(inheritedRuleDescription, inherited: hasInheritedRule)
                    }
                }
                VStack(alignment: .leading, spacing: KzSpace.s2) {
                    ruleSourceControl
                    if rule == nil {
                        summary(inheritedRuleDescription, inherited: hasInheritedRule)
                    }
                }
            }

            if rule != nil {
                customRule
            }

            RulesOptionRow("Insertion") {
                KzSelect("Insertion", value: insertionTitle) {
                    KzMenuItem(inheritedInsertionDescription, isChecked: insertion == nil) { insertion = nil }
                    KzMenuSeparator()
                    for method in [InsertionMethod.automatic, .type, .paste] {
                        KzMenuItem(method.title, isChecked: insertion == method) { insertion = method }
                    }
                }
                .help("Automatic types short plain text and pastes the rest. Choose Paste for apps that drop typed characters.")
            }
        }
    }

    private var insertionTitle: String {
        insertion?.title ?? inheritedInsertionDescription
    }

    /// One muted line: an inherit arrow when a group sets it, a check when nothing does.
    private func summary(_ text: String, inherited: Bool) -> some View {
        HStack(spacing: 6) {
            KzIcon(inherited ? .inherit : .check, size: 14)
            Text(text)
                .kzTextStyle(.caption)
                .lineLimit(2)
                .fixedSize(horizontal: false, vertical: true)
        }
        .foregroundStyle(KzColor.inkMuted)
        .accessibilityElement(children: .combine)
    }

    // MARK: - Custom

    private var ruleSourceControl: some View {
        KzSegmented("Apps", selection: source, options: [
            .init(.inherit, label: inheritOptionTitle),
            .init(.custom, label: "Custom"),
        ])
    }

    private var customRule: some View {
        VStack(alignment: .leading, spacing: KzSpace.s3) {
            KzSegmented("Mode", selection: ruleMode, options: [
                .init(.allow, label: "Allow only in"),
                .init(.deny, label: "Block in"),
            ])
            .frame(maxWidth: .infinity, alignment: .leading)

            let bundleIDs = rule?.bundleIDs ?? []
            if bundleIDs.isEmpty {
                if rule?.mode == .allow {
                    EditorNote(icon: .warning, tone: .warning,
                               text: AttributedString("No apps chosen, so this won't expand anywhere."))
                } else {
                    Text("No apps blocked yet.")
                        .kzTextStyle(.callout)
                        .foregroundStyle(KzColor.inkMuted)
                }
            } else {
                AppList(bundleIDs: bundleIDs) { id in
                    rule?.bundleIDs.removeAll { $0 == id }
                }
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

/// A label and a control on one line, as in the Window preview's "Insertion" row.
struct RulesOptionRow<Control: View>: View {
    private let label: String
    private let width: CGFloat
    private let control: Control

    init(_ label: String, width: CGFloat = 72, @ViewBuilder control: () -> Control) {
        self.label = label
        self.width = width
        self.control = control()
    }

    var body: some View {
        HStack(spacing: KzSpace.s3) {
            Text(label)
                .kzTextStyle(.bodyStrong)
                .foregroundStyle(KzColor.ink)
                .frame(width: width, alignment: .leading)
            control
            Spacer(minLength: 0)
        }
    }
}

/// The apps in a rule, on a `surface-sunken` well, each with a remove button.
struct AppList: View {
    var bundleIDs: [String]
    var remove: (String) -> Void

    var body: some View {
        let shape = RoundedRectangle(cornerRadius: KzRadius.md)
        VStack(spacing: 0) {
            ForEach(bundleIDs, id: \.self) { id in
                AppRow(bundleID: id) { remove(id) }
                if id != bundleIDs.last {
                    Rectangle().fill(KzColor.line).frame(height: 1).padding(.leading, 44)
                }
            }
        }
        .background(KzColor.surfaceSunken, in: shape)
        .frame(maxWidth: 420, alignment: .leading)
    }
}

/// Pull-down that adds a running app or opens a file chooser in `/Applications`.
struct AddAppMenu: View {
    var existing: Set<String>
    var onAdd: (String) -> Void

    var body: some View {
        KzButton("Add app", size: .sm, icon: .plus, iconEnd: .chevronDown, menuTitle: "Add app") {
            // Built when the menu opens, so the running apps are current.
            let running = AppCatalog.runningApps(excluding: existing)
            if !running.isEmpty {
                KzMenuHeader("Running apps")
                for app in running {
                    KzMenuItem(app.name, icon: .app) { onAdd(app.bundleID) }
                }
                KzMenuSeparator()
            }
            KzMenuItem("Choose app…", icon: .folder) {
                if let id = AppCatalog.chooseApp() { onAdd(id) }
            }
        }
    }
}

/// An app's icon, name and bundle ID, with a remove button.
struct AppRow: View {
    var bundleID: String
    var remove: () -> Void
    @Environment(\.kzSnapshotRendering) private var snapshotRendering

    var body: some View {
        let info = AppCatalog.info(for: bundleID)
        HStack(spacing: 10) {
            Group {
                if snapshotRendering {
                    // NSWorkspace icons composite badly in ImageRenderer (they dim the whole image).
                    KzIcon(.app, size: 20).foregroundStyle(KzColor.inkMuted)
                } else {
                    Image(nsImage: info.icon).resizable()
                }
            }
            .frame(width: 24, height: 24)
            .opacity(info.isInstalled ? 1 : 0.5)
            .accessibilityHidden(true)
            VStack(alignment: .leading, spacing: 0) {
                Text(info.name)
                    .kzTextStyle(.bodyStrong)
                    .foregroundStyle(KzColor.ink)
                    .lineLimit(1)
                Text(info.isInstalled ? bundleID : "Not installed")
                    .kzTextStyle(.monoCaption)
                    .foregroundStyle(KzColor.inkMuted)
                    .lineLimit(1)
                    .truncationMode(.middle)
            }
            Spacer(minLength: KzSpace.s2)
            KzIconButton(.x, label: "Remove \(info.name)", size: .sm, action: remove)
        }
        .padding(.horizontal, 10)
        .padding(.vertical, 6)
        .accessibilityElement(children: .contain)
    }
}
