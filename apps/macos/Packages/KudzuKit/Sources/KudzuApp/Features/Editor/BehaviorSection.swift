import KudzuCore
import KudzuDesign
import SwiftUI

struct BehaviorSection: View {
    var snippet: Snippet
    var groups: [SnippetGroup]
    var update: (@escaping (inout Snippet) -> Void) -> Void
    @State private var apps = InstalledApps.shared
    @Environment(AppModel.self) private var model

    private var inheritedInsertion: (value: InsertionMethod, from: SnippetGroup)? {
        EditorLogic.inherited(groups, groupID: snippet.groupID, key: \.insertion)
    }
    private var inheritedRule: (value: AppRule, from: SnippetGroup)? {
        EditorLogic.inherited(groups, groupID: snippet.groupID, key: \.rule)
    }

    var body: some View {
        VStack(spacing: 0) {
            LazyVGrid(columns: [GridItem(.adaptive(minimum: 124), spacing: 12)], alignment: .leading, spacing: 12) {
                compactMenu("Expand", value: snippet.fireMode == .immediate ? "Instantly" : "After the word",
                            help: Text(snippet.fireMode == .immediate ? "As soon as the last trigger character is typed." : "After you finish the word — space, return or punctuation.")) {
                    Picker("When to expand", selection: binding(\.fireMode)) {
                        Text("Instantly").tag(FireMode.immediate)
                        Text("After the word").tag(FireMode.wholeWord)
                    }
                }
                compactMenu("Capitalization", value: snippet.caseMode == .propagate ? "Adapt" : "Exact", help: capitalizationDescription) {
                    Picker("Capitalization", selection: binding(\.caseMode)) {
                        Text("Adapt").tag(CaseMode.propagate)
                        Text("Exact").tag(CaseMode.exact)
                    }
                }
                compactMenu("Insert by", value: insertionLabel, help: Text(insertionDescription)) {
                    Picker("Insertion method", selection: binding(\.insertion)) {
                        Text("Default").tag(Optional<InsertionMethod>.none)
                        if snippet.insertion == .automatic { Text("Automatic").tag(Optional.some(InsertionMethod.automatic)) }
                        Text("Type").tag(Optional.some(InsertionMethod.type))
                        Text("Paste").tag(Optional.some(InsertionMethod.paste))
                    }
                }
            }
            .padding(.vertical, 14)
            Hairline()
            VStack(alignment: .leading, spacing: 12) {
                BehaviorRow("Apps", description: Text(ruleDescription)) {
                    Segmented("App rule", selection: Binding<AppRuleMode?>(get: { snippet.rule?.mode }, set: { mode in
                        update { s in s.rule = mode.map { AppRule(mode: $0, bundleIDs: s.rule?.bundleIDs ?? [], insertion: s.rule?.insertion) } }
                    }), options: [
                        SegmentOption(inheritedRule == nil ? "All apps" : "Inherit", value: nil),
                        SegmentOption("Only in…", value: .allow),
                        SegmentOption("Except…", value: .deny),
                    ])
                }
                if let rule = snippet.rule {
                    VStack(alignment: .leading, spacing: 12) {
                        AppPicker(value: Binding(get: { rule.bundleIDs }, set: { ids in update { $0.rule?.bundleIDs = ids } }),
                                  addLabel: rule.mode == .allow ? "Allow an app" : "Block an app")
                        if rule.mode == .allow, !rule.bundleIDs.isEmpty {
                            HStack(spacing: 8) {
                                Icon.settings2.icon(14)
                                Text("In these apps, insert by").font(.system(size: 12))
                                insertionPicker("Insertion in these apps", size: .sm, value: Binding(get: { rule.insertion }, set: { value in update { $0.rule?.insertion = value } }))
                            }
                            .foregroundStyle(Color.kz.mutedForeground)
                        }
                    }
                    .padding(.bottom, 4)
                    .rise()
                }
            }
            .padding(.vertical, 12)
        }
        .padding(.horizontal, 16)
        .depth(.card, fill: Color.kz.surface, in: RoundedRectangle(cornerRadius: Radius.xl))
        .accessibilityElement(children: .contain)
        .accessibilityLabel("Behavior")
        .task { apps.load(from: model.backend) }
    }

    private func compactMenu<Choices: View>(_ title: String, value: String, help: Text, @ViewBuilder choices: () -> Choices) -> some View {
        VStack(alignment: .leading, spacing: 6) {
            Text(title).font(.system(size: 11, weight: .medium)).foregroundStyle(Color.kz.mutedForeground)
            Menu { choices().pickerStyle(.inline) } label: {
                HStack(spacing: 6) {
                    Text(value).lineLimit(1)
                    Spacer(minLength: 0)
                    Icon.chevronDown.icon(10)
                }
                .frame(maxWidth: .infinity)
            }
            .kzMenuTrigger(.secondary, size: .sm, fullWidth: true)
            .accessibilityLabel(title)
            .accessibilityValue(value)
            .help(help)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }

    private var insertionLabel: String {
        switch snippet.insertion {
        case .type: "Type"
        case .paste: "Paste"
        case .automatic: "Automatic"
        case nil: "Default"
        }
    }

    private var capitalizationDescription: Text {
        guard snippet.caseMode == .propagate else { return Text("The trigger must match exactly; text is never changed.") }
        let mono = Font.kz.mono(12)
        return Text("Follows how you type the trigger: ") + Text(";sig").font(mono) + Text(" → text, ") + Text(";SIG").font(mono) + Text(" → TEXT.")
    }

    private func binding<Value>(_ key: WritableKeyPath<Snippet, Value>) -> Binding<Value> {
        Binding(get: { snippet[keyPath: key] }, set: { value in update { $0[keyPath: key] = value } })
    }

    private func insertionPicker(_ title: String, size: Segmented<InsertionMethod?>.Size = .md, value: Binding<InsertionMethod?>) -> some View {
        var options: [SegmentOption<InsertionMethod?>] = [SegmentOption("Default", value: nil)]
        if value.wrappedValue == .automatic { options.append(SegmentOption("Automatic", value: .automatic)) }
        options += [SegmentOption("Type", value: .type), SegmentOption("Paste", value: .paste)]
        return Segmented(title, selection: value, size: size, options: options)
    }

    private var insertionDescription: String {
        if snippet.formatting?.source.utf16.elementsEqual(snippet.content.utf16) == true {
            return (snippet.insertion ?? inheritedInsertion?.value) == .type
                ? "Types plain text. Images use their text alternatives."
                : "Pastes formatting and images. Apps that need plain text use the text alternative."
        }
        switch snippet.insertion {
        case .type: return "Types the text key by key. Most compatible."
        case .paste: return "Pastes through the clipboard, then restores it. Best for long text."
        case .automatic: return "Types short text and pastes long text."
        case nil:
            if let inheritedInsertion {
                let label = inheritedInsertion.value == .type ? "Typing" : inheritedInsertion.value == .paste ? "Pasting" : "Automatic"
                return "Inherited from “\(inheritedInsertion.from.name)”: \(label)"
            }
            return "Default — typing for short text, pasting for long or in terminals."
        }
    }

    private var ruleDescription: String {
        if let rule = snippet.rule { return EditorLogic.describeRule(rule, apps: apps.apps) }
        if let inheritedRule { return "Inherited from “\(inheritedRule.from.name)”: \(EditorLogic.describeRule(inheritedRule.value, apps: apps.apps))" }
        return "Works in every app (except excluded apps in Settings)."
    }
}

/// `Row` in `behavior.tsx`: title and description on the left, the control centered on the right.
private struct BehaviorRow<Control: View>: View {
    var title: String
    var description: Text
    var control: Control

    init(_ title: String, description: Text, @ViewBuilder control: () -> Control) {
        self.title = title
        self.description = description
        self.control = control()
    }

    var body: some View {
        HStack(spacing: 24) {
            VStack(alignment: .leading, spacing: 0) {
                Text(title).font(.system(size: 13, weight: .medium))
                description
                    .font(.system(size: 12))
                    .foregroundStyle(Color.kz.mutedForeground)
                    .fixedSize(horizontal: false, vertical: true)
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            control
        }
        .padding(.vertical, 12)
    }
}
