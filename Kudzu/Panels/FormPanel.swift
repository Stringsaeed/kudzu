import AppKit
import Carbon
import KudzuCore
import KudzuUI
import SwiftUI

/// A form that takes keyboard focus without activating Kudzu, hosted on a `KzFloatingPanel`.
@MainActor
final class FormPanel: KzFloatingPanel {
    private var completion: (([String: String]?) -> Void)?

    init() {
        super.init(sizing: .fitting, chrome: .glass(), behavior: .modeless, level: .floating) {
            EmptyView()
        }
        onClose = { [weak self] _ in
            self?.finish(nil)
        }
    }

    func present(name: String, fields: [FormFieldSpec], values: [String: String],
                 preview: @escaping ([String: String]) -> String) async -> [String: String]? {
        let location = CaretLocator().locate()
        title = name
        let model = FormModel(name: name, fields: fields, values: values)
        setContent {
            FormPanelView(
                model: model,
                preview: preview,
                submit: { [weak self] in self?.finish(model.values) },
                cancel: { [weak self] in self?.finish(nil) }
            )
        }

        let placement: KzFloatingPlacement
        if location.source == .mouse {
            placement = .centered()
        } else {
            placement = .edge(.bottom, anchor: location.rect, gap: 6)
        }

        let screen = NSScreen.screens.first { $0.frame.contains(location.rect.origin) } ?? NSScreen.main

        keyHandler = { [weak self] event in
            guard let self else { return false }
            if self.isComposingText || KzMenu.isOpen || model.isConfirmingDiscard || self.attachedSheet != nil { return false }
            let flags = event.modifierFlags.intersection([.command, .option, .control, .shift])
            switch Int(event.keyCode) {
            case kVK_Escape:
                if model.handleEscape() == .closeImmediately {
                    self.finish(nil)
                }
                return true
            case kVK_Tab:
                if flags.contains(.shift) { self.selectPreviousKeyView(nil) }
                else { self.selectNextKeyView(nil) }
                return true
            case kVK_Return, kVK_ANSI_KeypadEnter:
                let multiline = fields.first { $0.name == model.focus }?.kind == .multiline
                if !multiline || flags.contains(.command) {
                    self.finish(model.values)
                    return true
                }
            default:
                break
            }
            return false
        }

        return await withCheckedContinuation { continuation in
            completion = { continuation.resume(returning: $0) }
            super.present(placement, on: screen, makeKey: true)
        }
    }

    func cancel() {
        finish(nil)
    }

    private func finish(_ values: [String: String]?) {
        // Take the continuation first: `dismiss` calls `onClose`, which re-enters with nil and
        // would otherwise resume it as a cancel before the submitted values arrive.
        let callback = completion
        completion = nil
        keyHandler = nil
        if isPresented {
            dismiss(.programmatic)
        }
        orderOut(nil)
        callback?(values)
    }
}

@MainActor @Observable
final class FormModel {
    enum CancelOutcome: Equatable {
        case closeImmediately
        case promptDiscard
        case dismissedDiscardPrompt
    }

    let name: String
    let fields: [FormFieldSpec]
    let initialValues: [String: String]
    var values: [String: String] {
        didSet {
            if !isDirty {
                isConfirmingDiscard = false
            }
        }
    }
    var focus: String?
    var isConfirmingDiscard = false

    init(name: String, fields: [FormFieldSpec], values: [String: String]) {
        self.name = name
        self.fields = fields
        self.initialValues = values
        self.values = values
        focus = fields.first?.name
    }

    var isDirty: Bool {
        values != initialValues
    }

    @discardableResult
    func handleEscape() -> CancelOutcome {
        if isConfirmingDiscard {
            isConfirmingDiscard = false
            return .dismissedDiscardPrompt
        }
        if isDirty {
            isConfirmingDiscard = true
            return .promptDiscard
        }
        return .closeImmediately
    }

    @discardableResult
    func requestCancel() -> CancelOutcome {
        if isDirty {
            isConfirmingDiscard = true
            return .promptDiscard
        }
        return .closeImmediately
    }

    func moveFocus(backward: Bool) {
        guard !fields.isEmpty else { return }
        let index = fields.firstIndex { $0.name == focus } ?? 0
        focus = fields[(index + (backward ? fields.count - 1 : 1)) % fields.count].name
    }
}

struct FormPanelView: View {
    @Bindable var model: FormModel
    let preview: ([String: String]) -> String
    let submit: () -> Void
    let cancel: () -> Void
    @FocusState private var focus: String?

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            HStack(spacing: 10) {
                Text(model.name.isEmpty ? "Fill in snippet" : model.name)
                    .font(WorkspaceTheme.label)
                    .foregroundStyle(WorkspaceTheme.ink)
                    .lineLimit(1)
                Spacer(minLength: 8)
                WorkspaceKeyHint(keys: "esc")
            }
            .padding(.horizontal, 16)
            .padding(.top, 14)
            .padding(.bottom, 10)

            WorkspaceHairline()

            ScrollView {
                VStack(alignment: .leading, spacing: 16) {
                    ForEach(model.fields, id: \.name) { field in
                        VStack(alignment: .leading, spacing: 6) {
                            Text(field.name)
                                .font(WorkspaceTheme.caption)
                                .foregroundStyle(WorkspaceTheme.secondary)
                            control(field)
                        }
                    }

                    VStack(alignment: .leading, spacing: 6) {
                        Text("Preview")
                            .font(WorkspaceTheme.caption)
                            .foregroundStyle(WorkspaceTheme.secondary)
                        let rendered = preview(model.values)
                        Text(rendered.isEmpty ? " " : rendered)
                            .font(WorkspaceTheme.body)
                            .foregroundStyle(WorkspaceTheme.ink)
                            .textSelection(.enabled)
                            .frame(maxWidth: .infinity, alignment: .leading)
                            .padding(.vertical, 8)
                            .overlay(alignment: .top) { WorkspaceHairline() }
                    }
                }
                .padding(16)
            }
            .frame(maxHeight: 340)
            .fixedSize(horizontal: false, vertical: true)

            WorkspaceHairline()

            HStack(spacing: 12) {
                Button("Cancel") {
                    if model.requestCancel() == .closeImmediately { cancel() }
                }
                Spacer()
                WorkspaceKeyHint(keys: "⌘↵", title: "to insert")
                Button("Insert", action: submit)
                    .keyboardShortcut(.defaultAction)
                    .buttonStyle(.borderedProminent)
                    .tint(WorkspaceTheme.accent)
            }
            .padding(.horizontal, 16)
            .padding(.vertical, 12)
        }
        .frame(width: 400)
        .background(WorkspaceTheme.paper)
        .onAppear { focus = model.focus }
        .onChange(of: model.focus) { _, value in focus = value }
        .onChange(of: focus) { _, value in model.focus = value }
        .confirmationDialog("Discard entered values?", isPresented: $model.isConfirmingDiscard,
                            titleVisibility: .visible) {
            Button("Discard", role: .destructive, action: cancel)
            Button("Keep editing", role: .cancel) {}
        }
    }

    @ViewBuilder
    private func control(_ field: FormFieldSpec) -> some View {
        let value = Binding(
            get: { model.values[field.name] ?? "" },
            set: { model.values[field.name] = $0 }
        )
        switch field.kind {
        case .text:
            FormSingleLineField(
                placeholder: field.defaultValue ?? "",
                text: value,
                fieldName: field.name,
                focus: $focus
            )
        case .multiline:
            KzMultilineTextInput(
                text: value,
                fieldName: field.name,
                focus: $focus
            )
        case .choice:
            Picker(field.name, selection: value) {
                ForEach(field.options, id: \.self) { option in
                    Text(option).tag(option)
                }
            }
            .labelsHidden()
            .frame(maxWidth: .infinity, alignment: .leading)
            .focused($focus, equals: field.name)
        }
    }
}

/// Bridges a string-keyed `@FocusState` to `KzTextInput`'s boolean `FocusState<Bool>.Binding`.
private struct FormSingleLineField: View {
    let placeholder: String
    @Binding var text: String
    let fieldName: String
    var focus: FocusState<String?>.Binding
    @FocusState private var isFocused: Bool

    var body: some View {
        TextField(placeholder, text: $text)
            .textFieldStyle(.roundedBorder)
            .accessibilityLabel(fieldName)
            .focused($isFocused)
            .onAppear {
                isFocused = (focus.wrappedValue == fieldName)
            }
            .onChange(of: focus.wrappedValue) { _, newValue in
                let shouldFocus = (newValue == fieldName)
                if isFocused != shouldFocus { isFocused = shouldFocus }
            }
            .onChange(of: isFocused) { _, focused in
                if focused, focus.wrappedValue != fieldName {
                    focus.wrappedValue = fieldName
                }
            }
    }
}

/// Minimal KudzuUI-styled multiline text editor for `FormPanel` (`KudzuUI` provides single-line
/// `KzTextInput` and the chip-based `KzEditorTextView`, so `Panels` supplies this plain multiline well).
struct KzMultilineTextInput: View {
    @Binding var text: String
    let fieldName: String
    var focus: FocusState<String?>.Binding
    @Environment(\.kzSnapshotRendering) private var isSnapshot
    @Environment(\.isEnabled) private var isEnabled

    private var isFocused: Bool {
        focus.wrappedValue == fieldName
    }

    var body: some View {
        Group {
            if isSnapshot {
                Text(text.isEmpty ? " " : text)
                    .font(WorkspaceTheme.body)
                    .foregroundStyle(isEnabled ? WorkspaceTheme.ink : WorkspaceTheme.secondary)
                    .frame(maxWidth: .infinity, minHeight: 72, alignment: .topLeading)
            } else {
                TextEditor(text: $text)
                    .accessibilityLabel(fieldName)
                    .focused(focus, equals: fieldName)
                    .font(WorkspaceTheme.body)
                    .foregroundStyle(isEnabled ? WorkspaceTheme.ink : WorkspaceTheme.secondary)
                    .scrollContentBackground(.hidden)
                    .frame(minHeight: 78, maxHeight: 110)
            }
        }
        .padding(8)
        .overlay(RoundedRectangle(cornerRadius: 6).strokeBorder(WorkspaceTheme.line))
        .background(WorkspaceTheme.navigation)
    }
}
