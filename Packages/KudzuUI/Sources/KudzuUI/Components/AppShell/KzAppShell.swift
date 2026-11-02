import AppKit
import SwiftUI

/// How a `KzAppShell` relates to its window.
public enum KzAppShellMode: Sendable {
    /// The shell is the content of a real window: it applies `KzWindowChrome` (hidden title and toolbar,
    /// full-size content, opaque background) and the system draws the traffic lights.
    case window
    /// The shell is shown inside something else (the gallery, a snapshot): the window is left alone,
    /// and the shell draws mock traffic lights, rounded corners and a window shadow.
    case embedded
}

/// The one Kudzu window: a glass sidebar, the list pane on `surface`, the detail pane on `surface-raised`.
/// Settings is a mode of this window, not a second window.
///
/// Widths come from `size-sidebar` (220) and `size-list` (300); the detail pane takes the rest. Every
/// pane's header row is `size-titlebar` tall, so the headers line up with the traffic lights; build
/// them with `KzListHeader` and `KzPaneHeader`. In `.window` mode the sidebar uses behind-window glass, so the
/// sidebar's backdrop blur reaches the desktop as a macOS sidebar does.
public struct KzAppShell<Sidebar: View, SidebarHeader: View, SidebarFooter: View, ListPane: View, Detail: View>: View {
    private let mode: KzAppShellMode
    private let sidebar: Sidebar
    private let sidebarHeader: SidebarHeader
    private let sidebarFooter: SidebarFooter
    private let listPane: ListPane
    private let detail: Detail
    @Environment(\.kzSnapshotRendering) private var snapshotRendering

    /// The smallest comfortable window size: both fixed panes plus a 360pt detail pane.
    public static var minimumSize: CGSize {
        CGSize(width: KzSize.sidebar + KzSize.list + 360, height: 480)
    }

    /// Creates the shell.
    /// - Parameters:
    ///   - mode: `.window` for the real window, `.embedded` for galleries and snapshots.
    ///   - sidebar: `KzSidebarSection`s. They are stacked `space-4` apart and scroll.
    ///   - sidebarHeader: Trailing controls in the sidebar's 52pt header row, e.g. a "Hide sidebar" `KzIconButton`.
    ///   - sidebarFooter: Pinned below a `line` hairline, e.g. the Settings `KzSidebarItem` with `⌘,`.
    ///   - list: A `KzListHeader`, search field and rows.
    ///   - detail: The detail content, starting with a `KzPaneHeader`.
    public init(
        mode: KzAppShellMode = .window,
        @ViewBuilder sidebar: () -> Sidebar,
        @ViewBuilder sidebarHeader: () -> SidebarHeader,
        @ViewBuilder sidebarFooter: () -> SidebarFooter,
        @ViewBuilder list: () -> ListPane,
        @ViewBuilder detail: () -> Detail
    ) {
        self.mode = mode
        self.sidebar = sidebar()
        self.sidebarHeader = sidebarHeader()
        self.sidebarFooter = sidebarFooter()
        self.listPane = list()
        self.detail = detail()
    }

    public var body: some View {
        let shell = HStack(spacing: 0) {
            sidebarColumn.frame(width: KzSize.sidebar)
            listColumn.frame(width: KzSize.list)
            detailColumn
        }
        .frame(minWidth: Self.minimumSize.width, minHeight: Self.minimumSize.height)
        switch mode {
        case .window:
            // The 52pt titlebar would otherwise be a top safe-area inset and push every pane down.
            shell
                .ignoresSafeArea()
                .modifier(KzHiddenTopScrollEdgeEffect())
                .kzWindowChrome()
        case .embedded:
            let window = RoundedRectangle(cornerRadius: KzRadius.xl, style: .continuous)
            shell
                .overlay(alignment: .topLeading) { KzMockTrafficLights() }
                .background(KzColor.surfaceRaised, in: window)
                .clipShape(window)
                .kzShadow(.window, in: window)
        }
    }

    // MARK: Panes

    private var sidebarColumn: some View {
        VStack(spacing: 0) {
            HStack(spacing: KzSpace.s2) {
                Spacer(minLength: 0)
                sidebarHeader
            }
            .padding(.trailing, KzSpace.s2)
            .frame(height: KzSize.titlebar)
            .kzWindowDragArea()
            sidebarBody
            VStack(spacing: 0) { sidebarFooter }
                .padding(KzSpace.s2)
                .frame(maxWidth: .infinity)
                .overlay(alignment: .top) { KzHairline() }
        }
        .frame(maxHeight: .infinity)
        .kzGlass(.regular, in: Rectangle(), blending: .behindWindow)
        .overlay(alignment: .trailing) { KzHairline(axis: .vertical, color: KzColor.glassEdge) }
    }

    @ViewBuilder
    private var sidebarBody: some View {
        let stack = VStack(alignment: .leading, spacing: KzSpace.s4) { sidebar }
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(.horizontal, KzSpace.s2)
            .padding(.top, KzSpace.s1)
            .padding(.bottom, KzSpace.s2)
        if snapshotRendering {
            // Offscreen renderers don't draw scroll views' content.
            stack.frame(maxHeight: .infinity, alignment: .top)
        } else {
            ScrollView { stack }.scrollBounceBehavior(.basedOnSize)
        }
    }

    private var listColumn: some View {
        VStack(spacing: 0) { listPane }
            .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .top)
            .background(KzColor.surface)
            .overlay(alignment: .trailing) { KzHairline(axis: .vertical) }
    }

    private var detailColumn: some View {
        VStack(spacing: 0) { detail }
            .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .top)
            .background(KzColor.surfaceRaised)
    }
}

extension KzAppShell where SidebarHeader == EmptyView {
    /// Creates the shell with the four standard slots.
    public init(
        mode: KzAppShellMode = .window,
        @ViewBuilder sidebar: () -> Sidebar,
        @ViewBuilder sidebarFooter: () -> SidebarFooter,
        @ViewBuilder list: () -> ListPane,
        @ViewBuilder detail: () -> Detail
    ) {
        self.init(mode: mode, sidebar: sidebar, sidebarHeader: { EmptyView() }, sidebarFooter: sidebarFooter, list: list, detail: detail)
    }
}

/// The three traffic lights, drawn where AppKit puts the real ones (see `KzWindowChrome`), for
/// shells that aren't in a real window.
private struct KzMockTrafficLights: View {
    // macOS's fixed traffic-light colours; they are not theme colours.
    private static let colors: [Color] = [
        Color(.sRGB, red: 1, green: 0.373, blue: 0.341),
        Color(.sRGB, red: 0.996, green: 0.737, blue: 0.18),
        Color(.sRGB, red: 0.157, green: 0.784, blue: 0.251),
    ]

    var body: some View {
        HStack(spacing: KzWindowChrome.Lights.pitch - KzWindowChrome.Lights.size) {
            ForEach(Self.colors.indices, id: \.self) { index in
                Circle()
                    .fill(Self.colors[index])
                    .overlay(Circle().strokeBorder(.black.opacity(0.15), lineWidth: 0.5))
                    .frame(width: KzWindowChrome.Lights.size, height: KzWindowChrome.Lights.size)
            }
        }
        .padding(.leading, KzWindowChrome.Lights.leading)
        .padding(.top, KzWindowChrome.Lights.top)
        .allowsHitTesting(false)
        .accessibilityHidden(true)
    }
}

/// Turns off the macOS 26 scroll edge effect under the titlebar. Every pane draws its own opaque
/// 52pt header there, and the effect would blur those headers (and the sidebar's) into a band.
private struct KzHiddenTopScrollEdgeEffect: ViewModifier {
    func body(content: Content) -> some View {
        if #available(macOS 26.0, *) {
            content.scrollEdgeEffectHidden(true, for: .top)
        } else {
            content
        }
    }
}
