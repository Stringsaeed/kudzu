import AppKit
import SwiftUI
import KudzuCore

/// A form that takes keyboard focus without activating Kudzu.
@MainActor
final class FormPanel: NSPanel {
    override var canBecomeKey: Bool { true }
    override var canBecomeMain: Bool { false }
    private var completion: (([String: String]?) -> Void)?
    private var monitor: Any?

    init() {
        super.init(contentRect: NSRect(x: 0, y: 0, width: 400, height: 460),
                   styleMask: [.titled, .nonactivatingPanel], backing: .buffered, defer: false)
        level = .floating
        hidesOnDeactivate = false
        isReleasedWhenClosed = false
        collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary]
    }

    func present(name: String, fields: [FormFieldSpec], values: [String: String],
                 preview: @escaping ([String: String]) -> String) async -> [String: String]? {
        let location = CaretLocator().locate()
        title = name
        let model = FormModel(fields: fields, values: values)
        contentView = NSHostingView(rootView: FormPanelView(model: model, preview: preview,
            submit: { [weak self] in self?.finish(model.values) }, cancel: { [weak self] in self?.finish(nil) }))
        let screen = NSScreen.screens.first { $0.frame.contains(location.rect.origin) } ?? NSScreen.main
        if let frame = screen?.visibleFrame {
            let size = self.frame.size
            var origin = NSPoint(x: location.rect.minX, y: location.rect.minY - size.height - 6)
            if origin.y < frame.minY { origin.y = location.rect.maxY + 6 }
            if location.source == .mouse {
                origin = NSPoint(x: frame.midX - size.width / 2, y: frame.midY - size.height / 2)
            }
            origin.x = min(max(origin.x, frame.minX), max(frame.minX, frame.maxX - size.width))
            origin.y = min(max(origin.y, frame.minY), max(frame.minY, frame.maxY - size.height))
            setFrameOrigin(origin)
        }
        monitor = NSEvent.addLocalMonitorForEvents(matching: .keyDown) { [weak self] event in
            guard let self, event.window === self else { return event }
            switch event.keyCode {
            case 53: self.finish(nil); return nil
            case 48:
                model.moveFocus(backward: event.modifierFlags.contains(.shift))
                return nil
            case 36, 76:
                let multiline = fields.first { $0.name == model.focus }?.kind == .multiline
                if !multiline || event.modifierFlags.contains(.command) {
                    self.finish(model.values)
                    return nil
                }
            default: break
            }
            return event
        }
        return await withCheckedContinuation { continuation in
            completion = { continuation.resume(returning: $0) }
            makeKeyAndOrderFront(nil)
        }
    }

    func cancel() { finish(nil) }

    private func finish(_ values: [String: String]?) {
        if let monitor { NSEvent.removeMonitor(monitor) }
        monitor = nil
        orderOut(nil)
        let callback = completion
        completion = nil
        callback?(values)
    }
}

@MainActor @Observable
private final class FormModel {
    let fields: [FormFieldSpec]
    var values: [String: String]
    var focus: String?

    init(fields: [FormFieldSpec], values: [String: String]) {
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

private struct FormPanelView: View {
    @Bindable var model: FormModel
    let preview: ([String: String]) -> String
    let submit: () -> Void
    let cancel: () -> Void
    @FocusState private var focus: String?

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            ScrollView {
                VStack(alignment: .leading, spacing: 12) {
                    ForEach(model.fields, id: \.name) { field in
                        VStack(alignment: .leading, spacing: 4) {
                            Text(field.name).font(.headline)
                            control(field).focused($focus, equals: field.name)
                        }
                    }
                    Divider()
                    Text("Preview").font(.headline)
                    Text(preview(model.values)).textSelection(.enabled)
                        .frame(maxWidth: .infinity, alignment: .leading)
                }.padding(2)
            }
            HStack {
                Button("Cancel", action: cancel)
                Spacer()
                Text("⌘↵ to insert").font(.caption).foregroundStyle(.secondary)
                Button("Insert", action: submit)
            }
        }
        .padding(16)
        .frame(width: 400, height: 460)
        .onAppear { focus = model.focus }
        .onChange(of: model.focus) { _, value in focus = value }
        .onChange(of: focus) { _, value in model.focus = value }
    }

    @ViewBuilder private func control(_ field: FormFieldSpec) -> some View {
        let value = Binding(get: { model.values[field.name] ?? "" }, set: { model.values[field.name] = $0 })
        switch field.kind {
        case .text: TextField(field.name, text: value).labelsHidden()
        case .multiline: TextEditor(text: value).frame(height: 80).border(.separator)
        case .choice:
            Picker(field.name, selection: value) {
                ForEach(Array(field.options.enumerated()), id: \.offset) { _, option in Text(option).tag(option) }
            }.labelsHidden()
        }
    }
}
