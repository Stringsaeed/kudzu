import KudzuUI
import SwiftUI

struct ToolbarGroupPage: GalleryPage {
    static let id = "toolbar-group"
    static let title = "ToolbarGroup"
    static let section = GallerySection.components
    static let icon = KzIconName.more

    @State private var favorite = true
    @State private var debugging = false

    var body: some View {
        GalleryPageLayout(title: Self.title, summary: "A small glass capsule that floats icon buttons over the detail pane: the one place glass appears inside the content area. Keep it to 2–4 actions; separate clusters with KzToolbarSeparator.") {
            SpecimenSection("Over the detail pane", note: "glass-strong, glass-edge, shadow-glass, radius-lg; 3pt padding, 2pt gaps, buttons 28pt tall.") {
                ZStack(alignment: .topTrailing) {
                    VStack(alignment: .leading, spacing: KzSpace.s2) {
                        Text("address").kzTextStyle(.title).foregroundStyle(KzColor.ink)
                        Text("221B Baker Street, London").kzTextStyle(.content).foregroundStyle(KzColor.inkMuted)
                    }
                    .padding(KzSpace.s6)
                    .frame(maxWidth: .infinity, minHeight: 140, alignment: .topLeading)

                    KzToolbarGroup(label: "Snippet actions") {
                        FloatDemoIconButton(icon: favorite ? .starFill : .star, label: "Favorite", tint: favorite ? KzColor.bloom : nil) { favorite.toggle() }
                        FloatDemoIconButton(icon: .bug, label: "Debug", isActive: debugging) { debugging.toggle() }
                        KzToolbarSeparator()
                        KzMenuButton {
                            KzMenuItem("Duplicate", icon: .copy) {}.keyboardShortcut("d")
                            KzMenuItem("Copy to clipboard", icon: .copy) {}
                            KzMenuSeparator()
                            KzMenuItem("Delete", icon: .trash, role: .danger) {}.keyboardShortcut(.delete)
                        } label: { isOpen in
                            FloatDemoIconButtonLabel(icon: .more, isActive: isOpen)
                        }
                        .buttonStyle(FloatDemoIconButtonStyle())
                        .accessibilityLabel("More")
                    }
                    .padding(KzSpace.s4)
                }
                .background(KzColor.surfaceRaised, in: RoundedRectangle(cornerRadius: KzRadius.lg))
                .overlay(RoundedRectangle(cornerRadius: KzRadius.lg).strokeBorder(KzColor.line))
            }

            SpecimenSection("Over colour", note: "The fill carries the contrast; the blur shows only in a real window.") {
                ZStack {
                    GlassBackdrop()
                    KzToolbarGroup {
                        FloatDemoIconButton(icon: .star, label: "Favorite") {}
                        FloatDemoIconButton(icon: .bug, label: "Debug") {}
                        KzToolbarSeparator()
                        FloatDemoIconButton(icon: .more, label: "More") {}
                    }
                }
                .frame(height: 120)
                .clipShape(RoundedRectangle(cornerRadius: KzRadius.lg))
            }
        }
    }
}

// MARK: - Stand-in for KzIconButton (built by the controls work)

/// `.kz-iconbtn` inside a toolbar: 30×28, `radius-md`, `ink-muted` → `ink` on hover, `vine-soft` when active.
struct FloatDemoIconButtonStyle: ButtonStyle {
    func makeBody(configuration: Configuration) -> some View {
        KzInteractive(RoundedRectangle(cornerRadius: KzRadius.md), isPressed: configuration.isPressed) { _ in
            configuration.label
        }
    }
}

struct FloatDemoIconButtonLabel: View {
    let icon: KzIconName
    var tint: Color?
    var isActive = false
    @Environment(\.kzInToolbarGroup) private var inToolbar

    var body: some View {
        KzIcon(icon, size: 16)
            .foregroundStyle(tint ?? (isActive ? KzColor.vineInk : KzColor.inkMuted))
            .frame(width: KzSize.control, height: inToolbar ? 28 : KzSize.control)
            .background(isActive ? KzColor.vineSoft : .clear, in: RoundedRectangle(cornerRadius: KzRadius.md))
    }
}

struct FloatDemoIconButton: View {
    let icon: KzIconName
    let label: String
    var tint: Color?
    var isActive = false
    let action: () -> Void

    var body: some View {
        Button(action: action) { FloatDemoIconButtonLabel(icon: icon, tint: tint, isActive: isActive) }
            .buttonStyle(FloatDemoIconButtonStyle())
            .accessibilityLabel(label)
            .help(label)
    }
}
