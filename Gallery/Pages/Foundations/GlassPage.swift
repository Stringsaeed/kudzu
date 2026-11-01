import KudzuUI
import SwiftUI

struct GlassPage: GalleryPage {
    static let id = "glass"
    static let title = "Glass"
    static let section = GallerySection.foundations
    static let icon = KzIconName.cloud

    var body: some View {
        GalleryPageLayout(
            title: "Glass",
            summary: "Only at the edges: the sidebar, menus, popovers and the floating toolbar. System blur, a translucent tint that carries the contrast, a 1px glass-stroke inside the top edge and a 1px glass-edge outside. Never stack glass on glass."
        ) {
            SpecimenSection("Styles", note: "Snapshots can't render the system blur, so they show the tint alone.") {
                ZStack {
                    GlassBackdrop()
                    HStack(spacing: KzSpace.s8) {
                        sample(".kzGlass(.regular, in:)", detail: "glass · sidebar") {
                            $0.kzGlass(.regular, in: RoundedRectangle(cornerRadius: KzRadius.xl))
                        }
                        sample(".kzGlass(.strong, in:, shadow: .glass)", detail: "glass-strong · toolbar") {
                            $0.kzGlass(.strong, in: RoundedRectangle(cornerRadius: KzRadius.lg), shadow: .glass)
                        }
                        sample(".kzGlass(.strong, in:, shadow: .float)", detail: "glass-strong · menus") {
                            $0.kzGlass(.strong, in: RoundedRectangle(cornerRadius: KzRadius.lg), shadow: .float)
                        }
                    }
                }
                .frame(height: 240)
                .clipShape(RoundedRectangle(cornerRadius: KzRadius.lg))
            }
            SpecimenSection("Sidebar", note: "In the app the sidebar uses .behindWindow blending so the blur reaches the desktop, and goes flat when the window is inactive.") {
                ZStack(alignment: .leading) {
                    GlassBackdrop()
                    VStack(alignment: .leading, spacing: 1) {
                        ForEach([(KzIconName.stack, "All snippets"), (.starFill, "Favorites"), (.clock, "Recently used"), (.chart, "Stats")], id: \.1) { icon, label in
                            HStack(spacing: 10) {
                                KzIcon(icon, size: 16).foregroundStyle(label == "All snippets" ? KzColor.vineInk : KzColor.inkMuted)
                                Text(label).kzTextStyle(.bodyStrong).foregroundStyle(KzColor.ink)
                            }
                            .padding(.horizontal, 10)
                            .frame(maxWidth: .infinity, minHeight: KzSize.control, alignment: .leading)
                            .background {
                                if label == "All snippets" {
                                    RoundedRectangle(cornerRadius: KzRadius.md).fill(KzColor.glassSelected)
                                        .kzShadow(.control, in: RoundedRectangle(cornerRadius: KzRadius.md))
                                }
                            }
                        }
                        Spacer()
                    }
                    .padding(KzSpace.s2)
                    .padding(.top, KzSpace.s8)
                    .frame(width: KzSize.sidebar)
                    .frame(maxHeight: .infinity)
                    .kzGlass(.regular, in: Rectangle(), blending: .behindWindow)
                }
                .frame(height: 260)
                .clipShape(RoundedRectangle(cornerRadius: KzRadius.lg))
            }
        }
    }

    private func sample(_ code: String, detail: String, apply: (AnyView) -> some View) -> some View {
        apply(AnyView(
            VStack(alignment: .leading, spacing: 4) {
                Text(detail).kzTextStyle(.bodyStrong).foregroundStyle(KzColor.ink)
                Text(code).kzTextStyle(.monoCaption).foregroundStyle(KzColor.inkMuted)
            }
            .padding(KzSpace.s4)
            .frame(width: 220, height: 120, alignment: .topLeading)
        ))
    }
}
