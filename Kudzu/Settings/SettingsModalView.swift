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
    }

    private func surface(width: CGFloat, close: @escaping () -> Void) -> some View {
        let shape = RoundedRectangle(cornerRadius: KzRadius.xl)
        return VStack(spacing: 0) {
            HStack(spacing: KzSpace.s2) {
                Text("Settings").foregroundStyle(KzColor.inkMuted)
                Text("/").foregroundStyle(KzColor.inkMuted)
                Text(selection.title).foregroundStyle(KzColor.ink).lineLimit(1)
                Spacer(minLength: 0)
                KzIconButton(.x, label: "Close settings", action: close)
            }
            .kzTextStyle(.bodyStrong)
            .padding(.horizontal, KzSpace.s4)
            .frame(height: KzSize.titlebar)
            .overlay(alignment: .bottom) { KzColor.line.frame(height: 1) }
            if width >= 700 {
                HStack(spacing: 0) {
                    ScrollView { navigation }
                        .frame(width: 184)
                        .background(KzColor.surfaceSunken)
                        .overlay(alignment: .trailing) { KzColor.line.frame(width: 1) }
                    page
                }
            } else {
                // A compact section menu leaves the existing settings rows enough room.
                KzSelect("Settings section", selection: $selection,
                         options: SettingsSection.allCases.map { ($0, $0.title) })
                    .padding(KzSpace.s3)
                page
            }
        }
        .background(KzColor.surface)
        .clipShape(shape)
        .overlay(shape.strokeBorder(KzColor.line))
        .kzShadow(.float, in: shape)
        .environment(\.settingsModalPresentation, true)
        .accessibilityElement(children: .contain)
        .accessibilityLabel("Settings")
    }

    private var navigation: some View {
        VStack(spacing: 1) {
            ForEach(SettingsSection.allCases) { section in
                KzSidebarItem(section.title, icon: section.icon, isActive: selection == section) {
                    selection = section
                }
            }
        }
        .padding(KzSpace.s2)
        .accessibilityElement(children: .contain)
        .accessibilityLabel("Settings sections")
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
