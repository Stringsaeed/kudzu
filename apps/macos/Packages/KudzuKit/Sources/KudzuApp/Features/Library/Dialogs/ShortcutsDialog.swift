// ⌘/ (`shortcuts.tsx`): every shortcut, plus the global search and clipboard accelerators.

import AppKit
import KudzuCore
import KudzuDesign
import SwiftUI

struct ShortcutsSheet: View {
    @Environment(AppModel.self) private var model
    @Environment(\.dismiss) private var dismiss
    @State private var viewportHeight = 760.0

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            HStack(spacing: 12) {
                Text("Keyboard shortcuts")
                    .font(.system(size: 15, weight: .semibold))
                    .accessibilityAddTraits(.isHeader)
                Spacer(minLength: 8)
                HStack(spacing: 8) {
                    Text("From any app: search")
                    Shortcut(keys: Keys.acceleratorLabels(model.settings.paletteHotkey), tone: .accent)
                    if model.settings.clipboardEnabled {
                        Text("·").foregroundStyle(Color.kz.faint)
                        Text("clipboard")
                        Shortcut(keys: Keys.acceleratorLabels(model.settings.clipboardHotkey), tone: .accent)
                    }
                }
                .font(.system(size: 12))
                .foregroundStyle(Color.kz.mutedForeground)
            }
            .padding(.horizontal, 24)
            .padding(.vertical, 16)
            .overlay(alignment: .bottom) { Hairline() }
            ScrollView {
                LazyVGrid(columns: [GridItem(.flexible(), spacing: 40), GridItem(.flexible(), spacing: 40)], alignment: .leading, spacing: 24) {
                    ForEach(LibraryShortcuts.sections) { section in
                        // `<h3 className="mb-2">` over a column of `h-7` rows with no gap.
                        VStack(alignment: .leading, spacing: 0) {
                            Text(section.title)
                                .font(.system(size: 11, weight: .medium))
                                .foregroundStyle(Color.kz.faint)
                                .padding(.bottom, 8)
                            ForEach(section.items) { item in
                                HStack(spacing: 16) {
                                    Text(item.label)
                                        .font(.system(size: 13))
                                        .foregroundStyle(Color.kz.foreground.opacity(0.85))
                                        .lineLimit(1)
                                    Spacer(minLength: 8)
                                    Shortcut(item.combo)
                                }
                                .frame(height: 28)
                            }
                        }
                        .accessibilityElement(children: .contain)
                        .accessibilityLabel(section.title)
                    }
                }
                .padding(.horizontal, 24)
                .padding(.vertical, 20)
            }
            .frame(maxHeight: CGFloat(LibraryShortcuts.contentHeight(viewportHeight: viewportHeight)))
        }
        .frame(width: 720)
        .onAppear {
            let window = NSApp.keyWindow
            viewportHeight = Double(window?.sheetParent?.frame.height ?? window?.frame.height ?? 760)
        }
        .background(Color.kz.popover)
        .onExitCommand { model.shortcutsOpen = false; dismiss() }
        .accessibilityLabel("Keyboard shortcuts")
        .accessibilityHint("Every shortcut in Kudzu.")
    }
}
