import KudzuCore
import KudzuUI
import SwiftUI

/// Dropdown chips: name, an editable and reorderable options list, and the default option.
struct ChoiceChipInspector: View {
    let tag: KudzuCore.Tag
    let context: ChipInspectorContext
    let onChange: (KudzuCore.Tag) -> Void

    fileprivate struct Option: Identifiable, Equatable {
        let id = UUID()
        var text: String
    }

    @State private var name: String
    @State private var options: [Option]
    @State private var defaultValue: String?
    /// The option row that should take focus, set when a row is added.
    @State private var focusRequest: UUID?
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
        InspectorStack {
            InspectorHeader(tag: tag, title: "Dropdown", subtitle: "Asks the user to pick one option before expanding.")
            KzField("Name") {
                VStack(alignment: .leading, spacing: KzSpace.s2) {
                    KzTextInput("e.g. tone", text: $name, focus: $nameFocused)
                    ValidationMessage(text: problem)
                }
            }
            KzField("Options", aside: options.count > 1 ? "Drag to reorder" : nil) {
                VStack(alignment: .leading, spacing: KzSpace.s2) {
                    optionsList
                    KzButton("Add option", variant: .ghost, size: .sm, icon: .plus, action: addOption)
                }
            }
            KzField("Default") {
                KzSelect("Default", selection: $defaultValue,
                         options: [(String?.none, "None")] + defaultCandidates.map { (Optional($0), $0) })
            }
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

    /// A `List` for `onMove` reordering; each row is its own `KzTextInput` well.
    private var optionsList: some View {
        List {
            ForEach($options) { $option in
                OptionRow(text: $option.text, focusRequest: $focusRequest, id: option.id, onSubmit: addOption) {
                    options.removeAll { $0.id == option.id }
                }
                .listRowSeparator(.hidden)
                .listRowBackground(Color.clear)
                .listRowInsets(EdgeInsets(top: 2, leading: 0, bottom: 2, trailing: 0))
            }
            .onMove { from, to in options.move(fromOffsets: from, toOffset: to) }
        }
        .listStyle(.plain)
        .scrollContentBackground(.hidden)
        .frame(height: min(CGFloat(max(options.count, 1)) * 38 + 4, 176))
    }

    private func addOption() {
        let option = Option(text: "")
        options.append(option)
        focusRequest = option.id
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

/// One option: a grip, the text and a remove button.
private struct OptionRow: View {
    @Binding var text: String
    @Binding var focusRequest: UUID?
    let id: UUID
    let onSubmit: () -> Void
    let onRemove: () -> Void
    @FocusState private var focused: Bool

    var body: some View {
        HStack(spacing: KzSpace.s1) {
            KzIcon(.grip, size: 14)
                .foregroundStyle(KzColor.inkFaint)
                .frame(width: 18)
                .accessibilityHidden(true)
            KzTextInput("Option", text: $text, focus: $focused)
                .onSubmit(onSubmit)
            KzIconButton(.x, label: "Remove \(text.isEmpty ? "option" : text)", size: .sm, action: onRemove)
        }
        .onAppear(perform: takeFocusIfRequested)
        .onChange(of: focusRequest) { takeFocusIfRequested() }
    }

    private func takeFocusIfRequested() {
        guard focusRequest == id else { return }
        focused = true
        focusRequest = nil
    }
}
