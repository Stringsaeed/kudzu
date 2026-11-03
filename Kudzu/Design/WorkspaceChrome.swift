import KudzuUI
import SwiftUI

extension EnvironmentValues {
    @Entry var workspaceInspectorSnapshot = false
}

/// Flat library chrome; native window policy and focus adapters remain owned by KudzuUI.
struct WorkspaceShell<Sidebar: View, Footer: View, Results: View, Detail: View>: View {
    let mode: KzAppShellMode
    @ViewBuilder let sidebar: () -> Sidebar
    @ViewBuilder let sidebarFooter: () -> Footer
    @ViewBuilder let list: () -> Results
    @ViewBuilder let detail: () -> Detail
    @Environment(\.kzSnapshotRendering) private var snapshot

    var body: some View {
        Group {
            if mode == .window {
                panes.ignoresSafeArea().modifier(WorkspaceScrollEdges()).kzWindowChrome()
            } else {
                panes.clipShape(RoundedRectangle(cornerRadius: 10))
            }
        }
        .font(WorkspaceTheme.body)
        .tint(WorkspaceTheme.accent)
    }

    private var panes: some View {
        HStack(spacing: 0) {
            VStack(spacing: 0) {
                Color.clear.frame(height: 52).kzWindowDragArea()
                HStack(spacing: 7) {
                    KzIcon(.leaf, size: 19).foregroundStyle(WorkspaceTheme.accent)
                    Text("Kudzu").font(.system(size: 14, weight: .semibold))
                    Spacer()
                }
                .padding(.horizontal, 16).padding(.bottom, 22)
                if snapshot {
                    navigation.frame(maxHeight: .infinity, alignment: .top)
                } else {
                    ScrollView { navigation }.scrollBounceBehavior(.basedOnSize)
                }
                sidebarFooter().padding(8)
                    .overlay(alignment: .top) { WorkspaceTheme.line.frame(height: 1) }
            }
            .frame(width: 180)
            .background(WorkspaceTheme.navigation)
            WorkspaceTheme.line.frame(width: 1)
            VStack(spacing: 0) { list() }
                .frame(width: 300).frame(maxHeight: .infinity)
                .background(WorkspaceTheme.navigation)
            WorkspaceTheme.line.frame(width: 1)
            detail().frame(minWidth: 380, maxWidth: .infinity, maxHeight: .infinity)
                .background(WorkspaceTheme.paper)
        }
        .frame(minWidth: 862, minHeight: 480)
    }

    private var navigation: some View {
        sidebar().padding(.horizontal, 8).padding(.bottom, 12)
    }
}

/// Small, native action with a predictable hit target and no surrounding capsule.
struct WorkspaceIconButton: View {
    let symbol: String
    let title: String
    let action: () -> Void

    init(_ symbol: String, _ title: String, action: @escaping () -> Void) {
        self.symbol = symbol
        self.title = title
        self.action = action
    }

    var body: some View {
        Button(action: action) {
            Image(systemName: symbol).font(.system(size: 13))
                .frame(width: 28, height: 28).contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .foregroundStyle(WorkspaceTheme.secondary)
        .help(title).accessibilityLabel(title)
    }
}

/// Quiet navigation selection, with a green leading marker and native button focus.
struct WorkspaceNavigationStyle: ButtonStyle {
    var isActive = false
    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .font(WorkspaceTheme.body)
            .padding(.horizontal, 9).frame(height: 30)
            .frame(maxWidth: .infinity, alignment: .leading)
            .contentShape(Rectangle())
            .background(isActive ? WorkspaceTheme.selection : (configuration.isPressed ? WorkspaceTheme.line : .clear))
            .overlay(alignment: .leading) {
                if isActive { WorkspaceTheme.accent.frame(width: 2).padding(.vertical, 6) }
            }
    }
}

/// Native menus use their actual labels in ImageRenderer, which cannot draw an NSMenu-backed view.
struct WorkspaceMenu<Content: View, Label: View>: View {
    @ViewBuilder let content: () -> Content
    @ViewBuilder let label: () -> Label
    @Environment(\.kzSnapshotRendering) private var snapshot

    var body: some View {
        if snapshot {
            label()
        } else {
            Menu(content: content, label: label)
        }
    }
}

/// Keeps long trigger collections and validation messages reachable without displacing the document.
struct WorkspaceTriggerArea<Content: View>: View {
    @ViewBuilder let content: () -> Content
    @State private var contentHeight: CGFloat = 34
    @Environment(\.kzSnapshotRendering) private var snapshot

    var body: some View {
        if snapshot {
            content().fixedSize(horizontal: false, vertical: true)
        } else {
            ScrollView {
                content()
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .fixedSize(horizontal: false, vertical: true)
                    .onGeometryChange(for: CGFloat.self) { $0.size.height } action: { contentHeight = $0 }
            }
            .scrollBounceBehavior(.basedOnSize)
            .frame(height: min(contentHeight, 112))
        }
    }
}

private struct WorkspaceScrollEdges: ViewModifier {
    func body(content: Content) -> some View {
        if #available(macOS 26.0, *) {
            content.scrollEdgeEffectHidden(true, for: .top)
        } else {
            content
        }
    }
}
