import KudzuCore
import KudzuDesign
import SwiftUI

/// The popover for a clicked chip (`ChipEditor` / `ChipForm` in tag-tools.tsx): a small form for
/// the tag, applied live.
struct ChipEditor: View {
    var tag: TagToken
    var snippets: [Snippet]
    var excludeID: UUID
    var change: (String) -> Void
    var remove: () -> Void
    var done: () -> Void
    @State private var draft: EditorTagDraft
    @State private var query = ""
    @State private var customFormat: Bool
    @State private var example = ""
    @State private var now = Date()
    @FocusState private var focused: String?
    @Environment(AppModel.self) private var model

    init(tag: TagToken, snippets: [Snippet], excludeID: UUID, change: @escaping (String) -> Void, remove: @escaping () -> Void, done: @escaping () -> Void) {
        self.tag = tag; self.snippets = snippets; self.excludeID = excludeID
        self.change = change; self.remove = remove; self.done = done
        let draft = EditorTagDraft(tag)
        _draft = State(initialValue: draft)
        let format = draft.string("format")
        _customFormat = State(initialValue: !format.isEmpty && !Template.dateFormatPresets.contains { $0.format == format })
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            HStack(spacing: 8) {
                Text(title).font(.system(size: 13, weight: .semibold))
                Spacer()
                MutedGhostButton(size: .iconXs, hoverTone: .destructive, action: remove) { Icon.trash2.icon() }
                    .accessibilityLabel("Remove from snippet")
            }.padding(.horizontal, 12).padding(.vertical, 10) // `px-3 py-2.5`
            Hairline(token: .border)
            VStack(alignment: .leading, spacing: 12) {
                if tag.verbatim { verbatim } else { fields }
            }.font(.system(size: 14)).padding(12)
            Hairline(token: .border)
            HStack(spacing: 12) {
                Text("Changes apply instantly")
                Spacer()
                HStack(spacing: 4) { Shortcut("enter"); Text("done") }
            }.font(.system(size: 11)).foregroundStyle(Color.kz.mutedForeground).padding(.horizontal, 12).padding(.vertical, 8)
        }
        .frame(width: 320)
        // Return closes the popover from anywhere but the options text area (the web's onKeyDown).
        .background {
            Button("Done", action: done).keyboardShortcut(.defaultAction)
                .opacity(0).frame(width: 0, height: 0).accessibilityHidden(true)
        }
        .onAppear { if draft.name == "field" || draft.name == "choice" { focused = "name" } }
        .onChange(of: customFormat) { _, custom in if custom { focused = "custom-format" } }
        .onChange(of: draft) { _, value in change(value.source) }
        .task(id: draft.source) {
            guard !tag.verbatim, draft.name == "date" || draft.name == "time" else { return }
            // Same backend renderer as the main preview: offsets, locale, clipboard and nesting
            // all contribute to the example, rather than approximating the template grammar.
            while !Task.isCancelled {
                let result = try? await model.backend.renderPreview(content: draft.source, snippetId: excludeID, formValues: [:], formatting: nil)
                guard !Task.isCancelled else { return }
                example = result?.text ?? ""
                now = Date()
                do { try await Task.sleep(for: .seconds(15)) } catch { return }
            }
        }
    }

    private var title: String {
        ["date": "Date", "time": "Time", "clipboard": "Clipboard", "cursor": "Cursor position", "snippet": "Nested snippet", "field": "Fill-in field", "choice": "Dropdown"][draft.name] ?? "Unrecognised tag"
    }

    private var verbatim: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack(alignment: .firstTextBaseline, spacing: 8) {
                Icon.triangleAlert.icon(14)
                Text("Kudzu doesn't understand this tag, so it will be typed exactly as written.").fixedSize(horizontal: false, vertical: true)
            }
            .font(.system(size: 12)).foregroundStyle(Color.kz.destructive)
            .padding(10).frame(maxWidth: .infinity, alignment: .leading)
            .background(Color.kz.mix(.destructive, alpha: 0.1), in: RoundedRectangle(cornerRadius: Radius.md))
            Text(tag.source).font(KZFonts.mono(size: 12)).textSelection(.enabled).fixedSize(horizontal: false, vertical: true)
                .padding(8).frame(maxWidth: .infinity, alignment: .leading)
                .background(Color.kz.mix(.foreground, alpha: 0.05), in: RoundedRectangle(cornerRadius: Radius.md))
        }
    }

    @ViewBuilder private var fields: some View {
        switch draft.name {
        case "date", "time": dateFields
        case "clipboard":
            switchRow("Trim whitespace", isOn: bool("trim"), detail: "Remove spaces and line breaks at both ends")
        case "cursor":
            Text("After the text is inserted, the caret jumps here — handy for filling in the middle of a template.")
                .font(.system(size: 12)).lineSpacing(5).foregroundStyle(Color.kz.mutedForeground).fixedSize(horizontal: false, vertical: true)
        case "field":
            field("Label", key: "name", hint: Text("Fields with the same label share one value."))
            field("Default value", key: "default", placeholder: "Optional")
            switchRow("Multiple lines", isOn: bool("multiline"))
        case "choice":
            field("Label", key: "name")
            ChipField("Options", hint: Text("One per line.")) {
                TextEditor(text: Binding(get: { Template.splitOptions(draft.string("options")).joined(separator: "\n") }, set: { draft.set("options", Template.joinOptions($0.components(separatedBy: "\n"))) }))
                    .font(KZFonts.mono(size: 12)).kzTextarea(minHeight: 80) // `min-h-20 font-mono text-xs`
            }
            ChipField("Default") {
                Picker("Default", selection: string("default")) {
                    Text("First option").tag("")
                    ForEach(Array(Template.splitOptions(draft.string("options")).filter { !$0.isEmpty }.enumerated()), id: \.offset) { _, value in Text(value).tag(value) }
                }.kzSelect()
            }
        case "snippet":
            PickList(query: $query, placeholder: "Find a snippet…", items: snippetItems, maxHeight: 224, initial: draft.string("id")) { item, _ in
                HStack(spacing: 8) {
                    Icon.fileText.icon(14).foregroundStyle(Color.kz.mutedForeground)
                    Text(snippets.first { $0.id.uuidString.lowercased() == item.id }?.name.nonEmpty ?? "Untitled").lineLimit(1)
                    Spacer(minLength: 0)
                    if item.id == draft.string("id") {
                        Circle().fill(Color.kz.iris).frame(width: 6, height: 6).accessibilityLabel("selected")
                        Icon.check.icon(16)
                    }
                }
            } empty: { Text("No snippets.") }
                .padding(-4) // `-m-1`
        default: Text(tag.source).font(KZFonts.mono(size: 12))
        }
    }

    private var snippetItems: [PickItem] {
        snippets.filter { $0.id != excludeID }.map { s in
            PickItem(id: s.id.uuidString.lowercased(), value: "\(s.name) \(s.triggers.joined(separator: " ")) \(s.id)", group: nil) { draft.set("id", s.id.uuidString.lowercased()) }
        }
    }

    private static let offsetPresets: [(value: String, label: String)] = [
        ("", "Today / now"), ("+1d", "Tomorrow"), ("-1d", "Yesterday"), ("+1w", "In a week"), ("+1m", "In a month"), ("+1h", "In an hour"),
    ]

    private var dateFields: some View {
        VStack(alignment: .leading, spacing: 12) {
            VStack(spacing: 0) {
                Text("Example").font(.system(size: 11)).tracking(0.275).textCase(.uppercase).foregroundStyle(Color.kz.faint)
                Text(example.isEmpty ? "—" : example).font(.system(size: 14, weight: .medium).monospacedDigit())
                    .lineLimit(1).truncationMode(.tail).textSelection(.enabled)
            }
            .multilineTextAlignment(.center)
            .frame(maxWidth: .infinity).padding(.horizontal, 12).padding(.vertical, 10)
            .background(Color.kz.mix(.foreground, alpha: 0.04), in: RoundedRectangle(cornerRadius: Radius.lg))
            Segmented("Kind", selection: $draft.name, options: [SegmentOption("Date", value: "date"), SegmentOption("Time", value: "time")])
            ChipField("Format") {
                Picker("Format", selection: Binding(get: { customFormat ? "__custom" : effectiveFormat }, set: { value in
                    customFormat = value == "__custom"
                    if !customFormat { draft.set("format", value) }
                })) {
                    ForEach(Template.dateFormatPresets, id: \.format) { preset in
                        Text("\(preset.label) — \(Template.formatICU(now, pattern: preset.format, locale: Locale.current.identifier, timeZone: .current))").tag(preset.format)
                    }
                    Text("Custom pattern…").tag("__custom")
                }.kzSelect()
                if customFormat {
                    TextField("Custom ICU pattern", text: Binding(get: { effectiveFormat }, set: { draft.set("format", $0) }), prompt: Text("EEEE, MMM d · h:mm a"))
                        .font(KZFonts.mono(size: 12)).kzInput()
                        .focused($focused, equals: "custom-format").onSubmit(done)
                }
            }
            ChipField("Shift", hint: Text("Combine units, e.g. ") + Text("+1w2d").font(KZFonts.mono(size: 11)) + Text(" or ") + Text("-30min").font(KZFonts.mono(size: 11)) + Text(".")) {
                FlowLayout(spacing: 4, lineSpacing: 4) {
                    ForEach(Self.offsetPresets, id: \.value) { preset in
                        OffsetPreset(label: preset.label, on: draft.string("offset") == preset.value) { draft.set("offset", preset.value) }
                    }
                }
                TextField("Custom shift", text: string("offset"), prompt: Text("+3d"))
                    .font(KZFonts.mono(size: 12)).kzInput().focused($focused, equals: "offset").onSubmit(done)
            }
            ChipField("Language") {
                TextField("Language", text: string("locale"), prompt: Text("System default (e.g. en_US, fr_FR)"))
                    .font(KZFonts.mono(size: 12)).kzInput().focused($focused, equals: "locale").onSubmit(done)
            }
        }
    }
    private var effectiveFormat: String { draft.string("format").isEmpty ? (draft.name == "date" ? "yyyy-MM-dd" : "HH:mm") : draft.string("format") }
    private func string(_ key: String) -> Binding<String> { Binding(get: { draft.string(key) }, set: { draft.set(key, $0) }) }
    private func bool(_ key: String) -> Binding<Bool> { Binding(get: { draft.bool(key) }, set: { draft.set(key, $0) }) }
    private func field(_ label: String, key: String, placeholder: String = "", hint: Text? = nil) -> some View {
        ChipField(label, hint: hint) {
            TextField(label, text: string(key), prompt: placeholder.isEmpty ? nil : Text(placeholder))
                .kzInput().focused($focused, equals: key).onSubmit(done)
        }
    }
    /// A labelled switch row, label leading and switch trailing (`justify-between`).
    private func switchRow(_ label: String, isOn: Binding<Bool>, detail: String? = nil) -> some View {
        HStack(spacing: 12) {
            VStack(alignment: .leading, spacing: 0) {
                Text(label)
                if let detail { Text(detail).font(.system(size: 12)).foregroundStyle(Color.kz.mutedForeground).fixedSize(horizontal: false, vertical: true) }
            }
            Spacer(minLength: 0)
            Toggle(label, isOn: isOn).labelsHidden().kzSwitch()
        }
    }
}

/// `Field` in tag-tools.tsx: a 12 pt muted label, the control, and an 11 pt faint hint.
private struct ChipField<Content: View>: View {
    var label: String
    var hint: Text?
    var content: Content

    init(_ label: String, hint: Text? = nil, @ViewBuilder content: () -> Content) {
        self.label = label
        self.hint = hint
        self.content = content()
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            Text(label).font(.system(size: 12, weight: .medium)).foregroundStyle(Color.kz.mutedForeground)
            content
            if let hint { hint.font(.system(size: 11)).foregroundStyle(Color.kz.faint).fixedSize(horizontal: false, vertical: true) }
        }
    }
}

/// A "Shift" preset: a small raised key that sinks into iris when chosen (`aria-pressed`).
private struct OffsetPreset: View {
    var label: String
    var on: Bool
    var action: () -> Void
    @State private var hovered = false

    var body: some View {
        Button(action: action) {
            Text(label).font(.system(size: 12))
                .foregroundStyle(on ? Color.kz.irisForeground : hovered ? Color.kz.foreground : Color.kz.mutedForeground)
                .padding(.horizontal, 8).frame(height: 24)
                .depth(on ? .pressed : hovered ? .raisedHover : .raised, fill: on ? Color.kz.irisSoft : Color.kz.surfaceRaised,
                       in: RoundedRectangle(cornerRadius: Radius.md))
        }
        .buttonStyle(.press)
        .onHover { hovered = $0 }
        .kzAnimation(.kz.hover, value: hovered)
        .accessibilityAddTraits(on ? .isSelected : [])
    }
}

private extension String { var nonEmpty: String? { isEmpty ? nil : self } }
