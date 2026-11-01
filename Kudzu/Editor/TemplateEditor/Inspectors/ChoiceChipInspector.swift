import KudzuCore
import SwiftUI

/// Dropdown chips: name, an editable and reorderable options list, and the default option.
struct ChoiceChipInspector: View {
    let tag: KudzuCore.Tag
    let context: ChipInspectorContext
    let onChange: (KudzuCore.Tag) -> Void

    private struct Option: Identifiable, Equatable {
        let id = UUID()
        var text: String
    }

    @State private var name: String
    @State private var options: [Option]
    @State private var defaultValue: String?
    @FocusState private var focus: UUID?
    @FocusState private var nameFocused: Bool

    init(tag: KudzuCore.Tag, context: ChipInspectorContext, onChange: @escaping (KudzuCore.Tag) -> Void) {
        self.tag = tag
        self.context = context
        self.onChange = onChange
        _name = State(initialValue: tag.stringAttribute("name") ?? "")
        _options = State(initialValue: ChoiceOptions.decode(tag.stringAttribute("options") ?? "").map { Option(text: $0) })
        _defaultValue = State(initialValue: tag.stringAttribute("default"))
    }

    private var problem: String? {
        let trimmed = name.trimmingCharacters(in: .whitespaces)
        if trimmed.isEmpty { return "A dropdown needs a name." }
        if context.nameConflicts(trimmed, .choice) { return "Another control already uses “\(trimmed)”. Pick a different name." }
        return nil
    }

    /// Distinct, non-empty options eligible as the default.
    private var defaultCandidates: [String] {
        var seen = Set<String>()
        return options.map(\.text).filter { !$0.isEmpty && seen.insert($0).inserted }
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            InspectorHeader(tag: tag, title: "Dropdown", subtitle: "Asks the user to pick one option before expanding.")
            Form {
                TextField("Name", text: $name, prompt: Text("e.g. tone"))
                    .focused($nameFocused)
                ValidationMessage(text: problem)
            }
            .formStyle(.columns)
            optionsList
            Form {
                Picker("Default", selection: $defaultValue) {
                    Text("None").tag(String?.none)
                    if !defaultCandidates.isEmpty { Divider() }
                    ForEach(defaultCandidates, id: \.self) { option in Text(option).tag(Optional(option)) }
                }
            }
            .formStyle(.columns)
        }
        .onAppear {
            if options.isEmpty { options.append(Option(text: "")) }
            nameFocused = true
        }
        .onChange(of: name) { commit() }
        .onChange(of: options) {
            if let value = defaultValue, !defaultCandidates.contains(value) { defaultValue = nil }
            commit()
        }
        .onChange(of: defaultValue) { commit() }
    }

    private var optionsList: some View {
        VStack(alignment: .leading, spacing: 6) {
            Text("Options").font(.subheadline.weight(.medium)).foregroundStyle(.secondary)
            List {
                ForEach($options) { $option in
                    HStack(spacing: 6) {
                        Image(systemName: "line.3.horizontal")
                            .foregroundStyle(.tertiary)
                            .accessibilityHidden(true)
                        TextField("Option", text: $option.text, prompt: Text("Option"))
                            .textFieldStyle(.plain)
                            .focused($focus, equals: option.id)
                            .onSubmit(addOption)
                        Button {
                            options.removeAll { $0.id == option.id }
                        } label: {
                            Image(systemName: "minus.circle.fill").foregroundStyle(.secondary)
                        }
                        .buttonStyle(.borderless)
                        .accessibilityLabel("Remove \(option.text.isEmpty ? "option" : option.text)")
                    }
                }
                .onMove { from, to in options.move(fromOffsets: from, toOffset: to) }
            }
            .listStyle(.bordered(alternatesRowBackgrounds: true))
            .frame(height: min(CGFloat(max(options.count, 2)) * 26 + 6, 170))
            Button(action: addOption) {
                Label("Add Option", systemImage: "plus")
            }
            .buttonStyle(.borderless)
            .controlSize(.small)
        }
    }

    private func addOption() {
        let option = Option(text: "")
        options.append(option)
        focus = option.id
    }

    private func commit() {
        guard problem == nil else { return }
        // Empty rows are being typed into; they are not saved as options.
        let values = options.map(\.text).filter { !$0.isEmpty }
        onChange(tag
            .setting("name", to: .string(name.trimmingCharacters(in: .whitespaces)))
            .setting("options", string: ChoiceOptions.encode(values))
            .setting("default", string: defaultValue))
    }
}
