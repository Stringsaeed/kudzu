import KudzuCore
import KudzuDesign
import SwiftUI

struct EditorInsertChoice {
    var text: String
    var configure = false
}

struct EditorVariable: Identifiable {
    var id: String
    var label: String
    var hint: String
    var icon: Image
    var keywords: String
    var choice: EditorInsertChoice

    static let all: [EditorVariable] = [
        .init(id: "date", label: "Date", hint: "Today, or any day relative to it", icon: Icon.calendarDays, keywords: "today day calendar", choice: .init(text: Template.buildTag("date", attrs: [.init(name: "format", value: .string("MMMM d, yyyy"))]))),
        .init(id: "time", label: "Time", hint: "The current time", icon: Icon.clock, keywords: "clock hour now", choice: .init(text: Template.buildTag("time", attrs: [.init(name: "format", value: .string("h:mm a"))]))),
        .init(id: "clipboard", label: "Clipboard", hint: "Whatever you last copied", icon: Icon.clipboard, keywords: "paste copy", choice: .init(text: Template.buildTag("clipboard"))),
        .init(id: "cursor", label: "Cursor position", hint: "Where the caret lands after expanding", icon: Icon.textCursor, keywords: "caret", choice: .init(text: Template.buildTag("cursor"))),
        .init(id: "field", label: "Fill-in field", hint: "Asks you for text each time", icon: Icon.textCursorInput, keywords: "form input ask prompt variable", choice: .init(text: Template.buildTag("field", attrs: [.init(name: "name", value: .string("Name"))]), configure: true)),
        .init(id: "choice", label: "Dropdown", hint: "Pick from a list each time", icon: Icon.listChecks, keywords: "form select list options", choice: .init(text: Template.buildTag("choice", attrs: [.init(name: "name", value: .string("Option")), .init(name: "options", value: .string("Yes|No"))]), configure: true))
    ]
}

struct InsertMenu: View {
    var snippets: [Snippet]
    var excludeID: UUID
    var pick: (EditorInsertChoice) -> Void
    @State private var query = ""

    private var items: [PickItem] {
        EditorVariable.all.map { v in
            PickItem(id: v.id, value: "\(v.label) \(v.keywords)", group: "Variables") { pick(v.choice) }
        } + snippets.filter { $0.id != excludeID }.prefix(400).map { s in
            PickItem(id: s.id.uuidString, value: "snippet \(s.name) \(s.triggers.joined(separator: " ")) \(s.id)", group: "Insert another snippet") {
                pick(.init(text: Template.buildTag("snippet", attrs: [.init(name: "id", value: .string(s.id.uuidString.lowercased()))])))
            }
        }
    }

    var body: some View {
        VStack(spacing: 0) {
            PickList(query: $query, placeholder: "Insert a variable or snippet…", items: items, maxHeight: 320) { item, _ in
                HStack(spacing: 10) {
                    if let v = EditorVariable.all.first(where: { $0.id == item.id }) {
                        v.icon.icon(14).frame(width: 28, height: 28)
                            .background(Color.kz.mix(.foreground, alpha: 0.06), in: RoundedRectangle(cornerRadius: Radius.md))
                        VStack(alignment: .leading, spacing: 2) {
                            Text(v.label).fontWeight(.medium)
                            Text(v.hint).font(.system(size: 12)).foregroundStyle(Color.kz.mutedForeground)
                        }
                    } else if let s = snippets.first(where: { $0.id.uuidString == item.id }) {
                        Icon.cornerDownRight.icon(14).foregroundStyle(Color.kz.mutedForeground)
                        Text(s.name.isEmpty ? "Untitled" : s.name).lineLimit(1)
                        Spacer(minLength: 0)
                        if let trigger = s.triggers.first { TriggerChip(trigger, muted: true) }
                    }
                }
            } empty: { Text("Nothing matches.") }
            Hairline(token: .border)
            HStack(spacing: 12) {
                HStack(spacing: 4) { Shortcut("enter"); Text("insert") }
                HStack(spacing: 4) { Shortcut("escape"); Text("close") }
                Spacer(minLength: 0)
            }
            .font(.system(size: 11))
            .foregroundStyle(Color.kz.mutedForeground)
            .padding(.horizontal, 12)
            .padding(.vertical, 8)
        }.frame(width: 320)
    }
}
