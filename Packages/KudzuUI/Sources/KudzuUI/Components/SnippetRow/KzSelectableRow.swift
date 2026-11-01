import AppKit
import SwiftUI

extension View {
    /// Gives a list row Kudzu's selectable look: a `vine-soft` fill when selected, `surface-hover`
    /// under the pointer, and the selected accessibility trait.
    ///
    /// The row doesn't own selection. Selection belongs to the list (`List(selection:)`) or to the
    /// consumer, who passes `isSelected`. `KzSnippetRow` and `KzListRow` use this.
    public func kzSelectableRow(isSelected: Bool) -> some View {
        modifier(KzSelectableRowModifier(isSelected: isSelected))
    }

    /// Outlines a row while its context menu is open, as native lists do for the right-clicked row.
    ///
    /// Pair it with `.kzContextMenu(isPresented:)` and apply it to the row view (inside `.kzListRow()`):
    ///
    /// ```swift
    /// KzSnippetRow(…).kzContextMenu(isPresented: $targeted) { … }.kzContextTarget(targeted)
    /// ```
    public func kzContextTarget(_ isTarget: Bool) -> some View {
        overlay {
            // Same inset 2px outline as keyboard focus, in `focus` (an alias of `vine-ink`).
            if isTarget {
                RoundedRectangle(cornerRadius: KzRadius.md).inset(by: 1).stroke(KzColor.focus, lineWidth: 2)
                    .allowsHitTesting(false)
            }
        }
    }

    /// Configures a view as a row of a `List`: no separator, no system row background, and a 1px gap
    /// between rows. The plain list style already adds the design system's 8px side gutter.
    ///
    /// Pair with `.kzSelectionList()` on the `List`.
    public func kzListRow() -> some View {
        self
            .listRowSeparator(.hidden)
            .listRowBackground(Color.clear)
            .listRowInsets(EdgeInsets(top: 0.5, leading: 0, bottom: 0.5, trailing: 0))
    }
}

extension List {
    /// Styles a `List` as Kudzu's selectable list pane: plain, transparent, and with the system
    /// selection highlight turned off so `kzSelectableRow` draws selection instead.
    ///
    /// Keep using `List(selection:)`: it gives you row recycling for thousands of rows, arrow-key
    /// and type-to-select navigation, multi-select and VoiceOver's selection semantics.
    public func kzSelectionList() -> some View {
        self
            .listStyle(.plain)
            .scrollContentBackground(.hidden)
            .background { KzTableConfigurator() }
    }
}

// MARK: - Row

private struct KzSelectableRowModifier: ViewModifier {
    let isSelected: Bool
    @State private var isHovered = false
    @Environment(\.kzPreviewInteraction) private var preview
    @Environment(\.isFocused) private var isFocused

    func body(content: Content) -> some View {
        let shape = RoundedRectangle(cornerRadius: KzRadius.md)
        let hovered = preview?.hovered ?? isHovered
        let focused = preview?.focused ?? isFocused
        // Selection wins over hover, as in the CSS (`.is-selected` follows `:hover`).
        let fill: Color = isSelected ? KzColor.vineSoft : hovered ? KzColor.surfaceHover : .clear
        content
            .background(shape.fill(fill))
            .overlay {
                // The CSS outline sits inside the row (offset -2px), so a 2px stroke inset by 1px.
                if focused { shape.inset(by: 1).stroke(KzColor.focus, lineWidth: 2) }
            }
            .contentShape(shape)
            .onHover { isHovered = $0 }
            .kzAnimation(.color, value: fill)
            .accessibilityAddTraits(isSelected ? .isSelected : [])
    }
}

// MARK: - Table configuration

/// Reaches the `NSTableView` behind a SwiftUI `List` and turns off its selection highlight and
/// background, so the rows' own selection fill is the only one drawn.
///
/// SwiftUI has no public switch for the macOS list selection colour, so this is a small AppKit
/// bridge. If the table can't be found the list simply keeps the system highlight.
private struct KzTableConfigurator: NSViewRepresentable {
    func makeNSView(context: Context) -> NSView { Probe() }
    func updateNSView(_ view: NSView, context: Context) { (view as? Probe)?.schedule() }

    final class Probe: NSView {
        private var observation: NSKeyValueObservation?
        private weak var configuredTable: NSTableView?

        override func viewDidMoveToWindow() {
            super.viewDidMoveToWindow()
            schedule()
        }

        func schedule(attempt: Int = 0) {
            // The table is a sibling in the hosting hierarchy and SwiftUI creates it a beat after this view,
            // so look again shortly if it isn't there yet.
            DispatchQueue.main.asyncAfter(deadline: .now() + (attempt == 0 ? 0 : 0.05)) { [weak self] in
                guard let self, !self.configure(), attempt < 20 else { return }
                self.schedule(attempt: attempt + 1)
            }
        }

        /// Returns whether a table was found.
        private func configure() -> Bool {
            var ancestor = superview
            while let view = ancestor {
                if let table = Self.firstTable(in: view) {
                    adopt(table)
                    return true
                }
                ancestor = view.superview
            }
            return false
        }

        private func adopt(_ table: NSTableView) {
            table.backgroundColor = .clear
            table.enclosingScrollView?.drawsBackground = false
            guard configuredTable !== table else { return }
            configuredTable = table
            table.selectionHighlightStyle = .none
            // SwiftUI puts the highlight style back whenever it updates the list, so hold it at `.none`.
            observation = table.observe(\.selectionHighlightStyle, options: [.new]) { table, _ in
                MainActor.assumeIsolated {
                    if table.selectionHighlightStyle != .none { table.selectionHighlightStyle = .none }
                }
            }
        }

        private static func firstTable(in view: NSView) -> NSTableView? {
            if let table = view as? NSTableView { return table }
            for sub in view.subviews { if let found = firstTable(in: sub) { return found } }
            return nil
        }
    }
}
