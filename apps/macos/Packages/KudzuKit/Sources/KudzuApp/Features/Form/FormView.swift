import KudzuCore
import KudzuDesign
import SwiftUI

/// The fill-in panel (`form-panel.tsx`). The shell hosts it at 440 pt wide and resizes the height
/// from ``AppModel/resizeForm(height:)``. Values start from ``FormPayload/initialValues``; there is no second form model.
public struct FormView: View {
    @Environment(AppModel.self) private var model
    @State private var session = FormSession()
    @State private var reportedHeight = 0.0
    @State private var previewHeight: CGFloat = 44
    @FocusState private var focus: Int?

    public init() {}

    public var body: some View {
        let request = model.formRequest
        let _ = session.sync(request, library: model.library)
        Group {
            if let request {
                card(request)
            } else {
                empty
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .top)
        .task(id: PreviewKey(revision: session.revision, library: model.library)) { await refresh(request) }
        .onAppear { focus = 0 }
        .onChange(of: session.token) { focus = 0 }
        .defaultFocus($focus, 0)
    }

    /// The web renders nothing until a request arrives.
    private var empty: some View {
        Color.clear.frame(maxWidth: .infinity, minHeight: 160)
    }

    private func card(_ request: FormPayload) -> some View {
        let theme = KZTheme(colorScheme)
        return VStack(spacing: 0) {
            header(request, theme: theme)
            hairline(theme)
            VStack(alignment: .leading, spacing: 12) {
                ForEach(Array(request.fields.enumerated()), id: \.offset) { index, field in
                    fieldControl(field, index: index, theme: theme)
                }
            }
            .padding(.horizontal, 16)
            .padding(.vertical, 14)
            preview(theme)
                .padding(.horizontal, 16)
                .padding(.bottom, 14)
            hairline(theme)
            footer(theme)
        }
        .background(theme[.popover].opacity(0.55))
        .foregroundStyle(theme[.popoverForeground])
        // The card's `onKeyDown`: Return submits unless a multi-line field has focus, Esc cancels.
        // Fields handle these first; this catches the choice field, whose pop-up menu doesn't.
        .onKeyPress(phases: .down) { keyPress($0, field: focus.flatMap { request.fields.indices.contains($0) ? request.fields[$0] : nil }) }
        .accessibilityElement(children: .contain)
        .accessibilityLabel("Fill in \(request.snippetName)")
        .background {
            GeometryReader { proxy in
                Color.clear.preference(key: FormHeightKey.self, value: proxy.size.height)
            }
        }
        .onPreferenceChange(FormHeightKey.self) { height in
            report(height)
        }
    }

    @Environment(\.colorScheme) private var colorScheme

    // MARK: Chrome

    private func header(_ request: FormPayload, theme: KZTheme) -> some View {
        HStack(spacing: 10) {
            Icon.textCursorInput.icon(14)
                .foregroundStyle(theme[.irisForeground])
                .frame(width: 24, height: 24)
                .background(theme[.irisSoft], in: RoundedRectangle(cornerRadius: Radius.md))
            Text(request.snippetName.isEmpty ? "Fill in snippet" : request.snippetName)
                .font(.system(size: 13, weight: .semibold))
                .lineLimit(1)
                .frame(maxWidth: .infinity, alignment: .leading)
            if let target = request.targetApp {
                Text("into \(target.name)")
                    .font(.system(size: 11))
                    .lineLimit(1)
                    .foregroundStyle(theme[.faint])
            }
        }
        .padding(.horizontal, 16)
        .padding(.vertical, 12)
        .background { WindowDragArea() }
    }

    /// `max-h-40 overflow-y-auto`: the box hugs its content up to 160 pt, then scrolls.
    private func preview(_ theme: KZTheme) -> some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 4) {
                Text("Preview")
                    .font(.system(size: 10.5, weight: .medium))
                    .foregroundStyle(theme[.faint])
                Group {
                    if let preview = session.preview {
                        FormattedText(preview)
                            .font(.system(size: 12.5))
                            .lineSpacing(5)
                            .foregroundStyle(theme[.foreground].opacity(0.85))
                            .frame(maxWidth: .infinity, alignment: .leading)
                            .textSelection(.enabled)
                    } else {
                        Text("…").font(.system(size: 12.5)).foregroundStyle(theme[.faint])
                    }
                }
            }
            .padding(.horizontal, 12)
            .padding(.vertical, 10)
            .frame(maxWidth: .infinity, alignment: .topLeading)
            .background {
                GeometryReader { proxy in
                    Color.clear.preference(key: PreviewHeightKey.self, value: proxy.size.height)
                }
            }
        }
        .scrollBounceBehavior(.basedOnSize)
        .frame(height: min(max(previewHeight, 1), 160))
        .onPreferenceChange(PreviewHeightKey.self) { previewHeight = $0 }
        .depth(.well, fill: theme[.foreground, alpha: 0.03], in: RoundedRectangle(cornerRadius: Radius.lg))
    }

    private func footer(_ theme: KZTheme) -> some View {
        HStack(spacing: 16) {
            hint("tab", "Next field", theme: theme)
            hint("escape", "Cancel", theme: theme)
            Spacer(minLength: 8)
            Button(action: submit) {
                HStack(spacing: 8) {
                    Text("Insert")
                    Shortcut("enter", tone: .onAccent)
                }
            }
            .buttonStyle(InsertButtonStyle())
            .accessibilityLabel("Insert")
        }
        .font(.system(size: 11.5))
        .foregroundStyle(theme[.mutedForeground])
        .padding(.horizontal, 16)
        .padding(.vertical, 10)
    }

    private func hint(_ combo: String, _ title: String, theme: KZTheme) -> some View {
        HStack(spacing: 6) {
            Shortcut(combo)
            Text(title)
        }
    }

    private func hairline(_ theme: KZTheme) -> some View {
        Rectangle().fill(theme[.hairline]).frame(height: 1)
    }

    // MARK: Fields

    private func fieldControl(_ field: FormFieldSpec, index: Int, theme: KZTheme) -> some View {
        VStack(alignment: .leading, spacing: 6) {
            Text(field.name)
                .font(.system(size: 12, weight: .medium))
                .foregroundStyle(theme[.mutedForeground])
            control(field, index: index)
        }
    }

    /// Every field shares the web's `common` well: `h-9 rounded-lg bg-foreground/[0.03] px-3
    /// text-[13.5px] shadow-well`, cleared with a `ring-2 ring-ring` while focused.
    @ViewBuilder private func control(_ field: FormFieldSpec, index: Int) -> some View {
        switch field.kind {
        case .choice:
            choice(field)
                .formWell(focused: focus == index)
                .focused($focus, equals: index)
                .accessibilityIdentifier("field-\(index)")
        case .multiline:
            // `rows={3} py-2 leading-relaxed`; the text view adds 5 pt of line padding to the 7.
            TextEditor(text: session.binding(field.name))
                .font(.system(size: Self.fieldFont))
                .lineSpacing(6)
                .scrollContentBackground(.hidden)
                .padding(.horizontal, 7)
                .padding(.vertical, 8)
                .frame(height: 82)
                .formWell(focused: focus == index)
                .focused($focus, equals: index)
                .autocorrectionDisabled()
                .accessibilityLabel(field.name)
                .accessibilityIdentifier("field-\(index)")
                .onKeyPress(phases: .down) { keyPress($0, field: field) }
        case .text:
            TextField(field.name, text: session.binding(field.name))
                .textFieldStyle(.plain)
                .labelsHidden()
                .font(.system(size: Self.fieldFont))
                .padding(.horizontal, 12)
                .frame(height: 36)
                .formWell(focused: focus == index)
                .focused($focus, equals: index)
                .autocorrectionDisabled()
                .accessibilityLabel(field.name)
                .accessibilityIdentifier("field-\(index)")
                .onKeyPress(phases: .down) { keyPress($0, field: field) }
        }
    }

    private static let fieldFont: CGFloat = 13.5

    /// `<select class="h-9 appearance-none pr-8">` + `ChevronDown`: the native menu, opened from a
    /// full-width well with the chevron at the trailing edge.
    private func choice(_ field: FormFieldSpec) -> some View {
        let value = session.values[field.name] ?? ""
        // A `<select>` whose value matches no option shows its first option.
        let shown = field.options.contains(value) ? value : (field.options.first ?? "")
        return Menu {
            Picker(field.name, selection: session.binding(field.name)) {
                ForEach(field.options, id: \.self) { option in
                    Text(option).tag(option)
                }
            }
            .pickerStyle(.inline)
            .labelsHidden()
        } label: {
            HStack(spacing: 8) {
                Text(shown)
                    .font(.system(size: Self.fieldFont))
                    .foregroundStyle(Color.kz.popoverForeground)
                    .lineLimit(1)
                    .frame(maxWidth: .infinity, alignment: .leading)
                Icon.chevronDown.icon(16).foregroundStyle(Color.kz.mutedForeground)
            }
            .padding(.leading, 12)
            .padding(.trailing, 10)
            .frame(maxWidth: .infinity)
            .frame(height: 36)
        }
        .menuStyle(.button)
        .buttonStyle(.press(scale: Press.scale, menuTrigger: true))
        .menuIndicator(.hidden)
        .accessibilityLabel(field.name)
        .accessibilityValue(shown)
    }

    private func keyPress(_ press: KeyPress, field: FormFieldSpec?) -> KeyPress.Result {
        let key: FormKey?
        switch press.key {
        case .escape: key = .escape
        case .return: key = .return(command: press.modifiers.contains(.command), control: press.modifiers.contains(.control))
        default: key = nil
        }
        guard let key else { return .ignored }
        switch FormKeys.effect(key, field: field?.kind) {
        case .submit:
            submit()
            return .handled
        case .cancel:
            cancel()
            return .handled
        case .newline:
            return .ignored
        }
    }

    // MARK: Actions

    private func submit() {
        guard let request = model.formRequest else { return }
        let values = session.values
        Task { await model.submitForm(token: request.token, values: values) }
    }

    private func cancel() {
        guard let request = model.formRequest else { return }
        Task { await model.cancelForm(token: request.token) }
    }

    private func refresh(_ request: FormPayload?) async {
        guard let request else { return }
        try? await Task.sleep(for: .milliseconds(50))
        guard !Task.isCancelled else { return }
        let formatting = model.library.snippets.first { $0.id == request.snippetId }?.formatting
        await session.refreshPreview(request) { values in
            try await model.backend.renderPreview(content: request.content, snippetId: request.snippetId, formValues: values, formatting: formatting)
        }
    }

    private struct PreviewKey: Hashable { var revision: Int; var library: Library }

    /// The native panel is the vibrant surface, so there is no extra webview padding. The shell clamps 160…680.
    private func report(_ height: CGFloat) {
        let next = FormLayout.reportedHeight(content: Double(height), vibrant: true)
        guard next >= 1, abs(next - reportedHeight) >= 1 else { return }
        reportedHeight = next
        Task { await model.resizeForm(height: next) }
    }
}

/// Field values and the live preview for the request currently on ``AppModel/formRequest``.
@MainActor @Observable
final class FormSession {
    var values: [String: String] = [:]
    var preview: RenderResult?
    private(set) var token: UInt64?
    private(set) var revision = 0

    func refreshPreview(_ request: FormPayload, render: ([String: String]) async throws -> RenderResult) async {
        let requestedRevision = revision
        let values = values
        guard !Task.isCancelled, token == request.token,
              let result = try? await render(values) else { return }
        guard !Task.isCancelled, token == request.token, revision == requestedRevision else { return }
        preview = result
    }

    func sync(_ request: FormPayload?, library: Library) {
        guard let request else {
            if token != nil {
                token = nil
                values = [:]
                preview = nil
            }
            return
        }
        guard token != request.token else { return }
        token = request.token
        values = FormValues.seed(request)
        preview = FormRendering.preview(request, values: values, library: library)
        revision += 1
    }

    func binding(_ name: String) -> Binding<String> {
        Binding(
            get: { self.values[name] ?? "" },
            set: { newValue in
                guard self.values[name] != newValue else { return }
                self.values[name] = newValue
                self.revision += 1
            }
        )
    }
}

/// An immediate preview from the in-memory library. The backend refresh replaces it after 50 ms,
/// which is how the web's `renderPreview` stays in step with nested snippets and the clipboard.
enum FormRendering {
    static func preview(_ request: FormPayload, values: [String: String], library: Library) -> RenderResult {
        let formatting = library.snippets.first { $0.id == request.snippetId }?.formatting
        let context = RenderContext(
            now: Date(),
            timeZone: .current,
            clipboard: nil,
            formValues: values,
            resolveSnippet: { id in library.snippets.first { $0.id == id }?.content },
            resolveFormatting: { id in library.snippets.first { $0.id == id }?.formatting },
            rootSnippetID: request.snippetId
        )
        return Template.renderFormatted(request.content, formatting: formatting, context: context)
    }
}

/// The web's field `common` classes: the well while idle; focused, the well clears
/// (`focus:bg-transparent focus:shadow-none`) and a 2 pt `ring` draws outside it, both over
/// `transition-shadow`.
private struct FormWell: ViewModifier {
    var focused: Bool

    func body(content: Content) -> some View {
        let shape = RoundedRectangle(cornerRadius: Radius.lg)
        content
            .focusEffectDisabled()
            .background {
                Color.clear
                    .depth(.well, fill: Color.kz.mix(.foreground, alpha: 0.03), in: shape)
                    .opacity(focused ? 0 : 1)
            }
            .overlay {
                RoundedRectangle(cornerRadius: Radius.lg + 2)
                    .strokeBorder(Color.kz.ring, lineWidth: 2)
                    .padding(-2)
                    .opacity(FormFieldChrome.needsFocusBoundary(focused: focused) ? 1 : 0)
                    .allowsHitTesting(false)
            }
            .themeAwareAnimation(.kz.colors, value: focused)
    }
}

extension View {
    fileprivate func formWell(focused: Bool) -> some View { modifier(FormWell(focused: focused)) }
}

/// The footer's Insert: `h-7 rounded-lg bg-iris px-3 text-xs font-medium text-iris-contrast
/// shadow-accent sheen-strong press hover:brightness-110 active:shadow-pressed active:bg-none`.
private struct InsertButtonStyle: ButtonStyle {
    func makeBody(configuration: Configuration) -> some View {
        InsertButton(configuration: configuration)
    }

    private struct InsertButton: View {
        var configuration: Configuration
        @State private var hovered = false

        var body: some View {
            let pressed = configuration.isPressed
            let shape = RoundedRectangle(cornerRadius: Radius.lg)
            configuration.label
                .font(.system(size: 12, weight: .medium))
                .foregroundStyle(Color.kz.irisContrast)
                .padding(.horizontal, 12)
                .frame(height: 28)
                .background {
                    Color.clear.depth(pressed ? .pressed : .accent, fill: Color.kz.iris, in: shape)
                    if !pressed { shape.fill(Sheen.strong.gradient) }
                }
                .brightness(hovered && !pressed ? 0.05 : 0)
                .contentShape(shape)
                .onHover { hovered = $0 }
                .themeAwareAnimation(.kz.hover, value: hovered)
                .themeAwareAnimation(.kz.hover, value: pressed)
                .press(pressed)
                .environment(\.kzKbdContext, .onFilledButton)
        }
    }
}

private struct PreviewHeightKey: PreferenceKey {
    static let defaultValue: CGFloat = 0
    static func reduce(value: inout CGFloat, nextValue: () -> CGFloat) { value = max(value, nextValue()) }
}

private struct FormHeightKey: PreferenceKey {
    static let defaultValue: CGFloat = 0
    static func reduce(value: inout CGFloat, nextValue: () -> CGFloat) { value = max(value, nextValue()) }
}

#Preview("Form") {
    FormPreviewHost()
}

#Preview("Form Dark") {
    FormPreviewHost().preferredColorScheme(.dark)
}

private struct FormPreviewHost: View {
    @State private var model: AppModel?

    var body: some View {
        Group {
            if let model {
                FormView().environment(model)
            } else {
                ProgressView()
            }
        }
        .frame(width: 440)
        .fixedSize(horizontal: false, vertical: true)
        .task { await load() }
    }

    private func load() async {
        guard model == nil else { return }
        var options = MockBackend.Options()
        options.demoForm = true
        let backend = MockBackend(options: options)
        let app = AppModel(backend: backend, isDev: true)
        await app.start()
        if var form = await backend.pendingForm() {
            form.initialValues = ["Name": "Ada"]
            app.formRequest = form
        }
        model = app
    }
}
