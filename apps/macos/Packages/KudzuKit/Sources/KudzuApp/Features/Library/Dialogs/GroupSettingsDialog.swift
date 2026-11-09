// Group settings (`group-settings.tsx`). Snippets in the group inherit insertion and the app rule
// unless they set their own. `AppPicker` is the editor feature's control.

import KudzuCore
import KudzuDesign
import SwiftUI

struct GroupSettingsForm: View {
    var group: SnippetGroup
    var appPicker: @MainActor (Binding<[String]>, String) -> AnyView
    @Environment(AppModel.self) private var model
    @Environment(LibraryActions.self) private var actions
    @Environment(\.dismiss) private var dismiss
    @State private var draft: SnippetGroup
    @State private var saving = false
    @State private var saveGeneration = 0
    @FocusState private var nameFocused: Bool

    init(group: SnippetGroup, appPicker: @escaping @MainActor (Binding<[String]>, String) -> AnyView) {
        self.group = group
        self.appPicker = appPicker
        _draft = State(initialValue: group)
    }

    var body: some View {
        DialogLayout(
            "Group settings",
            description: "Snippets in this group (and its subgroups) inherit these unless they set their own.",
            width: 520,
            spacing: 20,
            headerSpacing: 4
        ) {
            VStack(alignment: .leading, spacing: 20) {
                HStack(spacing: 12) {
                    // `Input className="h-9 flex-1"`.
                    TextField("Group name", text: $draft.name)
                        .kzInput(height: 36)
                        .focused($nameFocused)
                        .accessibilityLabel("Group name")
                        .onSubmit(save)
                    // `<label className="flex items-center gap-2 text-xs text-muted-foreground"><Switch/>On</label>`.
                    HStack(spacing: 8) {
                        Toggle(draft.isEnabled ? "On" : "Off", isOn: $draft.isEnabled)
                            .kzSwitch()
                            .labelsHidden()
                        Text(draft.isEnabled ? "On" : "Off")
                            .font(.system(size: 12))
                            .foregroundStyle(Color.kz.mutedForeground)
                            .accessibilityHidden(true)
                    }
                }
                SettingsRow("Insert by", description: "How text is delivered for snippets in this group.", layout: .inline) {
                    Segmented("Insertion method", selection: insertion, options: [
                        SegmentOption("Default", value: InsertionChoice.default),
                        SegmentOption("Type", value: .type),
                        SegmentOption("Paste", value: .paste),
                    ])
                }
                VStack(alignment: .leading, spacing: 12) {
                    SettingsRow("Apps", description: "Where snippets in this group can expand.", layout: .inline) {
                        Segmented("App rule", selection: rule, options: [
                            SegmentOption("All apps", value: RuleChoice.none),
                            SegmentOption("Only in…", value: .allow),
                            SegmentOption("Except…", value: .deny),
                        ])
                    }
                    if draft.rule != nil {
                        appPicker(bundles, "Add app")
                    }
                }
            }
            .disabled(saving)
        } footer: {
            Button("Cancel", action: close)
                .buttonStyle(.kz(.ghost))
                .keyboardShortcut(.cancelAction)
                .disabled(saving)
            Button("Save", action: save)
                .buttonStyle(.kz())
                .keyboardShortcut(.defaultAction)
                .disabled(saving)
        }
        .interactiveDismissDisabled(saving)
        .onAppear { nameFocused = true }
        .onDisappear { saveGeneration &+= 1 }
    }

    private var insertion: Binding<InsertionChoice> {
        Binding(
            get: {
                switch draft.insertion {
                case .type: .type
                case .paste: .paste
                default: .default
                }
            },
            set: { choice in
                switch choice {
                case .default: draft.insertion = nil
                case .type: draft.insertion = .type
                case .paste: draft.insertion = .paste
                }
            }
        )
    }

    private var rule: Binding<RuleChoice> {
        Binding(
            get: {
                switch draft.rule?.mode {
                case .allow: .allow
                case .deny: .deny
                default: .none
                }
            },
            set: { choice in
                switch choice {
                case .none: draft.rule = nil
                case .allow: draft.rule = AppRule(mode: .allow, bundleIDs: draft.rule?.bundleIDs ?? [])
                case .deny: draft.rule = AppRule(mode: .deny, bundleIDs: draft.rule?.bundleIDs ?? [])
                }
            }
        )
    }

    private var bundles: Binding<[String]> {
        Binding(
            get: { draft.rule?.bundleIDs ?? [] },
            set: { ids in
                guard var rule = draft.rule else { return }
                rule.bundleIDs = ids
                draft.rule = rule
            }
        )
    }

    private func save() {
        guard !saving else { return }
        saving = true
        saveGeneration &+= 1
        let generation = saveGeneration
        var next = draft
        let trimmed = draft.name.trimmingCharacters(in: .whitespacesAndNewlines)
        next.name = trimmed.isEmpty ? group.name : trimmed
        let model = model
        let actions = actions
        let dismiss = dismiss
        // Close only after the write succeeds. `perform` toasts the error and leaves this
        // sheet and its draft in place so a rejected save can be retried.
        actions.perform {
            defer { if generation == saveGeneration { saving = false } }
            try await model.saveGroup(next)
            guard generation == saveGeneration, actions.groupSettings?.id == next.id else { return }
            actions.groupSettings = nil
            dismiss()
        }
    }

    private func close() {
        guard !saving else { return }
        actions.groupSettings = nil
        dismiss()
    }
}

private enum InsertionChoice: Hashable { case `default`, type, paste }
private enum RuleChoice: Hashable { case none, allow, deny }
