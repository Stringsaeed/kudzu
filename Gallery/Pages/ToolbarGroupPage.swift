import KudzuUI
import SwiftUI

struct ToolbarGroupPage: GalleryPage {
    static let id = "toolbar-group"
    static let title = "ToolbarGroup"
    static let section = GallerySection.floating
    static let icon = KzIconName.more

    @State private var favorite = true
    @State private var debugging = false
    @State private var lastAction = "Open the More menu."

    var body: some View {
        GalleryPageLayout(title: Self.title, summary: "A small glass capsule that floats icon buttons over the detail pane: the one place glass appears inside the content area. Keep it to 2–4 actions; separate clusters with KzToolbarSeparator.") {
            SpecimenSection("Over the detail pane", note: "glass-strong, glass-edge, shadow-glass, radius-lg; 3pt padding, 2pt gaps, buttons 28pt tall.") {
                ZStack(alignment: .topTrailing) {
                    VStack(alignment: .leading, spacing: KzSpace.s2) {
                        Text("address").kzMonoTitle().foregroundStyle(KzColor.ink)
                        Text("221B Baker Street, London").kzTextStyle(.content).foregroundStyle(KzColor.inkMuted)
                    }
                    .padding(KzSpace.s6)
                    .frame(maxWidth: .infinity, minHeight: 140, alignment: .topLeading)

                    KzToolbarGroup(label: "Snippet actions") {
                        KzIconButton(favorite ? .starFill : .star, label: "Favorite", tone: favorite ? .bloom : .neutral) { favorite.toggle() }
                        KzIconButton(.bug, label: "Debug", active: debugging) { debugging.toggle() }
                        KzToolbarSeparator()
                        KzIconButton(.more, label: "More", placement: .pullDown(.trailing)) {
                            GalleryMenus.snippetActions { lastAction = $0 }
                        }
                    }
                    .padding(KzSpace.s4)
                }
                .background(KzColor.surfaceRaised, in: RoundedRectangle(cornerRadius: KzRadius.lg))
                .overlay(RoundedRectangle(cornerRadius: KzRadius.lg).strokeBorder(KzColor.line))
                Text(lastAction).kzTextStyle(.callout).foregroundStyle(KzColor.inkMuted)
            }

            SpecimenSection("Over colour", note: "The fill carries the contrast; the blur shows only in a real window.") {
                ZStack {
                    GlassBackdrop()
                    KzToolbarGroup {
                        KzIconButton(.star, label: "Favorite") {}
                        KzIconButton(.bug, label: "Debug", caret: true) {}
                        KzToolbarSeparator()
                        KzIconButton(.more, label: "More") {}
                    }
                }
                .frame(height: 120)
                .clipShape(RoundedRectangle(cornerRadius: KzRadius.lg))
            }
        }
    }
}
