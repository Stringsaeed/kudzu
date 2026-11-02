import AppKit
@testable import KudzuUI
import SwiftUI
import Testing

@MainActor
@Suite struct FloatingPlacementTests {
    private let screen = CGRect(x: 0, y: 0, width: 1000, height: 800)
    private let size = CGSize(width: 200, height: 300)

    @Test func belowAnchorLeadingAligned() {
        let anchor = CGRect(x: 100, y: 500, width: 140, height: 30)
        let frame = KzFloatingPlacement.edge(.bottom, anchor: anchor, gap: 4).frame(for: size, in: screen)
        #expect(frame == CGRect(x: 100, y: 196, width: 200, height: 300))
    }

    @Test func flipsAboveNearTheBottom() {
        let anchor = CGRect(x: 100, y: 100, width: 140, height: 30)
        let frame = KzFloatingPlacement.edge(.bottom, anchor: anchor, gap: 4).frame(for: size, in: screen)
        #expect(frame.minY == 134)
    }

    @Test func staysBelowWhenNeitherSideFitsButBelowIsBigger() {
        let anchor = CGRect(x: 100, y: 450, width: 140, height: 30)
        let tall = CGSize(width: 200, height: 500)
        let frame = KzFloatingPlacement.edge(.bottom, anchor: anchor, gap: 4).frame(for: tall, in: screen)
        // Below has 446pt, above 316pt: it stays below and is pushed up to stay on screen.
        #expect(frame.minY == 4)
    }

    @Test func centredAndTrailingAlignment() {
        let anchor = CGRect(x: 400, y: 500, width: 100, height: 30)
        #expect(KzFloatingPlacement.edge(.bottom, anchor: anchor, alignment: .center).frame(for: size, in: screen).minX == 350)
        #expect(KzFloatingPlacement.edge(.bottom, anchor: anchor, alignment: .trailing).frame(for: size, in: screen).maxX == 500)
    }

    @Test func clampsToTheScreen() {
        let anchor = CGRect(x: 900, y: 500, width: 80, height: 30)
        let frame = KzFloatingPlacement.edge(.bottom, anchor: anchor).frame(for: size, in: screen)
        #expect(frame.maxX == 996)
    }

    @Test func sideEdgesFlip() {
        let anchor = CGRect(x: 850, y: 400, width: 100, height: 30)
        let frame = KzFloatingPlacement.edge(.trailing, anchor: anchor, gap: 6).frame(for: size, in: screen)
        #expect(frame.maxX == 844) // no room on the right: opens to the left
        #expect(frame.maxY == 430) // tops aligned
    }

    @Test func contextMenuAtPoint() {
        let frame = KzFloatingPlacement.point(CGPoint(x: 300, y: 600)).frame(for: size, in: screen)
        #expect(frame == CGRect(x: 300, y: 300, width: 200, height: 300))
        // Near the bottom-right corner it opens up and to the left of the pointer.
        let corner = KzFloatingPlacement.point(CGPoint(x: 900, y: 100)).frame(for: size, in: screen)
        #expect(corner == CGRect(x: 700, y: 100, width: 200, height: 300))
    }

    @Test func popUpAlignsTheCheckedItem() {
        // The checked item's label (34, 70 from the top-left) lands on the field's label point.
        let frame = KzFloatingPlacement.aligning(contentPoint: CGPoint(x: 34, y: 70), screenPoint: CGPoint(x: 210, y: 415))
            .frame(for: size, in: screen)
        #expect(frame.minX == 176)
        #expect(frame.maxY == 485)
    }

    @Test func submenuOpensRightOrLeft() {
        let parent = CGRect(x: 100, y: 300, width: 220, height: 200)
        let right = KzFloatingPlacement.submenu(parent: parent, top: 480).frame(for: size, in: screen)
        #expect(right.minX == 322)
        #expect(right.maxY == 480)
        let edgeParent = CGRect(x: 700, y: 300, width: 220, height: 200)
        let left = KzFloatingPlacement.submenu(parent: edgeParent, top: 480).frame(for: size, in: screen)
        #expect(left.maxX == 698)
    }

    @Test func centredOnScreen() {
        #expect(KzFloatingPlacement.centered().frame(for: size, in: screen) == CGRect(x: 400, y: 250, width: 200, height: 300))
        let palette = KzFloatingPlacement.centered(topFraction: 1.0 / 8).frame(for: size, in: screen)
        #expect(palette.maxY == 700)
    }

    @Test func shadowMarginsFitTheFloatShadow() {
        let margins = KzFloatingChrome.margins(for: .float)
        // shadow-float: 0 16px 40px -8px → reaches 48 below, 16 above, 32 to the sides (+4 slack).
        #expect(margins.bottom == 52)
        #expect(margins.top == 20)
        #expect(margins.left == 36)
        #expect(KzFloatingChrome.margins(for: nil).top == 0)
        let window = KzFloatingChrome.windowFrame(for: CGRect(x: 100, y: 100, width: 200, height: 100), margins: margins)
        #expect(window == CGRect(x: 64, y: 48, width: 272, height: 172))
    }
}

@MainActor
@Suite struct FloatingSnapshotTests {
    @Test func toolbarGroup() throws {
        try snapshot("toolbar-group", width: 420) {
            HStack(spacing: KzSpace.s6) {
                KzToolbarGroup {
                    KzIconButton(.star, label: "Favorite") {}
                    KzIconButton(.bug, label: "Debug") {}
                    KzToolbarSeparator()
                    KzIconButton(.more, label: "More") {}
                }
                KzToolbarGroup {
                    KzIconButton(.starFill, label: "Favorite", tone: .bloom) {}
                    KzIconButton(.bug, label: "Debug", active: true) {}
                }
            }
            .padding(KzSpace.s6)
            .background(KzColor.surfaceRaised)
        }
    }

    @Test func popoverSurface() throws {
        try snapshot("popover-surface", width: 420) {
            KzPopoverSurface {
                VStack(alignment: .leading, spacing: KzSpace.s2) {
                    Text("Date format").kzTextStyle(.headline).foregroundStyle(KzColor.ink)
                    Text("{date:%d %b %Y}").kzTextStyle(.code).foregroundStyle(KzColor.inkMuted)
                    HStack(spacing: KzSpace.s1) {
                        Text("Press").kzTextStyle(.callout).foregroundStyle(KzColor.inkMuted)
                        KzKbd("⎋")
                        Text("to close").kzTextStyle(.callout).foregroundStyle(KzColor.inkMuted)
                    }
                }
                .frame(width: 220, alignment: .leading)
            }
            .padding(.bottom, KzSpace.s8)
        }
    }

    @Test func floatingPanelSurface() throws {
        try snapshot("floating-panel-surface", width: 480) {
            KzFloatingPanelSurface {
                VStack(alignment: .leading, spacing: 0) {
                    HStack(spacing: KzSpace.s2) {
                        KzIcon(.search, size: 16).foregroundStyle(KzColor.inkMuted)
                        Text("Search snippets").kzTextStyle(.body).foregroundStyle(KzColor.inkFaint)
                        Spacer()
                        KzKbd("⎋")
                    }
                    .padding(KzSpace.s3)
                    Rectangle().fill(KzColor.line).frame(height: 1)
                    Text("Results").kzTextStyle(.caption).foregroundStyle(KzColor.inkMuted).padding(KzSpace.s3)
                }
                .frame(width: 380, height: 140, alignment: .topLeading)
            }
            .padding(.bottom, KzSpace.s8)
        }
    }
}
