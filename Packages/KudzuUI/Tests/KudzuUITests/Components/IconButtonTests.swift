import KudzuUI
import SwiftUI
import Testing

@MainActor
@Suite struct IconButtonSnapshotTests {
    @Test func states() throws {
        try snapshot("iconbutton-states", width: 400) {
            VStack(alignment: .leading, spacing: 16) {
                ForEach(KzButton.Size.allCases, id: \.self) { size in
                    HStack(spacing: 16) {
                        KzIconButton(.compose, label: "New snippet", size: size) {}
                        KzIconButton(.compose, label: "New snippet", size: size) {}.kzPreviewInteraction(hovered: true)
                        KzIconButton(.compose, label: "New snippet", size: size) {}.kzPreviewInteraction(pressed: true)
                        KzIconButton(.compose, label: "New snippet", size: size) {}.kzPreviewInteraction(focused: true)
                        KzIconButton(.compose, label: "New snippet", size: size) {}.disabled(true)
                    }
                    HStack(spacing: 16) {
                        KzIconButton(.sidebar, label: "Sidebar", active: true, size: size) {}
                        KzIconButton(.starFill, label: "Favorite", size: size, tone: .bloom) {}
                        KzIconButton(.more, label: "More", caret: true, size: size) {}
                        KzIconButton(.more, label: "More", caret: true, size: size) {}.kzPreviewInteraction(hovered: true)
                        KzIconButton(.sidebar, label: "Sidebar", active: true, size: size) {}.disabled(true)
                    }
                }
            }
        }
    }
    @Test func activeFavoriteAndMenuStates() throws {
        try snapshot("iconbutton-variant-states", width: 400) {
            VStack(alignment: .leading, spacing: 16) {
                ForEach(KzButton.Size.allCases, id: \.self) { size in
                    ForEach(0..<5) { state in
                        HStack(spacing: 16) {
                            KzIconButton(.sidebar, label: "Sidebar", active: true, size: size) {}
                            KzIconButton(.starFill, label: "Favorite", size: size, tone: .bloom) {}
                            KzIconButton(.more, label: "More", caret: true, size: size) {}
                        }
                        .kzPreviewInteraction(hovered: state == 1, pressed: state == 2, focused: state == 3)
                        .disabled(state == 4)
                    }
                }
            }
        }
    }

    @Test func toolbarSizeAndMenuForm() throws {
        try snapshot("iconbutton-toolbar", width: 420) {
            HStack(spacing: 24) {
                KzToolbarGroup {
                    KzIconButton(.starFill, label: "Favorite", tone: .bloom) {}
                    KzIconButton(.bug, label: "Debug", caret: true) {}
                    KzToolbarSeparator()
                    KzIconButton(.more, label: "More", active: true) {}
                }
                KzIconButton(.sort, label: "Sort", caret: true, menuTitle: "Sort by") {
                    KzMenuItem("Name", isChecked: true) {}
                }
            }
        }
    }
}
