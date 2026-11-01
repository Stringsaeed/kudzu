import KudzuCore
import SwiftUI

/// Rows of the `/` insert menu.
struct SlashMenuView: View {
    let model: SlashMenuModel
    let onPick: (TemplateInsertItem) -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 1) {
            Text(model.query.isEmpty ? "Insert" : "Insert “\(model.query)”")
                .font(.caption.weight(.semibold))
                .foregroundStyle(.secondary)
                .padding(.horizontal, 8)
                .padding(.top, 2)
                .padding(.bottom, 3)
            ForEach(model.items) { item in
                InsertItemRow(item: item, isSelected: item == model.selection)
                    .contentShape(.rect)
                    .onTapGesture { onPick(item) }
                    .onHover { inside in if inside { model.selection = item } }
                    .accessibilityAddTraits(.isButton)
                    .accessibilityAddTraits(item == model.selection ? .isSelected : [])
            }
        }
        .padding(5)
        .frame(width: 250)
        .fixedSize(horizontal: false, vertical: true)
    }
}

/// One menu entry: tinted symbol, title and a short explanation.
struct InsertItemRow: View {
    let item: TemplateInsertItem
    var isSelected = false

    var body: some View {
        HStack(spacing: 8) {
            Image(systemName: item.symbolName)
                .font(.system(size: 12, weight: .medium))
                .foregroundStyle(isSelected ? Color.white : Color(nsColor: ChipStyle.tint(forItem: item)))
                .frame(width: 22, height: 22)
                .background(
                    RoundedRectangle(cornerRadius: 5, style: .continuous)
                        .fill(isSelected ? Color.white.opacity(0.2) : Color(nsColor: ChipStyle.tint(forItem: item)).opacity(0.14)))
            VStack(alignment: .leading, spacing: 0) {
                Text(item.title)
                    .font(.body)
                Text(item.subtitle)
                    .font(.caption)
                    .foregroundStyle(isSelected ? AnyShapeStyle(Color.white.opacity(0.8)) : AnyShapeStyle(.secondary))
            }
            Spacer(minLength: 0)
        }
        .foregroundStyle(isSelected ? Color.white : Color.primary)
        .padding(.horizontal, 6)
        .padding(.vertical, 3)
        .background(
            RoundedRectangle(cornerRadius: 5, style: .continuous)
                .fill(isSelected ? Color.accentColor : Color.clear))
        .accessibilityElement(children: .combine)
    }
}
