import KudzuUI
import SwiftUI

/// Kudzu's brand colours with a quieter, document-oriented application hierarchy.
enum WorkspaceTheme {
    static let paper = KzColor.surfaceRaised
    static let navigation = KzColor.surface
    static let ink = KzColor.ink
    static let secondary = KzColor.inkMuted
    static let line = KzColor.line
    static let accent = KzColor.vineInk
    static let selection = KzColor.vineSoft

    static let body = Font.system(size: 13)
    static let label = Font.system(size: 13, weight: .medium)
    static let caption = Font.system(size: 11)
    static let title = Font.system(size: 20, weight: .medium)
    static let mono = Font.system(size: 11, design: .monospaced)
}

/// A section separated by typography and spacing instead of another surrounding card.
struct WorkspaceSection<Content: View>: View {
    let title: String
    let description: String?
    let content: Content

    init(_ title: String, description: String? = nil, @ViewBuilder content: () -> Content) {
        self.title = title
        self.description = description
        self.content = content()
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text(title)
                .font(WorkspaceTheme.label)
                .foregroundStyle(WorkspaceTheme.ink)
                .accessibilityAddTraits(.isHeader)
            if let description {
                Text(description)
                    .font(.system(size: 12))
                    .foregroundStyle(WorkspaceTheme.secondary)
                    .fixedSize(horizontal: false, vertical: true)
            }
            VStack(spacing: 0) { content }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }
}

/// A responsive setting row; narrow layouts put the control below its label.
struct WorkspaceSettingRow<Control: View>: View {
    let title: String
    let description: String?
    let control: Control

    init(_ title: String, description: String? = nil, @ViewBuilder control: () -> Control) {
        self.title = title
        self.description = description
        self.control = control()
    }

    var body: some View {
        ViewThatFits(in: .horizontal) {
            HStack(spacing: 20) {
                label.frame(minWidth: 160, maxWidth: .infinity, alignment: .leading)
                control.fixedSize()
            }
            VStack(alignment: .leading, spacing: 8) {
                label
                control.frame(maxWidth: .infinity, alignment: .leading)
            }
        }
        .controlSize(.small)
        .padding(.vertical, 12)
        .overlay(alignment: .bottom) { WorkspaceTheme.line.frame(height: 1) }
    }

    private var label: some View {
        VStack(alignment: .leading, spacing: 3) {
            Text(title).font(WorkspaceTheme.body).foregroundStyle(WorkspaceTheme.ink)
            if let description {
                Text(description)
                    .font(.system(size: 12))
                    .foregroundStyle(WorkspaceTheme.secondary)
                    .fixedSize(horizontal: false, vertical: true)
            }
        }
    }
}
