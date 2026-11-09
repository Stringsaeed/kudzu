// The bar above the content editor (formatting-toolbar.tsx): the Plain text / Formatted selector
// and, in formatted mode, bold, italic, underline, link, image, lists and Remove formatting.
// Native pieces only: a segmented `Picker`, `Button`s in Kudzu's ghost style with help tags, and
// a `.popover` for the link address.

import KudzuCore
import KudzuDesign
import SwiftUI

public struct FormattingToolbar: View {
    var handle: TemplateEditorHandle

    public init(handle: TemplateEditorHandle) { self.handle = handle }

    public var body: some View {
        WrappingHStack(spacing: 2, lineSpacing: 2) {
            Segmented("Snippet format", selection: Binding(
                get: { handle.isFormatted },
                set: { formatted in
                    guard !handle.isComposing, formatted != handle.isFormatted else { return }
                    handle.setFormatted(formatted)
                }
            ), options: [
                SegmentOption("Plain text", value: false),
                SegmentOption("Formatted", value: true),
            ])
            .padding(.trailing, 8)

            if handle.isFormatted {
                StyleButton(handle: handle, kind: "bold", label: "Bold", icon: Icon.bold, shortcut: "mod+b", style: .bold)
                StyleButton(handle: handle, kind: "italic", label: "Italic", icon: Icon.italic, shortcut: "mod+i", style: .italic)
                StyleButton(handle: handle, kind: "underline", label: "Underline", icon: Icon.underline, shortcut: "mod+u", style: .underline)
                LinkButton(handle: handle)
                ImageTools(handle: handle)
                Rectangle().fill(Color.kz.border).frame(width: 1, height: 16).padding(.horizontal, 4)
                ToolButton(label: "Bulleted list", icon: Icon.list) { handle.toggleList(ordered: false) }
                ToolButton(label: "Numbered list", icon: Icon.listOrdered) { handle.toggleList(ordered: true) }
                ToolButton(label: "Remove formatting", icon: Icon.removeFormatting) { handle.changeFormat(nil) }
                    .disabled(!handle.hasSelection)
            }
        }
        .padding(.horizontal, 8)
        .padding(.vertical, 6)
        .frame(maxWidth: .infinity, alignment: .leading)
        .overlay(alignment: .bottom) { Rectangle().fill(Color.kz.hairline).frame(height: 1) }
        .accessibilityElement(children: .contain)
        .accessibilityLabel("Text formatting")
    }
}

// MARK: Buttons

/// A ghost icon button; muted unless pressed (`aria-pressed` → accent background).
struct ToolButton: View {
    var label: String
    var icon: Image
    var shortcut: String?
    var active = false
    var action: () -> Void

    var body: some View {
        Button(action: action) {
            icon.icon(16).foregroundStyle(active ? Color.kz.accentForeground : Color.kz.mutedForeground)
        }
        .buttonStyle(.kz(.ghost, size: .iconSm, expanded: active))
        .kzHelp(label, shortcut: shortcut)
        .accessibilityLabel(label)
        .accessibilityAddTraits(active ? .isSelected : [])
    }
}

struct StyleButton: View {
    var handle: TemplateEditorHandle
    var kind: String
    var label: String
    var icon: Image
    var shortcut: String
    var style: FormatStyle

    var body: some View {
        ToolButton(label: label, icon: icon, shortcut: shortcut, active: handle.isActive(kind)) { handle.changeFormat(style) }
            .disabled(!handle.hasSelection)
    }
}

/// Link: a popover with the address, validated like `safeLink`.
struct LinkButton: View {
    var handle: TemplateEditorHandle
    @State private var open = false
    @State private var url = ""
    @State private var error = ""
    @FocusState private var fieldFocused: Bool

    var body: some View {
        ToolButton(label: "Add or edit link", icon: Icon.link, active: handle.isActive("link") || open) {
            url = handle.linkAtSelection ?? ""
            error = ""
            open = true
        }
        .disabled(!handle.hasSelection)
        .kzPopover(isPresented: $open, width: 288) {
            Text("Link address").font(.system(size: 12, weight: .medium))
            TextField("https://example.com", text: $url)
                .kzInput(invalid: !error.isEmpty)
                .focused($fieldFocused)
                .onSubmit(apply)
                .accessibilityLabel("Link address")
            if !error.isEmpty {
                Text(error).font(.system(size: 12)).foregroundStyle(Color.kz.destructive)
            }
            Button("Apply link", action: apply)
                .buttonStyle(.kz(size: .sm, expanded: false))
                .keyboardShortcut(.defaultAction)
                .frame(maxWidth: .infinity, alignment: .leading)
        }
        .onChange(of: open) { _, isOpen in
            if isOpen { fieldFocused = true } else { handle.focus() }
        }
    }

    private func apply() {
        let value = url.trimmingCharacters(in: .whitespacesAndNewlines)
        guard EditorLinkRules.safeLink(value) else {
            error = "Use an https://, http://, or mailto: link."
            return
        }
        handle.applyLink(value)
        open = false
    }
}

// MARK: Wrapping row

/// `flex flex-wrap items-center`: children left to right, wrapping onto new rows.
struct WrappingHStack: Layout {
    var spacing: CGFloat
    var lineSpacing: CGFloat

    func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) -> CGSize {
        let rows = arrange(width: proposal.width ?? .infinity, subviews: subviews)
        let width = rows.map(\.width).max() ?? 0
        let height = rows.map(\.height).reduce(0, +) + lineSpacing * CGFloat(max(0, rows.count - 1))
        return CGSize(width: proposal.width ?? width, height: height)
    }

    func placeSubviews(in bounds: CGRect, proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) {
        var y = bounds.minY
        for row in arrange(width: bounds.width, subviews: subviews) {
            var x = bounds.minX
            for index in row.indices {
                let size = subviews[index].sizeThatFits(.unspecified)
                subviews[index].place(at: CGPoint(x: x, y: y + (row.height - size.height) / 2), proposal: ProposedViewSize(size))
                x += size.width + spacing
            }
            y += row.height + lineSpacing
        }
    }

    private struct Row { var indices: [Int] = []; var width: CGFloat = 0; var height: CGFloat = 0 }

    private func arrange(width: CGFloat, subviews: Subviews) -> [Row] {
        var rows: [Row] = [Row()]
        for index in subviews.indices {
            let size = subviews[index].sizeThatFits(.unspecified)
            let needed = (rows[rows.count - 1].indices.isEmpty ? 0 : spacing) + size.width
            if !rows[rows.count - 1].indices.isEmpty, rows[rows.count - 1].width + needed > width { rows.append(Row()) }
            let gap = rows[rows.count - 1].indices.isEmpty ? 0 : spacing
            rows[rows.count - 1].indices.append(index)
            rows[rows.count - 1].width += gap + size.width
            rows[rows.count - 1].height = max(rows[rows.count - 1].height, size.height)
        }
        return rows
    }
}
