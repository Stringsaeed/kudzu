import AppKit
import KudzuCore
import KudzuDesign
import SwiftUI

/// Compatibility alias matching `LibrarySlots` documentation (`SettingsView(section:)`).
public typealias SettingsView = SettingsScreen

/// Main Settings screen with a section list and scrollable detail pane (`settings.tsx`).
public struct SettingsScreen: View {
    @Environment(AppModel.self) private var model
    @Namespace private var navNamespace
    @FocusState private var navFocused: Bool

    private let initialSection: SettingsSection?
    @State private var selectedSection: SettingsSection = .general

    public init(section: SettingsSection? = nil) {
        self.initialSection = section
        self._selectedSection = State(initialValue: section ?? .general)
    }

    private var activeSection: SettingsSection {
        if case let .settings(routed) = model.view, let routed {
            return routed
        }
        return selectedSection
    }

    public var body: some View {
        HStack(spacing: 0) {
            sectionSidebar
            Hairline(.vertical)
            detailPane
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(Color.kz.background)
        .onAppear {
            if let initialSection {
                selectedSection = initialSection
            } else if case let .settings(routed) = model.view, let routed {
                selectedSection = routed
            }
        }
        .onChange(of: initialSection) { _, next in
            if let next {
                selectedSection = next
            }
        }
        .onChange(of: model.view) { _, newView in
            if case let .settings(routed) = newView, let routed {
                selectedSection = routed
            }
        }
    }

    // MARK: Section Sidebar

    /// `<nav className="w-48 gap-px border-r border-hairline px-2.5">`: a 48 pt title row, then
    /// plain 28 pt text rows under one sliding `bg-accent shadow-selected` pill. ↑/↓ move between
    /// sections only while the list has focus (the web handles them on the focused nav button).
    private var sectionSidebar: some View {
        VStack(alignment: .leading, spacing: 1) {
            Text("Settings")
                .font(.system(size: 13, weight: .semibold))
                .foregroundStyle(.kz.foreground)
                .padding(.horizontal, 10)
                .frame(maxWidth: .infinity, minHeight: 48, maxHeight: 48, alignment: .leading)
                .background { SettingsWindowDrag() }
                .accessibilityAddTraits(.isHeader)

            ForEach(SettingsNav.sections, id: \.self) { section in
                SettingsNavRow(
                    label: section.label,
                    selected: activeSection == section,
                    namespace: navNamespace
                ) {
                    navFocused = true
                    selectSection(section)
                }
            }
        }
        .padding(.horizontal, 10)
        .frame(width: 192)
        .frame(maxHeight: .infinity, alignment: .topLeading)
        .focusable()
        .focused($navFocused)
        .focusEffectDisabled()
        .onKeyPress(.upArrow) { moveSection(-1) }
        .onKeyPress(.downArrow) { moveSection(1) }
        .accessibilityElement(children: .contain)
        .accessibilityLabel("Settings sections")
    }

    // MARK: Detail Pane

    /// A 48 pt header bar with the section name over a hairline, then the page column
    /// (`mx-auto max-w-[680px] gap-8 px-8 py-8 stagger`, keyed by section so the rise replays).
    private var detailPane: some View {
        VStack(spacing: 0) {
            Text(SettingsNav.header(for: activeSection))
                .font(.system(size: 13, weight: .semibold))
                .foregroundStyle(.kz.foreground)
                .accessibilityAddTraits(.isHeader)
                .padding(.horizontal, 24)
                .frame(maxWidth: .infinity, minHeight: 48, maxHeight: 48, alignment: .leading)
                .background { SettingsWindowDrag() }
                .overlay(alignment: .bottom) { Hairline() }

            ScrollView {
                sectionContent(for: activeSection)
                    .font(.system(size: 13))
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .padding(32)
                    .frame(maxWidth: 680)
                    .frame(maxWidth: .infinity)
            }
            // A new page per section: the stagger replays and the page starts at the top.
            .id(activeSection)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    @ViewBuilder
    private func sectionContent(for section: SettingsSection) -> some View {
        switch section {
        case .general:
            GeneralSettingsView()
        case .expansion:
            ExpansionSettingsView()
        case .clipboard:
            ClipboardSettingsView()
        case .agents:
            AgentsSettingsView()
        case .apps:
            AppsSettingsView()
        case .data:
            DataSettingsView()
        case .shortcuts:
            ShortcutsSettingsView()
        case .about:
            AboutSettingsView()
        }
    }

    private func selectSection(_ section: SettingsSection) {
        selectedSection = section
        model.setSettingsSection(section)
    }

    private func moveSection(_ step: Int) -> KeyPress.Result {
        selectSection(SettingsNav.neighbour(of: activeSection, step: step))
        return .handled
    }
}

/// A section button (`h-7 rounded-md px-2.5 text-[13px] text-muted-foreground press
/// [--press-scale:0.985] hover:text-foreground`, `hover:bg-accent` unless selected).
private struct SettingsNavRow: View {
    var label: String
    var selected: Bool
    var namespace: Namespace.ID
    var action: () -> Void
    @State private var hovered = false

    var body: some View {
        Button(action: action) {
            Text(label)
                .font(.system(size: 13))
                .foregroundStyle(.token(selected || hovered ? .foreground : .mutedForeground))
                .lineLimit(1)
                .padding(.horizontal, 10)
                .frame(maxWidth: .infinity, minHeight: 28, maxHeight: 28, alignment: .leading)
                .background {
                    if hovered && !selected {
                        RoundedRectangle(cornerRadius: Radius.md).fill(.token(.accent))
                    }
                }
                .selectionPill(selected, in: namespace, id: "settings-nav-pill", style: .settingsNav)
                .contentShape(RoundedRectangle(cornerRadius: Radius.md))
        }
        .buttonStyle(.press(scale: Press.subtle))
        .animation(.kz.hover, value: hovered)
        .onHover { hovered = $0 }
        .accessibilityAddTraits(selected ? .isSelected : [])
    }
}

private extension SelectionPillStyle {
    /// `SelectionPill className="rounded-md bg-accent shadow-selected"`.
    static let settingsNav = SelectionPillStyle(radius: Radius.md, fill: .accent, depth: .selected, sheen: false)
}

/// The header rows are drag regions (`data-tauri-drag-region`); controls in front keep their clicks.
private struct SettingsWindowDrag: NSViewRepresentable {
    func makeNSView(context: Context) -> DragView { DragView() }
    func updateNSView(_ view: DragView, context: Context) {}

    final class DragView: NSView {
        override var mouseDownCanMoveWindow: Bool { true }
    }
}

#Preview("Settings — General") {
    let model = AppModel(backend: MockBackend(), isDev: true)
    SettingsScreen(section: .general)
        .environment(model)
        .frame(width: 920, height: 640)
        .task { await model.start() }
}
