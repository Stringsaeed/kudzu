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
            if self.isComposingText { return false }
            let flags = event.modifierFlags.intersection([.command, .option, .control, .shift])
            switch Int(event.keyCode) {
            case kVK_Escape:
                self.finish(nil)
                return true
            case kVK_Tab:
                model.moveFocus(backward: flags.contains(.shift))
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
    let name: String
    let fields: [FormFieldSpec]
    var values: [String: String]
    var focus: String?

    init(name: String, fields: [FormFieldSpec], values: [String: String]) {
        self.name = name
        self.fields = fields
        self.values = values
        focus = fields.first?.name
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
            // Header
            HStack(spacing: KzSpace.s2) {
                Text(model.name.isEmpty ? "Fill in snippet" : model.name)
                    .kzTextStyle(.title)
                    .foregroundStyle(KzColor.ink)
                    .lineLimit(1)
                Spacer(minLength: KzSpace.s2)
                KzKbd("esc")
            }
            .padding(.horizontal, KzSpace.s4)
            .padding(.top, KzSpace.s4)
            .padding(.bottom, KzSpace.s3)

            Rectangle().fill(KzColor.line).frame(height: 1)

            // Fields & Live Preview
            ScrollView {
                VStack(alignment: .leading, spacing: KzSpace.s4) {
                    KzFieldStack {
                        ForEach(model.fields, id: \.name) { field in
                            KzField(field.name) {
                                control(field)
                            }
                        }
                    }

                    VStack(alignment: .leading, spacing: KzSpace.s2) {
                        Text("Preview")
                            .kzTextStyle(.headline)
                            .foregroundStyle(KzColor.ink)
                        let rendered = preview(model.values)
                        let previewShape = RoundedRectangle(cornerRadius: KzRadius.md, style: .continuous)
                        Text(rendered.isEmpty ? " " : rendered)
                            .kzTextStyle(.content)
                            .foregroundStyle(KzColor.ink)
                            .textSelection(.enabled)
                            .frame(maxWidth: .infinity, alignment: .leading)
                            .padding(.horizontal, KzSpace.s3)
                            .padding(.vertical, KzSpace.s2)
                            .background(KzColor.surfaceRaised, in: previewShape)
                            .overlay(previewShape.strokeBorder(KzColor.line))
                    }
                }
                .padding(KzSpace.s4)
            }
            .frame(maxHeight: 340)
            .fixedSize(horizontal: false, vertical: true)

            Rectangle().fill(KzColor.line).frame(height: 1)

            // Footer actions
            HStack(spacing: KzSpace.s3) {
                KzButton("Cancel", variant: .secondary, action: cancel)
                Spacer()
                HStack(spacing: KzSpace.s1) {
                    KzKbd("⌘↵")
                    Text("to insert")
                        .kzTextStyle(.caption)
                        .foregroundStyle(KzColor.inkMuted)
                }
                KzButton("Insert", variant: .primary, isDefault: true, action: submit)
            }
            .padding(.horizontal, KzSpace.s4)
            .padding(.vertical, KzSpace.s3)
        }
        .frame(width: 400)
        .onAppear { focus = model.focus }
        .onChange(of: model.focus) { _, value in focus = value }
        .onChange(of: focus) { _, value in model.focus = value }
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
            KzSelect(
                field.name,
                selection: value,
                options: field.options.map { ($0, $0) }
            )
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
        KzTextInput(placeholder, text: $text, focus: $isFocused)
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
        let shape = RoundedRectangle(cornerRadius: KzRadius.md, style: .continuous)
        KzInteractive(shape, stateLayer: false, focusRing: .edge, isFocused: isFocused) { state in
            Group {
                if isSnapshot {
                    Text(text.isEmpty ? " " : text)
                        .kzTextStyle(.body)
                        .foregroundStyle(state.isEnabled ? KzColor.ink : KzDisabled.foreground)
                        .frame(maxWidth: .infinity, minHeight: 72, alignment: .topLeading)
                        .padding(.horizontal, 10)
                        .padding(.vertical, KzSpace.s2)
                } else {
                    TextEditor(text: $text)
                        .focused(focus, equals: fieldName)
                        .font(KzTextStyle.body.font)
                        .foregroundStyle(state.isEnabled ? KzColor.ink : KzDisabled.foreground)
                        .scrollContentBackground(.hidden)
                        .padding(.horizontal, 6)
                        .padding(.vertical, KzSpace.s2)
                        .frame(minHeight: 78, maxHeight: 110)
                }
            }
            .background(
                state.isFocused && state.isEnabled ? KzColor.surfaceRaised : KzColor.surfaceSunken,
                in: shape
            )
            .overlay(
                shape.strokeBorder(state.isHovered && !state.isFocused ? KzColor.line : .clear)
            )
        }
    }
}
