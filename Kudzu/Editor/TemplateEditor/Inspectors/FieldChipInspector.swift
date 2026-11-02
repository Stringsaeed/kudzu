import KudzuCore
import KudzuUI
import SwiftUI

/// Text field chips: name, default value and multiline.
struct FieldChipInspector: View {
    let tag: KudzuCore.Tag
    let context: ChipInspectorContext
    let onChange: (KudzuCore.Tag) -> Void

    @State private var name: String
    @State private var defaultValue: String
    @State private var multiline: Bool
    @FocusState private var nameFocused: Bool

    init(tag: KudzuCore.Tag, context: ChipInspectorContext, onChange: @escaping (KudzuCore.Tag) -> Void) {
        self.tag = tag
        self.context = context
        self.onChange = onChange
        _name = State(initialValue: tag.stringAttribute("name") ?? "")
        _defaultValue = State(initialValue: tag.stringAttribute("default") ?? "")
        _multiline = State(initialValue: tag.boolAttribute("multiline"))
    }

    private var problem: String? {
        let trimmed = name.trimmingCharacters(in: .whitespaces)
        if trimmed.isEmpty { return "A field needs a name." }
        if context.nameConflicts(trimmed, multiline ? .multiline : .text) {
            return "Another control already uses “\(trimmed)”. Pick a different name."
        }
        return nil
    }

    var body: some View {
        InspectorStack {
            InspectorHeader(tag: tag, title: "Text Field",
                            subtitle: "Asks for text before expanding. Fields with the same name share one value.")
            KzField("Name") {
                VStack(alignment: .leading, spacing: KzSpace.s2) {
                    KzTextInput("e.g. email", text: $name, focus: $nameFocused)
                    ValidationMessage(text: problem)
                }
            }
            KzField("Default") {
                KzTextInput("Optional", text: $defaultValue)
            }
            InspectorToggleRow(title: "Multiline", isOn: $multiline)
        }
        .onAppear { nameFocused = true }
        .onChange(of: name) { commit() }
        .onChange(of: defaultValue) { commit() }
        .onChange(of: multiline) { commit() }
    }

    private func commit() {
        guard problem == nil else { return }
        onChange(tag
            .setting("name", to: .string(name.trimmingCharacters(in: .whitespaces)))
            .setting("default", string: defaultValue)
            .setting("multiline", flag: multiline))
    }
}
