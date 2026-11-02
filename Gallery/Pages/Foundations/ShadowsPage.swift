import KudzuUI
import SwiftUI

struct ShadowsPage: GalleryPage {
    static let id = "shadows"
    static let title = "Shadows"
    static let section = GallerySection.foundations
    static let icon = KzIconName.sidebar

    var body: some View {
        GalleryPageLayout(
            title: "Shadows",
            summary: "None on content. Apply with .kzShadow(_:in:), which draws CSS blur and spread outside the shape and inset highlights inside its top edge."
        ) {
            LazyVGrid(columns: [GridItem(.adaptive(minimum: 180), spacing: KzSpace.s8)], alignment: .leading, spacing: KzSpace.s8) {
                ForEach(KzShadow.allCases, id: \.self) { shadow in
                    VStack(alignment: .leading, spacing: KzSpace.s3) {
                        let shape = RoundedRectangle(cornerRadius: KzRadius.lg)
                        shape.fill(KzColor.surfaceRaised)
                            .frame(height: 88)
                            .kzShadow(shadow, in: shape)
                        TokenName(".kzShadow(.\(shadow.rawValue.replacingOccurrences(of: "shadow-", with: "")), in:)")
                        Text(shadow.usage).kzTextStyle(.callout).foregroundStyle(KzColor.inkMuted)
                            .fixedSize(horizontal: false, vertical: true)
                    }
                }
            }
            .padding(KzSpace.s6)
            .background(KzColor.canvas, in: RoundedRectangle(cornerRadius: KzRadius.lg))
        }
    }
}
