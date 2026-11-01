import KudzuCore
import SwiftUI

/// Date and time chips: format preset or custom ICU pattern, offset, locale and a live preview.
struct DateChipInspector: View {
    let tag: KudzuCore.Tag
    let onChange: (KudzuCore.Tag) -> Void

    @State private var presetID: String
    @State private var format: String
    @State private var offset: String
    @State private var locale: String

    private static let customID = "custom"
    private static let quickOffsets: [(String, String)] = [
        ("None", ""), ("Tomorrow", "+1d"), ("Yesterday", "-1d"), ("In a week", "+1w"), ("A week ago", "-1w"),
        ("Next month", "+1m"), ("In an hour", "+1h"),
    ]

    init(tag: KudzuCore.Tag, onChange: @escaping (KudzuCore.Tag) -> Void) {
        self.tag = tag
        self.onChange = onChange
        let format = tag.stringAttribute("format") ?? DateFormatPreset.defaultFormat(forTagNamed: tag.name)
        _format = State(initialValue: format)
        _presetID = State(initialValue: DateFormatPreset.matching(format)?.id ?? Self.customID)
        _offset = State(initialValue: tag.stringAttribute("offset") ?? "")
        _locale = State(initialValue: tag.stringAttribute("locale") ?? "")
    }

    private var isTime: Bool { tag.name == "time" }
    private var offsetIsValid: Bool { offset.isEmpty || DateOffset(offset) != nil }
    private var localeIsKnown: Bool { locale.isEmpty || Locale.availableIdentifiers.contains(locale) }

    /// Days in a plain `±Nd` offset, so the stepper can nudge it; other offsets start the stepper from zero.
    private var stepperDays: Binding<Int> {
        Binding {
            guard offset.hasSuffix("d"), let days = Int(offset.dropLast()), DateOffset(offset) != nil else { return 0 }
            return days
        } set: { days in
            offset = days == 0 ? "" : String(format: "%+dd", days)
        }
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            InspectorHeader(tag: tag, title: isTime ? "Time" : "Date",
                            subtitle: isTime ? "Inserts the current time." : "Inserts today's date, or one relative to it.")
            Form {
                Picker("Format", selection: $presetID) {
                    ForEach(DateFormatPreset.all) { preset in Text(preset.title).tag(preset.id) }
                    Divider()
                    Text("Custom…").tag(Self.customID)
                }
                if presetID == Self.customID {
                    LabeledContent("Pattern") {
                        TextField("Pattern", text: $format, prompt: Text("ICU, e.g. EEE d MMM"))
                            .labelsHidden()
                            .font(.body.monospaced())
                    }
                }
                LabeledContent("Offset") {
                    HStack(spacing: 4) {
                        TextField("Offset", text: $offset, prompt: Text("+3d, -1w"))
                            .labelsHidden()
                        Stepper("Days", value: stepperDays, in: -3650...3650).labelsHidden()
                        Menu {
                            ForEach(Self.quickOffsets, id: \.1) { title, value in Button(title) { offset = value } }
                        } label: { Image(systemName: "ellipsis.circle") }
                            .menuStyle(.borderlessButton)
                            .menuIndicator(.hidden)
                            .fixedSize()
                            .accessibilityLabel("Common offsets")
                    }
                }
                if !offsetIsValid {
                    ValidationMessage(text: "Use a sign, then amounts with d, w, m, y, h or min, e.g. +1w2d.")
                }
                TextField("Locale", text: $locale, prompt: Text("System (\(Locale.current.identifier))"))
                if !localeIsKnown {
                    ValidationMessage(text: "Unknown locale identifier; the system will fall back.")
                }
            }
            .formStyle(.columns)
            preview
        }
        .onChange(of: presetID) {
            if let preset = DateFormatPreset.all.first(where: { $0.id == presetID }) { format = preset.format }
        }
        .onChange(of: format) { commit() }
        .onChange(of: offset) { commit() }
        .onChange(of: locale) { commit() }
    }

    private var preview: some View {
        TimelineView(.everyMinute) { context in
            HStack {
                Text("Preview").foregroundStyle(.secondary)
                Spacer()
                Text(rendered(at: context.date))
                    .font(.body.monospacedDigit())
                    .textSelection(.enabled)
                    .lineLimit(1)
                    .truncationMode(.middle)
            }
            .padding(.horizontal, 10)
            .padding(.vertical, 7)
            .background(RoundedRectangle(cornerRadius: 6, style: .continuous).fill(.quaternary.opacity(0.6)))
            .accessibilityElement(children: .combine)
        }
    }

    private var editedTag: KudzuCore.Tag {
        let defaultFormat = DateFormatPreset.defaultFormat(forTagNamed: tag.name)
        return tag
            .setting("format", string: format == defaultFormat ? nil : format)
            .setting("offset", string: offsetIsValid ? offset : tag.stringAttribute("offset"))
            .setting("locale", string: locale.trimmingCharacters(in: .whitespaces))
    }

    private func rendered(at date: Date) -> String {
        let context = RenderContext(now: date, calendar: .current, timeZone: .current, locale: .current)
        let text = TemplateRenderer.render(editedTag.source, context: context).text
        return text.isEmpty ? "—" : text
    }

    private func commit() {
        guard !format.isEmpty else { return }
        onChange(editedTag)
    }
}
