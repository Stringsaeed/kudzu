import KudzuUI
import SwiftUI

// App-local presentation helpers for settings, palette, forms, and onboarding.
// Shared tokens live in `WorkspaceTheme`; this file is not part of that contract.

enum WorkspaceSurface {
    static let contentMaxWidth: CGFloat = 680
    static let pagePadding: CGFloat = 24
    static let sectionGap: CGFloat = 24
    static let navRowHeight: CGFloat = 32
}

/// Inline status or confirmation copy without icon chrome.
struct WorkspaceInlineMessage: View {
    var text: String
    var isError = false

    var body: some View {
        Text(text)
            .font(WorkspaceTheme.body)
            .foregroundStyle(isError ? KzColor.danger : WorkspaceTheme.secondary)
            .textSelection(.enabled)
            .fixedSize(horizontal: false, vertical: true)
            .frame(maxWidth: .infinity, alignment: .leading)
    }
}

/// Compact text status (permissions, practice state) without badge capsules.
struct WorkspaceStatusTag: View {
    enum Tone { case neutral, accent, caution }

    let title: String
    var tone: Tone = .neutral

    var body: some View {
        Text(title)
            .font(WorkspaceTheme.caption.weight(.medium))
            .foregroundStyle(foreground)
            .padding(.horizontal, 6)
            .padding(.vertical, 2)
            .background(background, in: RoundedRectangle(cornerRadius: 4))
            .accessibilityLabel(title)
    }

    private var foreground: Color {
        switch tone {
        case .neutral: WorkspaceTheme.secondary
        case .accent: WorkspaceTheme.accent
        case .caution: KzColor.warning
        }
    }

    private var background: Color {
        switch tone {
        case .neutral: WorkspaceTheme.line.opacity(0.35)
        case .accent: WorkspaceTheme.selection
        case .caution: KzColor.warningSoft
        }
    }
}

/// Plain monospace trigger label (replaces decorative trigger chips in supporting surfaces).
struct WorkspaceTriggerLabel: View {
    let trigger: String

    var body: some View {
        Text(trigger)
            .font(WorkspaceTheme.mono)
            .foregroundStyle(WorkspaceTheme.accent)
            .lineLimit(1)
            .accessibilityLabel("Trigger \(trigger)")
    }
}

/// Keyboard hint as caption text, not a pill.
struct WorkspaceKeyHint: View {
    let keys: String
    var title: String?

    var body: some View {
        HStack(spacing: 4) {
            Text(keys)
                .font(WorkspaceTheme.mono)
                .foregroundStyle(WorkspaceTheme.secondary)
            if let title {
                Text(title)
                    .font(WorkspaceTheme.caption)
                    .foregroundStyle(WorkspaceTheme.secondary)
            }
        }
        .accessibilityElement(children: .combine)
    }
}

/// Settings modal sidebar row — flat, ~32pt, subtle selection fill.
struct SettingsSectionNavRow: View {
    let title: String
    let isSelected: Bool
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            Text(title)
                .font(isSelected ? WorkspaceTheme.label : WorkspaceTheme.body)
                .foregroundStyle(isSelected ? WorkspaceTheme.ink : WorkspaceTheme.secondary)
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding(.horizontal, 10)
                .frame(height: WorkspaceSurface.navRowHeight)
                .background(isSelected ? WorkspaceTheme.selection : Color.clear, in: RoundedRectangle(cornerRadius: 6))
        }
        .buttonStyle(.plain)
        .accessibilityAddTraits(isSelected ? [.isSelected] : [])
    }
}

/// Section action row (e.g. “Back up now”) aligned with workspace typography.
struct WorkspaceSectionAction: View {
    let title: String
    let action: () -> Void

    var body: some View {
        HStack {
            Spacer(minLength: 0)
            Button(title, action: action)
                .controlSize(.small)
                .buttonStyle(.bordered)
        }
    }
}

/// Flat horizontal separator used instead of nested cards.
struct WorkspaceHairline: View {
    var body: some View {
        WorkspaceTheme.line.frame(height: 1)
    }
}
