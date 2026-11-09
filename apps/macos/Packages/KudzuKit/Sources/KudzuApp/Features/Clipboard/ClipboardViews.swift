// The ⌃⌥V clipboard picker panel (`clipboard-picker.tsx`): a large search field with the kind
// filters, the newest copies on the left (⌘1–9 light up while ⌘ is held), the preview on the
// right, and a footer of key hints that briefly shows what just happened. Focus stays in the
// search field; arrows, Return, Tab and the ⌘ chords all work from there.

import AppKit
import KudzuCore
import KudzuDesign
import SwiftUI

/// Contents of the clipboard panel. Window presentation remains the shell's responsibility.
public struct ClipboardPickerView: View {
    public init() {}
    public var body: some View { ClipboardPicker() }
}

private struct ClipboardPicker: View {
    @Environment(AppModel.self) private var model
    @State private var query = ""
    @State private var filter = "all"
    @State private var activeId: UUID?
    @State private var showNumbers = false
    @State private var flash: String?
    @State private var flashTask: Task<Void, Never>?
    @State private var undo: [ClipEntry]?
    @State private var modifierMonitor = CommandKeyMonitor()
    @FocusState private var searchFocused: Bool

    /// `PICKER_FILTERS`.
    static let filters = ["all", "pinned", "text", "image", "link", "file"]

    private var terms: [String] { Clipboard.parseQuery(query).terms }
    private var built: (rows: [Clipboard.ClipRow], order: [ClipEntry]) {
        let results = Clipboard.searchClips(model.clipboard.entries, filter: filter, query: query)
        // Most recent first: what you just copied is one keystroke away.
        return Clipboard.buildRows(results, pinnedFirst: false, sections: query.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
    }
    private var pasteLabel: String {
        guard model.clipboard.canPaste else { return "Copy" }
        return model.clipboardTarget.map { "Paste into \($0.name)" } ?? "Paste"
    }

    var body: some View {
        let built = built
        let order = built.order
        let active = activeId.flatMap { id in order.first { $0.id == id } } ?? order.first
        VStack(spacing: 0) {
            searchBar
            HStack(spacing: 0) {
                results(rows: built.rows, order: order, active: active)
                    .frame(maxWidth: .infinity)
                Hairline(.vertical)
                preview(active)
                    .frame(maxWidth: .infinity)
            }
            footer(active)
        }
        .background(Color.kz.popover.opacity(0.55))
        .foregroundStyle(Color.kz.popoverForeground)
        .onKeyPress(phases: .down) { press in key(press, order: order, active: active) }
        .background { chords(order: order, active: active) }
        .background {
            PickerWindowObserver(onShow: reset, onBlur: {
                showNumbers = false
                Task { try? await model.backend.hideClipboard() }
            })
        }
        .task { await model.refreshClipboard(); reset() }
        .onAppear { modifierMonitor.install { showNumbers = $0 } }
        .onDisappear { modifierMonitor.remove() }
        .accessibilityElement(children: .contain)
        .accessibilityLabel("Clipboard history")
    }

    // MARK: Search

    private var searchBar: some View {
        HStack(spacing: 12) {
            Icon.search.icon(18).foregroundStyle(Color.kz.mutedForeground)
            TextField("Search clipboard history…", text: $query)
                .textFieldStyle(.plain)
                .font(.system(size: 17))
                .tracking(-0.17)
                .focused($searchFocused)
                .accessibilityLabel("Search clipboard history")
                .onChange(of: query) { _, _ in activeId = nil }
            PickerFilters(value: $filter) {
                activeId = nil
                searchFocused = true
            }
        }
        .padding(.horizontal, 16)
        .frame(height: 58)
        .overlay(alignment: .bottom) { Hairline() }
    }

    // MARK: Results

    private func results(rows: [Clipboard.ClipRow], order: [ClipEntry], active: ClipEntry?) -> some View {
        ScrollViewReader { proxy in
            ScrollView {
                if rows.isEmpty {
                    empty.frame(minHeight: 320)
                } else {
                    LazyVStack(spacing: 0) {
                        ForEach(rows, id: \.pickerKey) { row in
                            switch row {
                            case let .header(_, label, count):
                                ClipSectionHeader(label: label, count: count, density: .compact)
                            case let .entry(_, entry, index, dated):
                                ClipRowView(
                                    entry: entry,
                                    dated: dated,
                                    terms: terms,
                                    density: .compact,
                                    active: entry.id == active?.id,
                                    number: index < 9 ? index + 1 : nil,
                                    numbersAccent: showNumbers
                                )
                                .padding(.horizontal, 6)
                                .padding(.vertical, 1)
                                .id(entry.id)
                                .onTapGesture(count: 2) { paste(entry, shift: false) }
                                .onTapGesture { activeId = entry.id }
                            }
                        }
                    }
                    .padding(.top, 8)
                    .padding(.bottom, 8)
                }
            }
            .scrollIndicators(.automatic)
            .onChange(of: active?.id) { _, id in
                guard let id else { return }
                proxy.scrollTo(id)
            }
        }
        .accessibilityLabel(query.isEmpty ? "Recent copies" : "Matches")
    }

    private var empty: some View {
        let entries = model.clipboard.entries
        let enabled = model.settings.clipboardEnabled
        return VStack(spacing: 8) {
            Icon.clipboardList.icon(20).foregroundStyle(Color.kz.faint)
            Text(!enabled ? "Clipboard history is paused." : entries.isEmpty ? "Nothing copied yet." : !query.isEmpty ? "Nothing matches “\(query)”." : "Nothing here yet.")
                .font(.system(size: 14))
                .foregroundStyle(Color.kz.mutedForeground)
            Text(!enabled ? "Turn it on in Kudzu's settings." : entries.isEmpty ? "Copy something in any app and it shows up here." : "Try another filter, or app: and is: to narrow it down.")
                .font(.system(size: 12))
                .foregroundStyle(Color.kz.faint)
        }
        .multilineTextAlignment(.center)
        .padding(.horizontal, 32)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    // MARK: Preview

    @ViewBuilder private func preview(_ active: ClipEntry?) -> some View {
        VStack(alignment: .leading, spacing: 0) {
            if let active {
                ClipHeading(entry: active) { setNotSecret(active) }
                    .padding(.horizontal, 20)
                    .padding(.top, 14)
                    .padding(.bottom, 10)
                ScrollView {
                    ClipContentView(entry: active, terms: terms)
                        .padding(.horizontal, 20)
                        .padding(.bottom, 16)
                        .frame(maxWidth: .infinity, alignment: .leading)
                }
                .id(active.id)
                ClipDetails(entry: active, compact: true)
                    .padding(.horizontal, 20)
                    .padding(.vertical, 12)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .overlay(alignment: .top) { Hairline() }
            } else {
                Color.clear
            }
        }
        .frame(maxHeight: .infinity)
        .background(Color.kz.foreground.opacity(0.015))
        .accessibilityElement(children: .contain)
        .accessibilityLabel("Preview")
    }

    // MARK: Footer

    private func footer(_ active: ClipEntry?) -> some View {
        HStack(spacing: 16) {
            HStack(spacing: 6) {
                Icon.cornerDownLeft.icon(12)
                    .foregroundStyle(Color.kz.irisContrast)
                    .frame(width: 18, height: 18)
                    .depth(.accent, fill: Color.kz.iris, in: RoundedRectangle(cornerRadius: 5))
                    .sheen(.strong, in: RoundedRectangle(cornerRadius: 5))
                Text(flash ?? pasteLabel)
                    .foregroundStyle(flash == nil ? Color.kz.mutedForeground : Color.kz.foreground)
                    .lineLimit(1)
            }
            .accessibilityElement(children: .combine)
            if flash == nil {
                if model.clipboard.canPaste { hint("mod+enter", "Copy") }
                if active?.rich == true { hint("shift+enter", model.settings.clipboardPastePlain ? "Formatted" : "Plain text") }
                hint("mod+p", active?.pinned == true ? "Unpin" : "Pin")
                hint("mod+backspace", "Delete")
            }
            Spacer(minLength: 8)
            hint("tab", "Filter")
            Shortcut("escape")
        }
        .font(.system(size: 11.5))
        .foregroundStyle(Color.kz.mutedForeground)
        .padding(.horizontal, 16)
        .frame(height: 40)
        .overlay(alignment: .top) { Hairline() }
    }

    private func hint(_ combo: String, _ label: String) -> some View {
        HStack(spacing: 6) { Shortcut(combo); Text(label) }.fixedSize()
    }

    // MARK: Keys

    /// ⌘ chords as window key equivalents, so the search field and the menu bar don't take them.
    private func chords(order: [ClipEntry], active: ClipEntry?) -> some View {
        Group {
            ForEach(1..<10, id: \.self) { number in
                Button("Paste \(number)") { if order.count >= number { paste(order[number - 1], shift: false) } }
                    .keyboardShortcut(KeyEquivalent(Character("\(number)")), modifiers: .command)
                Button("Paste \(number) as plain text") { if order.count >= number { paste(order[number - 1], shift: true) } }
                    .keyboardShortcut(KeyEquivalent(Character("\(number)")), modifiers: [.command, .shift])
            }
            Button("Copy") { copyOnly(active, shift: false) }.keyboardShortcut(.return, modifiers: .command)
            Button("Copy as plain text") { copyOnly(active, shift: true) }.keyboardShortcut(.return, modifiers: [.command, .shift])
            Button("Paste as plain text") { paste(active, shift: true) }.keyboardShortcut(.return, modifiers: .shift)
            Button("Pin") { togglePin(active) }.keyboardShortcut("p", modifiers: .command)
            Button("Delete") { delete(active, order: order) }.keyboardShortcut(.delete, modifiers: .command)
            Button("Undo delete") { restore() }.keyboardShortcut("z", modifiers: .command)
            Button("Save as snippet") {
                guard let active, Clipboard.hasText(active) else { return }
                Task { try? await model.backend.clipboardToSnippet(id: active.id) }
            }
            .keyboardShortcut("s", modifiers: .command)
            Button("Open") {
                guard let active, active.kind == .link || active.kind == .file else { return }
                Task {
                    try? await model.openClip(active)
                    try? await model.backend.hideClipboard()
                }
            }
            .keyboardShortcut("o", modifiers: .command)
            Button("Clipboard settings") {
                Task { try? await model.backend.showMainView(.settings(section: .clipboard)) }
            }
            .keyboardShortcut(",", modifiers: .command)
        }
        .opacity(0)
        .frame(width: 0, height: 0)
        .accessibilityHidden(true)
    }

    private func key(_ press: KeyPress, order: [ClipEntry], active: ClipEntry?) -> KeyPress.Result {
        let control = press.modifiers.contains(.control) && press.modifiers.isDisjoint(with: [.command, .option])
        switch press.key {
        case .downArrow: move(1, order: order, active: active); return .handled
        case .upArrow: move(-1, order: order, active: active); return .handled
        case .pageDown: move(7, order: order, active: active); return .handled
        case .pageUp: move(-7, order: order, active: active); return .handled
        case .return:
            guard !press.modifiers.contains(.command) else { return .ignored }
            paste(active, shift: press.modifiers.contains(.shift))
            return .handled
        case .tab:
            let index = Self.filters.firstIndex(of: filter) ?? 0
            let step = press.modifiers.contains(.shift) ? -1 : 1
            filter = Self.filters[(index + step + Self.filters.count) % Self.filters.count]
            activeId = nil
            return .handled
        case .escape:
            if query.isEmpty { Task { try? await model.backend.hideClipboard() } } else { query = "" }
            return .handled
        default:
            break
        }
        if control && press.characters.lowercased() == "n" { move(1, order: order, active: active); return .handled }
        if control && press.characters.lowercased() == "p" { move(-1, order: order, active: active); return .handled }
        return .ignored
    }

    private func move(_ delta: Int, order: [ClipEntry], active: ClipEntry?) {
        guard !order.isEmpty else { return }
        let current = active.flatMap { a in order.firstIndex { $0.id == a.id } } ?? 0
        activeId = order[min(order.count - 1, max(0, current + delta))].id
    }

    // MARK: Actions

    private func reset() {
        query = ""
        filter = "all"
        activeId = nil
        flash = nil
        undo = nil
        searchFocused = true
    }

    /// Shows `text` in the footer in place of the paste hint for `seconds`.
    private func say(_ text: String, seconds: Double = 1.6) {
        flashTask?.cancel()
        flash = text
        flashTask = Task {
            try? await Task.sleep(for: .seconds(seconds))
            if !Task.isCancelled { flash = nil }
        }
    }

    private func plain(_ entry: ClipEntry, shift: Bool) -> Bool { entry.rich && model.settings.clipboardPastePlain != shift }

    private func paste(_ entry: ClipEntry?, shift: Bool) {
        guard let entry else { return }
        Task {
            do {
                if try await model.pasteClip(entry, plain: plain(entry, shift: shift)) == .copied {
                    say("Copied. To paste directly, allow Kudzu in Accessibility settings.", seconds: 2.6)
                    try? await Task.sleep(for: .milliseconds(1800))
                    try? await model.backend.hideClipboard()
                }
            } catch {
                say(error.localizedDescription, seconds: 3)
            }
        }
    }

    private func copyOnly(_ entry: ClipEntry?, shift: Bool) {
        guard let entry else { return }
        Task {
            try? await model.copyClip(entry, plain: plain(entry, shift: shift))
            say("Copied")
            try? await Task.sleep(for: .milliseconds(420))
            try? await model.backend.hideClipboard()
        }
    }

    private func togglePin(_ entry: ClipEntry?) {
        guard let entry, let pinned = model.togglePinned([entry]) else { return }
        say(pinned ? "Pinned. It won't expire." : "Unpinned")
    }

    private func delete(_ entry: ClipEntry?, order: [ClipEntry]) {
        guard let entry, let index = order.firstIndex(where: { $0.id == entry.id }) else { return }
        if index + 1 < order.count { activeId = order[index + 1].id } else if index > 0 { activeId = order[index - 1].id }
        say("Deleted. ⌘Z to undo", seconds: 4)
        Task { undo = try? await model.removeClips([entry.id]) }
    }

    private func restore() {
        guard let entries = undo, query.isEmpty else { return }
        undo = nil
        Task { try? await model.restoreClips(entries) }
        say("Restored")
    }

    private func setNotSecret(_ entry: ClipEntry) {
        _ = model.markClipSecret(entry, secret: false)
        say("Marked as not a secret")
    }
}

/// The kind filters as a chip row with a sliding pill (`ClipFilters` in the picker): not in the
/// tab order, since Tab cycles them and focus stays in the search field.
private struct PickerFilters: View {
    @Binding var value: String
    var onChange: () -> Void
    @Namespace private var pill

    var body: some View {
        HStack(spacing: 4) {
            ForEach(Clipboard.FILTERS.filter { ClipboardPicker.filters.contains($0.value) }, id: \.value) { option in
                FilterChip(label: option.label, selected: option.value == value, namespace: pill) {
                    value = option.value
                    onChange()
                }
            }
        }
        .padding(2)
        .depth(.well, fill: Color.kz.foreground.opacity(0.035), in: Capsule())
        .fixedSize()
        .accessibilityElement(children: .contain)
        .accessibilityLabel("Show")
    }
}

private struct FilterChip: View {
    let label: String
    let selected: Bool
    let namespace: Namespace.ID
    let action: () -> Void
    @State private var hovered = false

    var body: some View {
        Button(action: action) {
            Text(label)
                .font(.system(size: 11.5, weight: .medium))
                .foregroundStyle(selected || hovered ? Color.kz.foreground : Color.kz.mutedForeground)
                .padding(.horizontal, 10)
                .frame(height: 24)
                .background {
                    if hovered && !selected { Capsule().fill(Color.kz.foreground.opacity(0.05)) }
                }
                .contentShape(Capsule())
        }
        .buttonStyle(.press)
        .focusable(false)
        .selectionPill(selected, in: namespace, style: SelectionPillStyle(radius: 12))
        .onHover { hovered = $0 }
        .animation(.kz.colors, value: hovered)
        .accessibilityAddTraits(selected ? .isSelected : [])
    }
}

private extension Clipboard.ClipRow {
    var pickerKey: String {
        switch self {
        case let .header(key, _, _), let .entry(key, _, _, _): key
        }
    }
}

/// Resets the picker each time its panel becomes key, and hides it when it loses focus.
private struct PickerWindowObserver: NSViewRepresentable {
    let onShow: () -> Void
    let onBlur: () -> Void
    func makeNSView(context: Context) -> ObserverView { let view = ObserverView(); view.onShow = onShow; view.onBlur = onBlur; return view }
    func updateNSView(_ view: ObserverView, context: Context) { view.onShow = onShow; view.onBlur = onBlur }
    final class ObserverView: NSView {
        var onShow: () -> Void = {}
        var onBlur: () -> Void = {}
        override func viewDidMoveToWindow() {
            super.viewDidMoveToWindow()
            NotificationCenter.default.removeObserver(self)
            guard let window else { return }
            NotificationCenter.default.addObserver(self, selector: #selector(shown), name: NSWindow.didBecomeKeyNotification, object: window)
            NotificationCenter.default.addObserver(self, selector: #selector(blurred), name: NSWindow.didResignKeyNotification, object: window)
        }
        @objc private func shown() { onShow() }
        @objc private func blurred() { onBlur() }
        deinit { NotificationCenter.default.removeObserver(self) }
    }
}
