import KudzuUI
import SwiftUI

struct IconsPage: GalleryPage {
    static let id = "icons"
    static let title = "Icons"
    static let section = GallerySection.foundations
    static let icon = KzIconName.leaf

    var body: some View {
        GalleryPageLayout(
            title: "Icons",
            summary: "KzIcon(.name, size:). 16-unit grid, 1.5 strokes, round caps and joins. ink-muted at rest, ink on hover, vine-ink when active, bloom only for the favourite star."
        ) {
            SpecimenSection("Design system set", note: "The 33 glyphs from the design system.") {
                grid(KzIconName.allCases.filter { !$0.isAddition })
            }
            SpecimenSection("Additions", note: "Drawn for the app in the same style. SVGs in design/kudzu/additions/icons.") {
                grid(KzIconName.allCases.filter(\.isAddition))
            }
            SpecimenSection("Sizes", note: "28 for the empty-state mark, 16 in the sidebar, toolbar and settings, 14 in small buttons and menus, 12 in chips and rows.") {
                GalleryCard {
                    VStack(alignment: .leading, spacing: KzSpace.s4) {
                        ForEach([28, 16, 14, 12] as [CGFloat], id: \.self) { size in
                            HStack(alignment: .top, spacing: KzSpace.s3) {
                                Text("\(Int(size))").kzTextStyle(.monoCaption).foregroundStyle(KzColor.inkMuted).frame(width: 20, alignment: .leading)
                                LazyVGrid(columns: [GridItem(.adaptive(minimum: size, maximum: size), spacing: size * 0.6)], alignment: .leading, spacing: size * 0.6) {
                                    ForEach(KzIconName.allCases) { name in
                                        KzIcon(name, size: size).foregroundStyle(name == .starFill ? KzColor.bloom : KzColor.ink)
                                    }
                                }
                            }
                        }
                    }
                }
            }
            SpecimenSection("Colour roles") {
                GalleryCard {
                    HStack(spacing: KzSpace.s8) {
                        role("Rest", KzColor.inkMuted, .folder)
                        role("Hover", KzColor.ink, .folder)
                        role("Active", KzColor.vineInk, .folder)
                        role("Favourite", KzColor.bloom, .starFill)
                        role("Warning", KzColor.warning, .warning)
                        role("Danger", KzColor.danger, .error)
                    }
                }
            }
            SpecimenSection("Menu bar", note: "KzMenuBarIcon.<state>.image(): 18pt template images; macOS tints them. permissionsMissing and secureInput are additions.") {
                GalleryCard {
                    HStack(spacing: KzSpace.s8) {
                        ForEach(KzMenuBarIcon.allCases, id: \.self) { icon in
                            VStack(spacing: KzSpace.s2) {
                                HStack(spacing: KzSpace.s2) {
                                    Image(nsImage: icon.image()).renderingMode(.template).foregroundStyle(KzColor.ink)
                                        .padding(4)
                                        .background(KzColor.surfaceSunken, in: RoundedRectangle(cornerRadius: KzRadius.xs))
                                    Image(nsImage: icon.image()).renderingMode(.template).foregroundStyle(KzColorToken.ink.dark.color)
                                        .padding(4)
                                        .background(KzColorToken.canvas.dark.color, in: RoundedRectangle(cornerRadius: KzRadius.xs))
                                }
                                TokenName(".\(icon.rawValue)")
                            }
                        }
                    }
                }
            }
        }
    }

    private func grid(_ names: [KzIconName]) -> some View {
        GalleryCard {
            LazyVGrid(columns: [GridItem(.adaptive(minimum: 104), spacing: KzSpace.s2)], alignment: .leading, spacing: KzSpace.s4) {
                ForEach(names) { name in
                    VStack(spacing: KzSpace.s2) {
                        KzIcon(name, size: 16, label: name.accessibilityLabel)
                            .foregroundStyle(name == .starFill ? KzColor.bloom : KzColor.ink)
                        Text(name.rawValue).kzTextStyle(.monoCaption).foregroundStyle(KzColor.inkMuted)
                    }
                    .frame(maxWidth: .infinity)
                }
            }
        }
    }

    private func role(_ label: String, _ color: Color, _ icon: KzIconName) -> some View {
        VStack(spacing: KzSpace.s2) {
            KzIcon(icon, size: 16).foregroundStyle(color)
            Text(label).kzTextStyle(.caption).foregroundStyle(KzColor.inkMuted)
        }
    }
}
