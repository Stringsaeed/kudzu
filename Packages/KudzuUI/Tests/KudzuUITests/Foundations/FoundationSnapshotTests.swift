import KudzuUI
import SwiftUI
import Testing

/// Renders foundation specimens to `Packages/KudzuUI/Snapshots/foundations-*.png` for visual review.
/// The full gallery pages render with `KudzuGallery --snapshot` (see the README).
@MainActor
@Suite("Foundation snapshots")
struct FoundationSnapshotTests {
    @Test func icons() throws {
        try snapshot("foundations-icons") {
            VStack(alignment: .leading, spacing: KzSpace.s3) {
                ForEach([28, 16, 14, 12] as [CGFloat], id: \.self) { size in
                    HStack(spacing: size >= 16 ? 10 : 8) {
                        ForEach(KzIconName.allCases) { name in
                            KzIcon(name, size: size)
                                .foregroundStyle(name == .starFill ? KzColor.bloom : name.isAddition ? KzColor.vineInk : KzColor.ink)
                        }
                    }
                }
            }
        }
    }

    @Test func menuBarGlyphs() throws {
        try snapshot("foundations-menubar") {
            HStack(spacing: KzSpace.s6) {
                ForEach(KzMenuBarIcon.allCases, id: \.self) { icon in
                    VStack(spacing: KzSpace.s2) {
                        HStack(spacing: 8) {
                            Image(nsImage: icon.image()).renderingMode(.template).foregroundStyle(KzColor.ink)
                            Image(nsImage: icon.image()).renderingMode(.template).resizable().frame(width: 72, height: 72).foregroundStyle(KzColor.ink)
                        }
                        Text(icon.rawValue).kzTextStyle(.monoCaption).foregroundStyle(KzColor.inkMuted)
                    }
                }
            }
        }
    }

    @Test func type() throws {
        try snapshot("foundations-type", width: 520) {
            VStack(alignment: .leading, spacing: KzSpace.s2) {
                ForEach(KzTextStyle.allCases, id: \.self) { style in
                    HStack(alignment: .firstTextBaseline) {
                        Text(style.rawValue).kzTextStyle(.monoCaption).foregroundStyle(KzColor.inkMuted).frame(width: 96, alignment: .leading)
                        Text(style.spec.sample).kzTextStyle(style).foregroundStyle(KzColor.ink)
                            .background(KzColor.vineSoft.opacity(0.6))
                    }
                }
            }
        }
    }

    @Test func colours() throws {
        try snapshot("foundations-colours", width: 520) {
            LazyVGrid(columns: Array(repeating: GridItem(.flexible(), spacing: KzSpace.s2), count: 4), spacing: KzSpace.s2) {
                ForEach(KzColorToken.allCases, id: \.self) { token in
                    VStack(alignment: .leading, spacing: 2) {
                        RoundedRectangle(cornerRadius: KzRadius.sm).fill(token.color).frame(height: 32)
                            .overlay(RoundedRectangle(cornerRadius: KzRadius.sm).strokeBorder(KzColor.line))
                        Text(token.rawValue).kzTextStyle(.monoCaption).foregroundStyle(KzColor.inkMuted)
                    }
                }
            }
        }
    }

    @Test func shadowsAndGlass() throws {
        try snapshot("foundations-shadows-glass", width: 560, background: .canvas) {
            VStack(alignment: .leading, spacing: KzSpace.s6) {
                HStack(spacing: KzSpace.s6) {
                    ForEach(KzShadow.allCases, id: \.self) { shadow in
                        let shape = RoundedRectangle(cornerRadius: KzRadius.lg)
                        shape.fill(KzColor.surfaceRaised).frame(width: 96, height: 64)
                            .kzShadow(shadow, in: shape)
                            .overlay(Text(shadow.rawValue).kzTextStyle(.caption).foregroundStyle(KzColor.inkMuted))
                    }
                }
                .padding(KzSpace.s4)
                ZStack {
                    LinearGradient(colors: [KzColor.vine, KzColor.bloom, KzColor.vineInk], startPoint: .topLeading, endPoint: .bottomTrailing)
                        .frame(height: 140)
                    HStack(spacing: KzSpace.s6) {
                        Text("glass").kzTextStyle(.bodyStrong).foregroundStyle(KzColor.ink)
                            .frame(width: 160, height: 96)
                            .kzGlass(.regular, in: RoundedRectangle(cornerRadius: KzRadius.lg))
                        Text("glass-strong").kzTextStyle(.bodyStrong).foregroundStyle(KzColor.ink)
                            .frame(width: 160, height: 96)
                            .kzGlass(.strong, in: RoundedRectangle(cornerRadius: KzRadius.lg), shadow: .float)
                    }
                }
            }
        }
    }

    @Test func interactionStates() throws {
        try snapshot("foundations-interaction") {
            HStack(spacing: KzSpace.s4) {
                stateSample("rest")
                stateSample("hover").kzPreviewInteraction(hovered: true)
                stateSample("pressed").kzPreviewInteraction(pressed: true)
                stateSample("focus").kzPreviewInteraction(focused: true)
                stateSample("disabled").disabled(true)
            }
            .padding(KzSpace.s2)
        }
    }

    private func stateSample(_ label: String) -> some View {
        let shape = RoundedRectangle(cornerRadius: KzRadius.md)
        return KzInteractive(shape) { state in
            HStack(spacing: 6) {
                KzIcon(.compose, size: 16)
                Text(label).kzTextStyle(.bodyStrong)
            }
            .foregroundStyle(state.isEnabled ? KzColor.ink : KzDisabled.foreground)
            .padding(.horizontal, KzSpace.s3)
            .frame(height: KzSize.control)
            .background(KzColor.surfaceRaised, in: shape)
            .overlay(shape.strokeBorder(KzColor.line))
        }
    }
}
