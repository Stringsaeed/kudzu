// The window toolbar (`toolbar.tsx`): sidebar toggle, breadcrumbs, and the ⌘K search button.
// On macOS the leading inset clears the traffic lights, and drops away in fullscreen.

import AppKit
import KudzuCore
import KudzuDesign
import SwiftUI

struct LibraryToolbar: View {
    /// Matches the window titlebar so the traffic lights share this row.
    static let height: CGFloat = 42

    @Binding var columns: NavigationSplitViewVisibility
    @Environment(AppModel.self) private var model
    @Environment(LibraryFocus.self) private var focus
    @State private var fullscreen = false
    /// `max-[1000px]:w-36` on the search button.
    @State private var narrow = false

    private var sidebarOpen: Bool { columns != .detailOnly }

    var body: some View {
        let crumbs = LibraryBreadcrumbs.crumbs(route: model.view, groups: model.library.groups, snippet: model.selectedSnippet)
        HStack(spacing: 12) {
            Button {
                columns = sidebarOpen ? .detailOnly : .all
            } label: {
                Icon.panelLeft.icon(16)
            }
            .buttonStyle(.kz(.ghost, size: .icon))
            .foregroundStyle(Color.kz.mutedForeground)
            .accessibilityLabel(sidebarOpen ? "Hide sidebar" : "Show sidebar")
            .accessibilityValue(sidebarOpen ? "expanded" : "collapsed")
            .kzHelp(sidebarOpen ? "Hide sidebar" : "Show sidebar")
            Hairline(.vertical, token: .border).frame(height: 18) // `<Separator orientation="vertical" className="my-3" />`
            Breadcrumbs(crumbs.enumerated().map { index, crumb in
                BreadcrumbEntry(id: index, label: crumb.label) {
                    guard let destination = crumb.destination else { return }
                    model.setView(destination)
                    focus.focusList()
                }
            })
            .frame(maxWidth: .infinity, alignment: .leading)
            // `variant="outline" h-8 w-48 justify-start gap-2 text-xs text-muted-foreground`.
            Button { model.commandOpen = true } label: {
                HStack(spacing: 8) {
                    Icon.search.icon(14)
                    Text("Search…").frame(maxWidth: .infinity, alignment: .leading)
                    Shortcut("mod+k")
                }
                .font(.system(size: 12))
                .foregroundStyle(Color.kz.mutedForeground)
            }
            .buttonStyle(.kz(.outline))
            .frame(width: narrow ? 144 : 192)
            .accessibilityLabel("Search commands and snippets")
        }
        .padding(.leading, model.platform == .macos && !fullscreen ? 88 : 12)
        .padding(.trailing, 16)
        .frame(height: Self.height)
        .onGeometryChange(for: Bool.self) { $0.size.width < 1000 } action: { narrow = $0 }
        .overlay(alignment: .bottom) { Hairline() }
        .background { WindowDrag() }
        .background { FullscreenProbe(isFullscreen: $fullscreen) }
    }
}

/// Empty toolbar space drags the window. Buttons stay in front and keep their clicks.
private struct WindowDrag: NSViewRepresentable {
    func makeNSView(context: Context) -> DragView { DragView() }
    func updateNSView(_ view: DragView, context: Context) {}

    final class DragView: NSView {
        override var mouseDownCanMoveWindow: Bool { true }
    }
}

/// Reports whether the hosting window is fullscreen, so the toolbar can drop the traffic-light inset.
private struct FullscreenProbe: NSViewRepresentable {
    @Binding var isFullscreen: Bool

    func makeNSView(context: Context) -> ProbeView {
        let view = ProbeView()
        view.isFullscreen = $isFullscreen
        return view
    }

    func updateNSView(_ view: ProbeView, context: Context) {
        view.isFullscreen = $isFullscreen
    }

    final class ProbeView: NSView {
        var isFullscreen: Binding<Bool>?
        private var last: Bool?

        override func hitTest(_ point: NSPoint) -> NSView? { nil }

        override func viewDidMoveToWindow() {
            super.viewDidMoveToWindow()
            report()
        }

        override func layout() {
            super.layout()
            report()
        }

        private func report() {
            let full = window?.styleMask.contains(.fullScreen) == true
            guard full != last else { return }
            last = full
            isFullscreen?.wrappedValue = full
        }
    }
}
