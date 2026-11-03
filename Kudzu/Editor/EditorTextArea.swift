import AppKit
import SwiftUI

/// A bounded, resizable text area that keeps its native editor mounted while its viewport changes.
struct EditorTextArea<Content: View, Preview: View>: View {
    @Binding var preferredHeight: Double
    var isFocused: Bool
    var previewVisible: Bool
    @ViewBuilder let content: () -> Content
    @ViewBuilder let preview: () -> Preview
    @State private var dragOrigin: CGFloat?
    @State private var hoveringGrip = false

    private let minimumHeight: CGFloat = 116

    var body: some View {
        GeometryReader { geometry in
            let previewHeight = previewVisible ? min(160, max(0, geometry.size.height - minimumHeight - 12)) : 0
            let maximumHeight = max(minimumHeight, geometry.size.height - (previewVisible ? previewHeight + 12 : 0))
            let height = min(maximumHeight, max(minimumHeight, preferredHeight > 0 ? preferredHeight : maximumHeight))
            VStack(spacing: 0) {
                VStack(spacing: 0) {
                    content().frame(maxWidth: .infinity, maxHeight: .infinity)
                    WorkspaceTheme.line.frame(height: 1)
                    resizeGrip(height: height, maximum: maximumHeight)
                }
                .frame(height: height)
                .background(Color(nsColor: .textBackgroundColor))
                .clipShape(RoundedRectangle(cornerRadius: 6))
                .overlay(RoundedRectangle(cornerRadius: 6)
                    .strokeBorder(isFocused ? WorkspaceTheme.accent : WorkspaceTheme.line, lineWidth: 1))
                if previewVisible, previewHeight > 0 {
                    preview().frame(height: previewHeight).padding(.top, 12)
                }
                Spacer(minLength: 0)
            }
        }
        .frame(minHeight: minimumHeight, maxHeight: .infinity)
        .onDisappear {
            if hoveringGrip { NSCursor.pop(); hoveringGrip = false }
        }
    }

    private func resizeGrip(height: CGFloat, maximum: CGFloat) -> some View {
        HStack {
            Spacer()
            Image(systemName: "arrow.up.and.down")
                .font(.system(size: 8, weight: .medium))
                .foregroundStyle(WorkspaceTheme.secondary)
                .padding(.trailing, 8)
        }
        .frame(height: 14)
        .contentShape(Rectangle())
        .onHover { inside in
            guard inside != hoveringGrip else { return }
            hoveringGrip = inside
            if inside { NSCursor.resizeUpDown.push() } else { NSCursor.pop() }
        }
        .gesture(DragGesture(minimumDistance: 1)
            .onChanged { value in
                if dragOrigin == nil { dragOrigin = height }
                preferredHeight = min(maximum, max(minimumHeight, (dragOrigin ?? height) + value.translation.height))
            }
            .onEnded { _ in dragOrigin = nil })
        .onTapGesture(count: 2) { preferredHeight = 0 }
        .focusable()
        .accessibilityIdentifier("kudzu.editor.resize")
        .accessibilityLabel("Text area height")
        .accessibilityValue("\(Int(height)) points")
        .accessibilityAdjustableAction { direction in
            switch direction {
            case .increment: preferredHeight = min(maximum, height + 40)
            case .decrement: preferredHeight = max(minimumHeight, height - 40)
            @unknown default: break
            }
        }
        .accessibilityAction(named: "Expand text area") { preferredHeight = 0 }
        .onKeyPress(.upArrow) { preferredHeight = max(minimumHeight, height - 40); return .handled }
        .onKeyPress(.downArrow) { preferredHeight = min(maximum, height + 40); return .handled }
        .onKeyPress(.return) { preferredHeight = 0; return .handled }
        .help("Drag to resize. Double-click or press Return to fill the available space.")
    }
}
