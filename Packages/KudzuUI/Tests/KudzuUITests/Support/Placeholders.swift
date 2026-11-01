import KudzuUI
import SwiftUI

// Stand-ins for controls other agents own (KzIconButton, KzButton, KzToggle, KzSelect, KzSearchField).
// Replace them with the real components when the branches are merged. They only exist so snapshots of
// this module's structure components show realistic content.

/// PLACEHOLDER for `KzIconButton`: a 30pt ghost icon button.
struct PlaceholderIconButton: View {
    let icon: KzIconName
    var size: CGFloat = KzSize.control
    var body: some View {
        KzIcon(icon, size: size < KzSize.control ? 14 : 16)
            .foregroundStyle(KzColor.inkMuted)
            .frame(width: size, height: size)
    }
}

/// PLACEHOLDER for `KzButton`: a 30pt secondary button.
struct PlaceholderButton: View {
    let title: String
    var small = false
    var primary = false
    var body: some View {
        let shape = RoundedRectangle(cornerRadius: small ? KzRadius.sm : KzRadius.md)
        Text(title)
            .kzTextStyle(small ? .callout : .bodyStrong, matchLineBox: false)
            .foregroundStyle(primary ? KzColor.onVine : KzColor.ink)
            .padding(.horizontal, small ? KzSpace.s2 : KzSpace.s3)
            .frame(height: small ? KzSize.controlSm : KzSize.control)
            .background(primary ? KzColor.vine : KzColor.surfaceRaised, in: shape)
            .overlay(shape.strokeBorder(primary ? Color.clear : KzColor.line))
            .fixedSize()
    }
}

/// PLACEHOLDER for `KzToggle`.
struct PlaceholderToggle: View {
    let isOn: Bool
    var body: some View {
        Capsule().fill(isOn ? KzColor.vine : KzColor.surfaceSunken)
            .overlay(Capsule().strokeBorder(isOn ? KzColor.vineInk : KzColor.edge))
            .overlay(alignment: isOn ? .trailing : .leading) {
                Circle().fill(isOn ? KzColor.onVine : KzColor.edge).frame(width: 14, height: 14).padding(.horizontal, 2)
            }
            .frame(width: 32, height: 20)
    }
}

/// PLACEHOLDER for `KzSelect`.
struct PlaceholderSelect: View {
    let value: String
    var body: some View {
        HStack(spacing: KzSpace.s2) {
            Text(value).kzTextStyle(.bodyStrong, matchLineBox: false).foregroundStyle(KzColor.ink)
            Spacer(minLength: 0)
            KzIcon(.chevronsV, size: 14).foregroundStyle(KzColor.inkMuted)
        }
        .padding(.leading, 10).padding(.trailing, 6)
        .frame(width: 140, height: KzSize.control)
        .background(KzColor.surfaceSunken, in: RoundedRectangle(cornerRadius: KzRadius.md))
    }
}

/// PLACEHOLDER for `KzSearchField`.
struct PlaceholderSearch: View {
    var body: some View {
        HStack(spacing: KzSpace.s2) {
            KzIcon(.search, size: 14)
            Text("Search snippets").kzTextStyle(.body, matchLineBox: false)
            Spacer(minLength: 0)
            KzKbd("⌘F")
        }
        .foregroundStyle(KzColor.inkMuted)
        .padding(.leading, 10).padding(.trailing, 6)
        .frame(height: KzSize.control)
        .background(KzColor.surfaceSunken, in: RoundedRectangle(cornerRadius: KzRadius.md))
    }
}

/// PLACEHOLDER for `KzTextInput` / `KzTriggerInput`: a 34pt sunken field.
struct PlaceholderField: View {
    var text: String? = nil
    var placeholder = ""
    var body: some View {
        HStack(spacing: 6) {
            if let text { KzTriggerChip(text) } else { Text(placeholder).kzTextStyle(.body, matchLineBox: false).foregroundStyle(KzColor.inkMuted) }
            Spacer(minLength: 0)
        }
        .padding(.horizontal, 10)
        .frame(maxWidth: .infinity, minHeight: KzSize.field, alignment: .leading)
        .background(KzColor.surfaceSunken, in: RoundedRectangle(cornerRadius: KzRadius.md))
    }
}

/// PLACEHOLDER for `KzSegmented`.
struct PlaceholderSegmented: View {
    let options: [String]
    var selected = 0
    var body: some View {
        HStack(spacing: 2) {
            ForEach(Array(options.enumerated()), id: \.offset) { index, option in
                Text(option).kzTextStyle(.callout, matchLineBox: false)
                    .foregroundStyle(index == selected ? KzColor.ink : KzColor.inkMuted)
                    .padding(.horizontal, KzSpace.s3)
                    .frame(height: 26)
                    .background(index == selected ? KzColor.surfaceRaised : .clear, in: RoundedRectangle(cornerRadius: KzRadius.sm))
            }
        }
        .padding(2)
        .background(KzColor.surfaceSunken, in: RoundedRectangle(cornerRadius: KzRadius.md))
        .fixedSize()
    }
}
