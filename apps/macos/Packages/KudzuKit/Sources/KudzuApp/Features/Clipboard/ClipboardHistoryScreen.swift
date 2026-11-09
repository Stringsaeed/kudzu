// Clipboard history in the main window (`clipboard-view.tsx`): the list column (header with the
// recording toggle and options, search, day-grouped rows, selection bar or retention footer) and
// the preview column (breadcrumbs, heading, content, facts, and the action bar). The kind filter
// comes from the sidebar's Clipboard section. The native `List` owns selection, shift/⌘-click and
// arrow keys; Return, Space, Delete, J/K, `/` and type-to-search are added here.

import AppKit
import KudzuCore
import KudzuDesign
import SwiftUI

/// Clipboard history for the main window. The host supplies the shared app model.
public struct ClipboardHistoryView: View {
    private let filter: ClipFilter?
    public init(filter: ClipFilter? = nil) { self.filter = filter }
    public var body: some View { ClipboardHistoryScreen(filter: filter?.rawValue ?? "all") }
}

private struct ClipboardHistoryScreen: View {
    let filter: String
    @Environment(AppModel.self) private var model
    @Environment(LibraryFocus.self) private var focus
    @State private var query = ""
    @State private var selection: Set<UUID> = []
    @State private var activeId: UUID?
    @State private var lastIndex = 0
    @State private var quickLook = false
    @State private var clearOpen = false
    @State private var clearPinned = false
    @FocusState private var field: Field?

    private enum Field: Hashable { case search, list }

    // MARK: Data

    private var terms: [String] { Clipboard.parseQuery(query).terms }
    private var results: [ClipEntry] { Clipboard.searchClips(model.clipboard.entries, filter: filter, query: query) }
    private var built: (rows: [Clipboard.ClipRow], order: [ClipEntry]) {
        let pinnedFirst = query.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty && filter != "pinned"
        return Clipboard.buildRows(results, pinnedFirst: pinnedFirst, sections: true)
    }
    private var filterLabel: String { Clipboard.FILTERS.first { $0.value == filter }?.label ?? "All" }
    private var enabled: Bool { model.settings.clipboardEnabled }
    private var retention: Int { Int(model.settings.clipboardRetentionDays) }

    var body: some View {
        let built = built
        let order = built.order
        let active = activeId.flatMap { id in order.first { $0.id == id } }
        GeometryReader { geo in
            HStack(spacing: 0) {
                listColumn(rows: built.rows, order: order, active: active)
                    // `w-[clamp(300px,30vw,372px)]`: 30% of the window is about 37.5% of the card.
                    .frame(width: min(372, max(300, geo.size.width * 0.375)))
                Hairline(.vertical)
                previewColumn(active)
            }
        }
        .onChange(of: filter) { _, _ in selection = active.map { [$0.id] } ?? [] }
        .onChange(of: order.map(\.id), initial: true) { _, ids in keepActive(ids) }
        .onChange(of: focus.request?.serial) { _, _ in followFocus() }
        .task { await model.refreshClipboard() }
        .background { shortcuts(order: order, active: active) }
        .sheet(isPresented: $quickLook) {
            if let active { QuickLookSheet(entry: active, terms: terms, move: { move($0, order: order) }) }
        }
        .sheet(isPresented: $clearOpen) { clearDialog }
    }

    // MARK: List column

    private func listColumn(rows: [Clipboard.ClipRow], order: [ClipEntry], active: ClipEntry?) -> some View {
        let bulk = selection.count > 1 ? order.filter { selection.contains($0.id) } : []
        return VStack(spacing: 0) {
            listHeader
            searchField(order: order, active: active)
            if !enabled && !model.clipboard.entries.isEmpty { pausedBanner }
            list(rows: rows, order: order, active: active)
            if bulk.isEmpty {
                footer
            } else {
                bulkBar(bulk)
                    .padding(8)
                    .transition(KZTransition.rise)
            }
        }
        .animation(.kz.springSoft(duration: Motion.moderate), value: bulk.isEmpty)
        .accessibilityElement(children: .contain)
        .accessibilityLabel("Clipboard history")
    }

    private var listHeader: some View {
        HStack(spacing: 8) {
            HStack(spacing: 4) {
                Text(filter == "all" ? "Clipboard" : filterLabel).font(.system(size: 13, weight: .semibold))
                Text("\(results.count)").font(.system(size: 13).monospacedDigit()).foregroundStyle(Color.kz.faint)
            }
            .lineLimit(1)
            .frame(maxWidth: .infinity, alignment: .leading)
            RecordingToggle(enabled: enabled) { setEnabled(!enabled) }
            Menu { optionsMenu } label: { Icon.moreHorizontal.icon(16) }
                .kzMenuTrigger(.ghost, size: .iconSm)
                .foregroundStyle(Color.kz.mutedForeground)
                .accessibilityLabel("Clipboard options")
                .kzHelp("Options")
        }
        .padding(.leading, 20)
        .padding(.trailing, 8)
        .frame(height: 48)
    }

    @ViewBuilder private var optionsMenu: some View {
        Button { setEnabled(!enabled) } label: {
            Label(enabled ? "Pause recording" : "Resume recording", systemImage: enabled ? "pause" : "play")
        }
        Menu {
            Picker("Delete items after", selection: Binding(
                get: { retention },
                set: { days in update { $0.clipboardRetentionDays = UInt32(days) } }
            )) {
                ForEach(Clipboard.RETENTION_OPTIONS, id: \.0) { option in Text(option.1).tag(option.0) }
            }
            .pickerStyle(.inline)
        } label: {
            Label("Keep history", systemImage: "list.clipboard")
        }
        Button { model.setView(.settings(section: .clipboard)) } label: {
            Label("Clipboard settings…", systemImage: "slider.horizontal.3")
        }
        Divider()
        Button(role: .destructive) { clearOpen = true } label: { Label("Clear history…", systemImage: "trash") }
            .disabled(model.clipboard.entries.isEmpty)
    }

    private func searchField(order: [ClipEntry], active: ClipEntry?) -> some View {
        let focused = field == .search
        let shape = RoundedRectangle(cornerRadius: Radius.lg)
        return HStack(spacing: 8) {
            Icon.search.icon(14).foregroundStyle(Color.kz.faint)
            TextField("Search history", text: $query)
                .textFieldStyle(.plain)
                .font(.system(size: 13))
                .focused($field, equals: .search)
                .accessibilityLabel("Search clipboard history")
                .accessibilityHint("Type app: followed by an app name, or is: followed by a kind such as image or link, to narrow results.")
                .onKeyPress(phases: .down) { press in searchKey(press, order: order, active: active) }
            if query.isEmpty {
                Shortcut("mod+f")
            } else {
                Button { query = "" } label: { Icon.x.icon(14) }
                    .buttonStyle(.plain)
                    .foregroundStyle(Color.kz.faint)
                    .accessibilityLabel("Clear search")
            }
        }
        .padding(.horizontal, 10)
        .frame(height: 32)
        .background {
            ZStack {
                if focused {
                    shape.strokeBorder(Color.kz.ring, lineWidth: 2)
                } else {
                    Color.clear.depth(.well, fill: Color.kz.foreground.opacity(0.03), in: shape)
                }
            }
        }
        .padding(.horizontal, 12)
        .padding(.bottom, 8)
    }

    private var pausedBanner: some View {
        HStack(spacing: 8) {
            Icon.pause.icon(14)
            Text("Recording is paused. New copies aren't saved.").frame(maxWidth: .infinity, alignment: .leading)
            Button("Resume") { setEnabled(true) }
                .buttonStyle(.plain)
                .fontWeight(.medium)
        }
        .font(.system(size: 12))
        .foregroundStyle(Color.kz.warning)
        .padding(.horizontal, 12).padding(.vertical, 8)
        .background(Color.kz.warning.opacity(0.1), in: RoundedRectangle(cornerRadius: Radius.lg))
        .overlay(RoundedRectangle(cornerRadius: Radius.lg).strokeBorder(Color.kz.warning.opacity(0.18)))
        .padding(.horizontal, 12)
        .padding(.bottom, 8)
        .accessibilityElement(children: .combine)
    }

    private func list(rows: [Clipboard.ClipRow], order: [ClipEntry], active: ClipEntry?) -> some View {
        let listFocused = field == .list || selection.count <= 1
        return ScrollViewReader { proxy in
            List(selection: selectionBinding(order: order)) {
                if rows.isEmpty {
                    empty.listRowSeparator(.hidden).listRowInsets(EdgeInsets()).listRowBackground(Color.clear)
                }
                ForEach(rows, id: \.listKey) { row in
                    switch row {
                    case let .header(_, label, count):
                        ClipSectionHeader(label: label, count: count)
                            .listRowSeparator(.hidden)
                            .listRowInsets(EdgeInsets())
                    case let .entry(_, entry, _, dated):
                        ClipRowView(
                            entry: entry,
                            dated: dated,
                            terms: terms,
                            active: entry.id == active?.id,
                            inSelection: selection.count > 1 && selection.contains(entry.id),
                            focused: listFocused
                        )
                        .tag(entry.id)
                        .id(entry.id)
                        .contextMenu { rowMenu(entry, order: order) }
                        .onTapGesture(count: 2) { copy(entry) }
                        .listRowSeparator(.hidden)
                        .listRowInsets(EdgeInsets(top: 1, leading: 6, bottom: 1, trailing: 6))
                    }
                }
            }
            .listStyle(.plain)
            .scrollContentBackground(.hidden)
            .environment(\.defaultMinListRowHeight, 1)
            .focused($field, equals: .list)
            .onKeyPress(phases: .down) { press in listKey(press, order: order, active: active) }
            .onChange(of: activeId) { _, id in
                guard let id else { return }
                withAnimation(.kz.ease(duration: Motion.fast)) { proxy.scrollTo(id) }
            }
        }
    }

    @ViewBuilder private var empty: some View {
        if model.clipboard.entries.isEmpty {
            ClipWelcome(enabled: enabled, hotkey: model.settings.clipboardHotkey, retention: retention) { setEnabled(true) }
        } else {
            VStack(spacing: 12) {
                Group {
                    if query.isEmpty {
                        Text("No \(filterLabel.lowercased()) yet")
                    } else {
                        Text("Nothing matches ") + Text("“\(query)”").foregroundStyle(Color.kz.foreground)
                    }
                }
                .font(.system(size: 14))
                .foregroundStyle(Color.kz.mutedForeground)
                Button("Show everything") {
                    query = ""
                    model.setView(.clipboard(filter: nil))
                }
                .buttonStyle(.kz(.secondary, size: .sm))
            }
            .frame(maxWidth: .infinity)
            .padding(.horizontal, 24)
            .padding(.top, 64)
            .rise()
        }
    }

    private var footer: some View {
        HStack(spacing: 4) {
            Text("Kept \(Clipboard.retentionLabel(retention))\(model.clipboard.diskUsage > 0 ? " · " + Clipboard.formatBytes(Int(clamping: model.clipboard.diskUsage)) : "")")
                .lineLimit(1)
                .frame(maxWidth: .infinity, alignment: .leading)
            HoverTextButton("Change") { model.setView(.settings(section: .clipboard)) }
        }
        .font(.system(size: 11))
        .foregroundStyle(Color.kz.faint)
        .padding(.horizontal, 16)
        .frame(height: 36)
        .overlay(alignment: .top) { Hairline() }
    }

    /// `rounded-xl bg-surface-raised p-1.5 pl-3 shadow-float`.
    private func bulkBar(_ bulk: [ClipEntry]) -> some View {
        let allPinned = bulk.allSatisfy(\.pinned)
        return HStack(spacing: 4) {
            Text("\(bulk.count) selected").font(.system(size: 12, weight: .medium).monospacedDigit())
                .frame(maxWidth: .infinity, alignment: .leading)
            ActionButton(allPinned ? "Unpin" : "Pin", icon: allPinned ? Icon.pinOff : Icon.pin, shortcut: "mod+p") { pin(bulk) }
            ActionButton("Delete", icon: Icon.trash2, shortcut: "mod+backspace", destructive: true) { remove(bulk) }
        }
        .padding(6)
        .padding(.leading, 6)
        .depth(.float, fill: Color.kz.surfaceRaised, in: RoundedRectangle(cornerRadius: Radius.xl))
        .accessibilityElement(children: .contain)
        .accessibilityLabel("Selection actions")
    }

    // MARK: Preview column

    @ViewBuilder private func previewColumn(_ active: ClipEntry?) -> some View {
        VStack(spacing: 0) {
            HStack {
                if let active {
                    Breadcrumbs([
                        BreadcrumbEntry(id: 0, label: filter == "all" ? "History" : filterLabel) { field = .list },
                        BreadcrumbEntry(id: 1, label: concealed(active) ? "Secret" : Clipboard.describe(active).title),
                    ])
                }
            }
            .padding(.horizontal, 24)
            .frame(maxWidth: .infinity, alignment: .leading)
            .frame(height: 48)
            .overlay(alignment: .bottom) { Hairline() }
            if let active {
                ScrollView {
                    VStack(alignment: .leading, spacing: 24) {
                        ClipHeading(entry: active) { setSecret(active, false) }
                        ClipContentView(entry: active, terms: terms)
                        ClipDetails(entry: active)
                    }
                    .padding(.horizontal, 24)
                    .padding(.vertical, 20)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .rise()
                }
                .id(active.id)
                actionBar(active)
            } else {
                Text(model.clipboard.entries.isEmpty ? "" : "Select an item to preview it")
                    .font(.system(size: 12))
                    .foregroundStyle(Color.kz.faint)
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
            }
        }
        .accessibilityElement(children: .contain)
        .accessibilityLabel("Preview")
    }

    private func actionBar(_ active: ClipEntry) -> some View {
        HStack(spacing: 4) {
            Button { copy(active) } label: {
                HStack(spacing: 6) {
                    Icon.copy.icon(14)
                    Text("Copy")
                    Shortcut("enter")
                }
            }
            .buttonStyle(.kz(size: .sm))
            if active.rich {
                Button { copy(active, flipPlain: true) } label: {
                    HStack(spacing: 6) {
                        Icon.type.icon(14)
                        Text(model.settings.clipboardPastePlain ? "With formatting" : "As plain text")
                        Shortcut("shift+enter", tone: .ghost)
                    }
                }
                .buttonStyle(.kz(.ghost, size: .sm))
            }
            Spacer(minLength: 8)
            if Clipboard.hasText(active) {
                ActionButton("Save as snippet", icon: Icon.textCursorInput, shortcut: "mod+s") { saveSnippet(active) }
            }
            if active.kind == .link || active.kind == .file {
                ActionButton(active.kind == .link ? "Open link" : "Open", icon: Icon.externalLink, shortcut: "mod+o") { open(active) }
            }
            if active.kind == .file {
                ActionButton("Show in Finder", icon: Icon.folderOpen, shortcut: "mod+r") { reveal(active) }
            }
            if Clipboard.hasText(active) {
                ActionButton(active.isSecret ? "Not a secret — show it" : "Hide as a secret", icon: active.isSecret ? Icon.shieldOff : Icon.shieldCheck) {
                    setSecret(active, !active.isSecret)
                }
            }
            ActionButton(active.pinned ? "Unpin" : "Pin — keep forever", icon: active.pinned ? Icon.pinOff : Icon.pin, shortcut: "mod+p", active: active.pinned) {
                pin([active])
            }
            ActionButton("Delete", icon: Icon.trash2, shortcut: "mod+backspace", destructive: true) { remove([active]) }
        }
        .padding(.horizontal, 16)
        .frame(height: 56)
        .overlay(alignment: .top) { Hairline() }
        .accessibilityElement(children: .contain)
        .accessibilityLabel("Item actions")
    }

    // MARK: Dialogs

    private var clearDialog: some View {
        DialogLayout("Clear clipboard history?", description: "Everything Kudzu remembered is deleted from this Mac. This can't be undone.") {
            Toggle("Also delete pinned items (\(model.clipCounts()[.pinned, default: 0]))", isOn: $clearPinned)
                .kzCheckbox()
                .font(.system(size: 13))
        } footer: {
            Button("Cancel") { clearOpen = false }
                .buttonStyle(.kz(.outline))
                .keyboardShortcut(.cancelAction)
            Button("Clear history") { clearHistory() }
                .buttonStyle(.kz(.destructive))
                .keyboardShortcut(.defaultAction)
        }
    }

    /// Window shortcuts while the clipboard is showing (`useHotkeys` in `clipboard-view.tsx`).
    private func shortcuts(order: [ClipEntry], active: ClipEntry?) -> some View {
        Group {
            Button("Pin") { pin(selected(order: order, active: active)) }
                .keyboardShortcut("p", modifiers: .command)
            Button("Save as snippet") { if let active, Clipboard.hasText(active) { saveSnippet(active) } }
                .keyboardShortcut("s", modifiers: .command)
            Button("Open") { if let active, active.kind == .link || active.kind == .file { open(active) } }
                .keyboardShortcut("o", modifiers: .command)
            Button("Show in Finder") { if let active, active.kind == .file { reveal(active) } }
                .keyboardShortcut("r", modifiers: .command)
        }
        .opacity(0)
        .frame(width: 0, height: 0)
        .accessibilityHidden(true)
    }

    // MARK: Selection

    /// The list's selection. A single selection is the previewed row; extending it keeps the row the
    /// cursor last moved to as the preview, like the web's `activeId` beside `multi`.
    private func selectionBinding(order: [ClipEntry]) -> Binding<Set<UUID>> {
        Binding(
            get: { selection.count > 1 ? selection : (activeId.map { [$0] } ?? []) },
            set: { next in
                let added = next.subtracting(selection)
                selection = next
                if next.count == 1 { activeId = next.first }
                else if let id = added.first ?? (activeId.flatMap { next.contains($0) ? $0 : nil }) ?? next.first { activeId = id }
            }
        )
    }

    /// Keeps a valid active row: the first one, or the neighbour of one that disappeared.
    private func keepActive(_ ids: [UUID]) {
        if let activeId, let index = ids.firstIndex(of: activeId) {
            lastIndex = index
        } else if !ids.isEmpty {
            activeId = ids[min(lastIndex, ids.count - 1)]
            selection = []
        } else {
            activeId = nil
            selection = []
        }
    }

    private func move(_ delta: Int, order: [ClipEntry], extend: Bool = false) {
        guard !order.isEmpty else { return }
        let ids = order.map(\.id)
        let current = activeId.flatMap { ids.firstIndex(of: $0) } ?? (delta > 0 ? -1 : ids.count)
        let next = min(ids.count - 1, max(0, current + delta))
        if extend, let from = activeId.flatMap({ ids.firstIndex(of: $0) }) {
            let bounds = min(from, next)...max(from, next)
            selection = selection.union(ids[bounds])
        } else {
            selection = []
        }
        activeId = ids[next]
    }

    private func selected(order: [ClipEntry], active: ClipEntry?) -> [ClipEntry] {
        if selection.count > 1 { return order.filter { selection.contains($0.id) } }
        return active.map { [$0] } ?? []
    }

    private func followFocus() {
        switch focus.request?.target {
        case .clipboardSearch: field = .search
        case .clipboardList: field = .list
        default: break
        }
    }

    // MARK: Keys

    private func searchKey(_ press: KeyPress, order: [ClipEntry], active: ClipEntry?) -> KeyPress.Result {
        switch press.key {
        case .downArrow, .upArrow:
            move(press.key == .downArrow ? 1 : -1, order: order, extend: press.modifiers.contains(.shift))
            return .handled
        case .return:
            if let active { copy(active, flipPlain: press.modifiers.contains(.shift)) }
            return .handled
        case .escape:
            if query.isEmpty { field = .list } else { query = "" }
            return .handled
        default:
            return .ignored
        }
    }

    private func listKey(_ press: KeyPress, order: [ClipEntry], active: ClipEntry?) -> KeyPress.Result {
        let command = press.modifiers.contains(.command)
        let plainKey = press.modifiers.isDisjoint(with: [.command, .control, .option])
        if command && (press.key == .delete || press.key == .deleteForward) {
            remove(selected(order: order, active: active)); return .handled
        }
        if command && press.characters.lowercased() == "c" {
            if let active { copy(active) }; return .handled
        }
        guard plainKey else { return .ignored }
        switch press.key {
        case .return:
            if let active { copy(active, flipPlain: press.modifiers.contains(.shift)) }
            return .handled
        case .space:
            if active != nil { quickLook = true }
            return .handled
        case .delete, .deleteForward:
            remove(selected(order: order, active: active))
            return .handled
        case .escape:
            guard selection.count > 1 else { return .ignored }
            selection = []
            return .handled
        default:
            break
        }
        switch press.characters {
        case "j": move(1, order: order, extend: press.modifiers.contains(.shift)); return .handled
        case "k": move(-1, order: order, extend: press.modifiers.contains(.shift)); return .handled
        case "/": field = .search; return .handled
        default: break
        }
        // Type to search.
        guard press.characters.count == 1, let character = press.characters.first,
              !character.isWhitespace, character.unicodeScalars.allSatisfy({ $0.value < 0xF700 }) else { return .ignored }
        query.append(press.characters)
        field = .search
        return .handled
    }

    // MARK: Actions

    private func concealed(_ entry: ClipEntry) -> Bool { entry.isSecret && !model.settings.clipboardShowSecrets }

    private func update(_ change: @escaping (inout SettingsPatch) -> Void) {
        Task {
            do { try await model.updateSettings(SettingsPatch(change)) } catch {
                toast.error("Couldn't save that setting", description: error.localizedDescription)
            }
        }
    }

    private func setEnabled(_ on: Bool) { update { $0.clipboardEnabled = on } }

    private func copy(_ entry: ClipEntry, flipPlain: Bool = false) {
        let plain = entry.rich && model.settings.clipboardPastePlain != flipPlain
        Task {
            do {
                try await model.copyClip(entry, plain: plain)
                toast.success(plain ? "Copied as plain text" : "Copied", duration: 1.6)
            } catch {
                toast.error("Couldn't copy", description: error.localizedDescription)
            }
        }
    }

    private func remove(_ entries: [ClipEntry]) {
        guard !entries.isEmpty else { return }
        selection = []
        let model = model
        let hideSecrets = !model.settings.clipboardShowSecrets
        Task {
            do {
                let removed = try await model.removeClips(entries.map(\.id))
                let label: String
                if removed.count == 1 {
                    let entry = removed[0]
                    label = entry.isSecret && hideSecrets
                        ? "Deleted a secret"
                        : "Deleted “\(ClipPreviewRules.prefix(Clipboard.describe(entry).title, 40).shown)”"
                } else {
                    label = "Deleted \(removed.count) items"
                }
                toast(label, action: ToastButton("Undo") { Task { try? await model.restoreClips(removed) } }, duration: 6)
                field = .list
            } catch {
                toast.error("Couldn't delete", description: error.localizedDescription)
                await model.refreshClipboard()
            }
        }
    }

    private func pin(_ entries: [ClipEntry]) {
        guard let pinned = model.togglePinned(entries) else { return }
        toast.success(pinned ? (entries.count > 1 ? "Pinned \(entries.count) items" : "Pinned. It won't expire.") : "Unpinned", duration: 1.6)
    }

    private func setSecret(_ entry: ClipEntry, _ secret: Bool) {
        let before = model.markClipSecret(entry, secret: secret)
        let model = model
        toast.success(
            secret ? "Hidden as a secret" : "Marked as not a secret",
            description: secret
                ? "It stays hidden until you hover over it, and AI agents can't see it."
                : "It's shown normally, and AI agents can read it if you share clipboard history with them.",
            action: ToastButton("Undo") { Task { try? await model.setClipSecret(entry.id, secret: before) } },
            duration: 6
        )
    }

    private func saveSnippet(_ entry: ClipEntry) {
        Task {
            do { try await model.saveClipAsSnippet(entry) } catch {
                toast.error("Couldn't save the snippet", description: error.localizedDescription)
            }
        }
    }

    private func open(_ entry: ClipEntry) {
        Task {
            do { try await model.openClip(entry) } catch {
                toast.error("Couldn't open it", description: error.localizedDescription)
            }
        }
    }

    private func reveal(_ entry: ClipEntry) {
        Task { try? await model.revealClip(entry) }
    }

    private func clearHistory() {
        clearOpen = false
        let includePinned = clearPinned
        Task {
            do {
                let count = try await model.clearClipboard(includePinned: includePinned)
                selection = []
                toast.success(count == 1 ? "Deleted 1 item" : "Deleted \(count) items")
            } catch {
                toast.error("Couldn't clear history", description: error.localizedDescription)
                await model.refreshClipboard()
            }
        }
    }

    @ViewBuilder private func rowMenu(_ entry: ClipEntry, order: [ClipEntry]) -> some View {
        Button("Copy") { copy(entry) }
        if entry.rich { Button(model.settings.clipboardPastePlain ? "Copy with Formatting" : "Copy as Plain Text") { copy(entry, flipPlain: true) } }
        Button(entry.pinned ? "Unpin" : "Pin") { pin([entry]) }
        if Clipboard.hasText(entry) { Button("Save as Snippet") { saveSnippet(entry) } }
        if entry.kind == .link || entry.kind == .file { Button("Open") { open(entry) } }
        if entry.kind == .file { Button("Show in Finder") { reveal(entry) } }
        if Clipboard.hasText(entry) { Button(entry.isSecret ? "Not a Secret" : "Hide as a Secret") { setSecret(entry, !entry.isSecret) } }
        Divider()
        Button("Delete", role: .destructive) { remove([entry]) }
    }
}

// MARK: Pieces

/// The header's "Recording / Paused" pill: green dot with a soft halo, or a warning-tinted pill.
private struct RecordingToggle: View {
    let enabled: Bool
    let action: () -> Void
    @State private var hovered = false

    var body: some View {
        Button(action: action) {
            HStack(spacing: 6) {
                Circle()
                    .fill(enabled ? Color.kz.success : Color.kz.warning)
                    .frame(width: 6, height: 6)
                    .background {
                        if enabled { Circle().fill(Color.kz.success.opacity(0.2)).padding(-3) }
                    }
                Text(enabled ? "Recording" : "Paused")
            }
            .font(.system(size: 11, weight: .medium))
            .foregroundStyle(enabled ? (hovered ? Color.kz.foreground : Color.kz.mutedForeground) : Color.kz.warning)
            .padding(.horizontal, 8)
            .frame(height: 24)
            .background {
                if enabled {
                    if hovered { Capsule().fill(Color.kz.foreground.opacity(0.05)) }
                } else {
                    Capsule().fill(Color.kz.warning.opacity(hovered ? 0.18 : 0.12))
                }
            }
            .contentShape(Capsule())
        }
        .buttonStyle(.plain)
        .onHover { hovered = $0 }
        .animation(.kz.colors, value: hovered)
        .animation(.kz.colors, value: enabled)
        .accessibilityLabel(enabled ? "Recording copies. Press to pause." : "Paused. Press to resume recording.")
        .accessibilityAddTraits(enabled ? .isSelected : [])
        .kzHelp(enabled ? "Pause recording" : "Resume recording")
    }
}

/// A toolbar icon button with a tooltip naming it and its shortcut (`Action`).
private struct ActionButton: View {
    let label: String
    let icon: Image
    var shortcut: String?
    var destructive = false
    var active = false
    let action: () -> Void
    @State private var hovered = false

    init(_ label: String, icon: Image, shortcut: String? = nil, destructive: Bool = false, active: Bool = false, action: @escaping () -> Void) {
        self.label = label
        self.icon = icon
        self.shortcut = shortcut
        self.destructive = destructive
        self.active = active
        self.action = action
    }

    var body: some View {
        Button(action: action) { icon.icon(16) }
            .buttonStyle(.kz(.ghost, size: .iconSm))
            .foregroundStyle(active ? Color.kz.irisForeground : destructive && hovered ? Color.kz.destructive : Color.kz.mutedForeground)
            .onHover { hovered = $0 }
            .accessibilityLabel(label)
            .accessibilityAddTraits(active ? .isSelected : [])
            .kzHelp(label, shortcut: shortcut)
    }
}

/// Small text button that brightens on hover (the footer's "Change").
private struct HoverTextButton: View {
    let title: String
    let action: () -> Void
    @State private var hovered = false

    init(_ title: String, action: @escaping () -> Void) {
        self.title = title
        self.action = action
    }

    var body: some View {
        Button(title, action: action)
            .buttonStyle(.plain)
            .foregroundStyle(hovered ? Color.kz.foreground : Color.kz.mutedForeground)
            .padding(.horizontal, 4)
            .onHover { hovered = $0 }
    }
}

/// The empty history: what it does and how to open it anywhere, or a way to turn it on.
private struct ClipWelcome: View {
    let enabled: Bool
    let hotkey: String
    let retention: Int
    let onEnable: () -> Void

    var body: some View {
        VStack(spacing: 16) {
            Icon.clipboardList.icon(20)
                .foregroundStyle(Color.kz.mutedForeground)
                .frame(width: 48, height: 48)
                .depth(.raised, fill: Color.kz.surfaceRaised, in: RoundedRectangle(cornerRadius: Radius.xl2))
                .sheen(.normal, in: RoundedRectangle(cornerRadius: Radius.xl2))
            VStack(spacing: 6) {
                Text(enabled ? "Everything you copy, in one place" : "Clipboard history is off")
                    .font(.system(size: 14, weight: .medium))
                Text(enabled
                    ? "Copy text, links, images or files and they appear here with the time you copied them. Kudzu keeps them \(Clipboard.retentionLabel(retention)), only on this Mac."
                    : "Turn it on to find anything you copied, from any app.")
                    .font(.system(size: 12))
                    .lineSpacing(3)
                    .foregroundStyle(Color.kz.mutedForeground)
                    .multilineTextAlignment(.center)
            }
            if enabled {
                HStack(spacing: 8) {
                    Text("Paste from history anywhere")
                    Shortcut(keys: Keys.acceleratorLabels(hotkey), tone: .accent)
                }
                .font(.system(size: 12))
                .foregroundStyle(Color.kz.mutedForeground)
            } else {
                Button(action: onEnable) { ButtonLabel("Turn on", icon: Icon.play) }
                    .buttonStyle(.kz(size: .sm))
            }
        }
        .frame(maxWidth: .infinity)
        .padding(.horizontal, 32)
        .padding(.top, 64)
        .rise()
    }
}

/// Space on a row: a large preview, ↑/↓ moving through the list, Space or Esc to close.
private struct QuickLookSheet: View {
    let entry: ClipEntry
    let terms: [String]
    let move: (Int) -> Void
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        VStack(spacing: 0) {
            ClipHeading(entry: entry)
                .padding(.horizontal, 24)
                .padding(.vertical, 14)
                .overlay(alignment: .bottom) { Hairline() }
            ScrollView {
                ClipContentView(entry: entry, terms: terms, large: true)
                    .padding(24)
                    .frame(maxWidth: .infinity, alignment: .leading)
            }
        }
        .frame(minWidth: 640, idealWidth: 920, maxWidth: 920, minHeight: 420, idealHeight: 640)
        .background(Color.kz.background)
        .focusable()
        .focusEffectDisabled()
        .onKeyPress(.space) { dismiss(); return .handled }
        .onKeyPress(.downArrow) { move(1); return .handled }
        .onKeyPress(.upArrow) { move(-1); return .handled }
        .onExitCommand { dismiss() }
    }
}

private extension Clipboard.ClipRow {
    /// Stable identity in the list: the day or the entry's key.
    var listKey: String {
        switch self {
        case let .header(key, _, _), let .entry(key, _, _, _): key
        }
    }
}
