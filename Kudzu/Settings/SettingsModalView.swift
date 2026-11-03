import AppKit
import KudzuUI
import SwiftUI

extension EnvironmentValues {
    @Entry var settingsModalPresentation = false
    @Entry var settingsModalFocusSession: SettingsModalFocusSession?
}

/// Settings stays inside the main content window, above a still-mounted library/editor.
struct SettingsModalView<Content: View>: View {
    @Binding var selection: SettingsSection
    let close: () -> Void
    @ViewBuilder var content: (SettingsSection) -> Content

    var body: some View {
        GeometryReader { geometry in
            let frame = SettingsModalLayout.frame(in: geometry.size)
            ZStack {
                Color.black.opacity(0.32)
                    .contentShape(Rectangle())
                    .onTapGesture { /* The backdrop never discards or closes anything. */ }
                    .accessibilityHidden(true)
                SettingsModalHost(close: close) { requestClose in
                    surface(width: frame.width, close: requestClose)
                }
                .frame(width: frame.width, height: frame.height)
            }
            .frame(width: geometry.size.width, height: geometry.size.height)
        }
        .ignoresSafeArea(.container)
    }

    private func surface(width: CGFloat, close: @escaping () -> Void) -> some View {
        let shape = RoundedRectangle(cornerRadius: 10, style: .continuous)
        return VStack(spacing: 0) {
            HStack(spacing: 0) {
                if width >= 700 {
                    Text("Settings")
                        .font(WorkspaceTheme.label)
                        .foregroundStyle(WorkspaceTheme.secondary)
                        .padding(.leading, 20)
                        .frame(width: 168, height: 52, alignment: .leading)
                        .background(WorkspaceTheme.navigation)
                        .overlay(alignment: .trailing) { WorkspaceTheme.line.frame(width: 1) }
                }
                Text(selection.title)
                    .font(.system(size: 16, weight: .medium))
                    .foregroundStyle(WorkspaceTheme.ink)
                    .lineLimit(1)
                    .frame(maxWidth: WorkspaceSurface.contentMaxWidth, alignment: .leading)
                    .frame(maxWidth: .infinity)
                    .padding(.horizontal, WorkspaceSurface.pagePadding)
                    .accessibilityAddTraits(.isHeader)
                    .overlay(alignment: .trailing) {
                        Button(action: close) {
                            Image(systemName: "xmark")
                                .font(.system(size: 12, weight: .medium))
                                .foregroundStyle(WorkspaceTheme.secondary)
                                .frame(width: 28, height: 28)
                        }
                        .buttonStyle(.borderless)
                        .accessibilityLabel("Close settings")
                        .help("Close settings")
                        .padding(.trailing, 12)
                    }
            }
            .frame(height: 52)
            .overlay(alignment: .bottom) { WorkspaceHairline() }
            if width >= 700 {
                HStack(spacing: 0) {
                    ScrollView {
                        VStack(spacing: 2) {
                            ForEach(SettingsSection.allCases) { section in
                                SettingsSectionNavRow(
                                    title: section.title,
                                    isSelected: selection == section
                                ) {
                                    selection = section
                                }
                            }
                        }
                        .padding(10)
                    }
                    .frame(width: 168)
                    .background(WorkspaceTheme.navigation)
                    .overlay(alignment: .trailing) { WorkspaceTheme.line.frame(width: 1) }
                    page
                }
            } else {
                Picker("Settings section", selection: $selection) {
                    ForEach(SettingsSection.allCases) { section in
                        Text(section.title).tag(section)
                    }
                }
                .labelsHidden()
                .pickerStyle(.menu)
                .padding(.horizontal, WorkspaceSurface.pagePadding)
                .padding(.vertical, 10)
                page
            }
        }
        .background(WorkspaceTheme.paper)
        .font(WorkspaceTheme.body)
        .tint(WorkspaceTheme.accent)
        .clipShape(shape)
        .overlay(shape.strokeBorder(WorkspaceTheme.line, lineWidth: 1))
        .shadow(color: .black.opacity(0.12), radius: 24, y: 8)
        .environment(\.settingsModalPresentation, true)
        .accessibilityElement(children: .contain)
        .accessibilityLabel("Settings")
    }

    private var page: some View {
        content(selection)
            .id(selection)
            .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
            .clipped()
    }
}

/// A separate hosting subtree gives AppKit a concrete focus boundary, without creating a window.
private struct SettingsModalHost<Content: View>: NSViewRepresentable {
    let close: () -> Void
    @ViewBuilder var content: (@escaping () -> Void) -> Content

    func makeCoordinator() -> SettingsModalFocusSession { SettingsModalFocusSession() }

    func makeNSView(context: Context) -> ModalHostingView {
        let session = context.coordinator
        session.close = close
        let host = ModalHostingView(rootView: AnyView(content { session.requestClose() }
            .environment(\.settingsModalFocusSession, session)
            .environment(\.self, context.environment)))
        host.sizingOptions = []
        host.setAccessibilityRole(.group)
        host.setAccessibilityLabel("Settings")
        host.setAccessibilityModal(true)
        host.didAttach = { [weak host, weak session] in
            guard let host else { return }
            session?.begin(in: host)
        }
        return host
    }

    func updateNSView(_ host: ModalHostingView, context: Context) {
        let session = context.coordinator
        session.close = close
        host.rootView = AnyView(content { session.requestClose() }
            .environment(\.settingsModalFocusSession, session)
            .environment(\.self, context.environment))
    }

    static func dismantleNSView(_ host: ModalHostingView, coordinator: SettingsModalFocusSession) {
        host.didAttach = nil
        coordinator.end()
    }
}

private final class ModalHostingView: NSHostingView<AnyView> {
    var didAttach: (() -> Void)?
    override var acceptsFirstResponder: Bool { true }

    override func viewDidMoveToWindow() {
        super.viewDidMoveToWindow()
        if window != nil { didAttach?() }
    }
}
