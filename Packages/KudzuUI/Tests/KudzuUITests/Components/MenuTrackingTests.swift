import CoreGraphics
@testable import KudzuUI
import Testing

/// A root menu at x 100…308 and a submenu 2pt to its right, at x 310…518.
private let root = CGRect(x: 100, y: 100, width: 208, height: 200)
private let submenu = CGRect(x: 310, y: 150, width: 208, height: 120)

@MainActor
@Suite struct MenuHitTests {
    @Test func levelsWinAndTheDeepestIsOnTop() {
        #expect(KzMenuHit.locate(CGPoint(x: 150, y: 150), in: [root]) == .level(0))
        #expect(KzMenuHit.locate(CGPoint(x: 400, y: 200), in: [root, submenu]) == .level(1))
        let overlapping = CGRect(x: 250, y: 150, width: 208, height: 120)
        #expect(KzMenuHit.locate(CGPoint(x: 260, y: 200), in: [root, overlapping]) == .level(1))
    }

    /// The 2pt gap between a menu and its submenu is part of the way across, not outside.
    @Test func gapBetweenMenuAndSubmenuCountsAsInside() {
        #expect(KzMenuHit.locate(CGPoint(x: 308, y: 200), in: [root, submenu]) == .gap)
        #expect(KzMenuHit.locate(CGPoint(x: 309.5, y: 200), in: [root, submenu]) == .gap)
        // A submenu flipped to the left of its parent.
        let left = CGRect(x: 0, y: 150, width: 98, height: 120)
        #expect(KzMenuHit.locate(CGPoint(x: 99, y: 200), in: [root, left]) == .gap)
        // Beside the gap, above or below the submenu, is outside.
        #expect(KzMenuHit.locate(CGPoint(x: 309, y: 120), in: [root, submenu]) == .outside)
        #expect(KzMenuHit.locate(CGPoint(x: 309, y: 200), in: [root]) == .outside)
        #expect(KzMenuHit.locate(CGPoint(x: 600, y: 200), in: [root, submenu]) == .outside)
    }
}

@MainActor
@Suite struct MenuPointerRoutingTests {
    @Test func hoveringASubmenuItemSchedulesItToOpen() {
        var router = KzMenuPointerRouter(highlights: [nil], parentRows: [nil])
        #expect(router.pointerMoved(.level(0), row: 3, rowHasSubmenu: true, immediate: false)
            == .schedule(level: 0, row: 3, delay: KzMenuPointerRouter.openDelay))
        #expect(router.highlights == [3])
        #expect(router.pointerMoved(.level(0), row: 1, rowHasSubmenu: false, immediate: false) == .cancel)
        #expect(router.pointerMoved(.level(0), row: 3, rowHasSubmenu: true, immediate: true)
            == .schedule(level: 0, row: 3, delay: .zero))
    }

    /// Hover a submenu item, then leave the menu before it opens: it doesn't open.
    @Test func leavingTheMenuCancelsAPendingSubmenu() {
        var router = KzMenuPointerRouter(highlights: [nil], parentRows: [nil])
        _ = router.pointerMoved(.level(0), row: 3, rowHasSubmenu: true, immediate: false)
        #expect(router.pointerMoved(.outside, row: nil, rowHasSubmenu: false, immediate: false) == .cancel)
        #expect(router.highlights == [nil])
        // When the delayed change wakes, the row is no longer highlighted, so it does nothing.
        #expect(!router.isCurrent(level: 0, row: 3))
    }

    /// Crossing another parent item towards an open submenu, then leaving: the open submenu's item
    /// is highlighted again in the parent and the pending switch is dropped.
    @Test func leavingRestoresTheOpenSubmenusItem() {
        var router = KzMenuPointerRouter(highlights: [3, nil], parentRows: [nil, 3])
        #expect(router.pointerMoved(.level(0), row: 4, rowHasSubmenu: false, immediate: false)
            == .schedule(level: 0, row: 4, delay: KzMenuPointerRouter.switchDelay))
        #expect(router.pointerMoved(.outside, row: nil, rowHasSubmenu: false, immediate: false) == .cancel)
        #expect(router.highlights == [3, nil])
        #expect(!router.isCurrent(level: 0, row: 4))
    }

    @Test func gapKeepsEverything() {
        var router = KzMenuPointerRouter(highlights: [3, nil], parentRows: [nil, 3])
        #expect(router.pointerMoved(.level(0), row: 3, rowHasSubmenu: true, immediate: false) == .cancel)
        #expect(router.pointerMoved(.gap, row: nil, rowHasSubmenu: false, immediate: false) == .keep)
        #expect(router.highlights == [3, nil])
        #expect(router.pointerInside)
        #expect(router.pointerMoved(.level(1), row: 0, rowHasSubmenu: false, immediate: false) == .cancel)
        #expect(router.highlights == [3, 0])
    }

    /// Wandering outside with a keyboard highlight doesn't clear it.
    @Test func keyboardHighlightSurvivesThePointerOutside() {
        var router = KzMenuPointerRouter(highlights: [2], parentRows: [nil])
        #expect(router.pointerMoved(.outside, row: nil, rowHasSubmenu: false, immediate: false) == .cancel)
        #expect(router.highlights == [2])
    }

    @Test func enteringASubmenuKeepsItsParentHighlighted() {
        var router = KzMenuPointerRouter(highlights: [4, nil], parentRows: [nil, 3])
        _ = router.pointerMoved(.level(1), row: 1, rowHasSubmenu: false, immediate: false)
        #expect(router.highlights == [3, 1])
        #expect(router.isCurrent(level: 1, row: 1))
    }
}

@MainActor
@Suite struct MenuPressTrackingTests {
    private let origin = CGPoint(x: 10, y: 10)

    @Test func openingClickLeavesTheMenuOpen() {
        var tracker = KzMenuPressTracker(pressLocation: origin, openedAt: 0)
        #expect(tracker.mouseUp(insideMenu: false, at: 0.1) == .none)
        // A pop-up opens over its button: a quick click releases over the menu without choosing.
        var popUp = KzMenuPressTracker(pressLocation: origin, openedAt: 0)
        #expect(popUp.mouseUp(insideMenu: true, at: 0.1) == .none)
    }

    @Test func holdingThenReleasingOverAnItemChoosesIt() {
        var tracker = KzMenuPressTracker(pressLocation: origin, openedAt: 0)
        #expect(tracker.mouseUp(insideMenu: true, at: KzMenuPressTracker.holdToChooseDelay + 0.1) == .choose)
    }

    @Test func openingDragOntoAnItemChoosesIt() {
        var tracker = KzMenuPressTracker(pressLocation: origin, openedAt: 0)
        tracker.mouseDragged(to: CGPoint(x: 40, y: 60))
        #expect(tracker.mouseUp(insideMenu: true, at: 0.1) == .choose)
    }

    @Test func openingDragThatNeverEntersTheMenuCloses() {
        var tracker = KzMenuPressTracker(pressLocation: origin, openedAt: 0)
        tracker.mouseDragged(to: CGPoint(x: 10, y: 40))
        #expect(tracker.mouseUp(insideMenu: false, at: 0.1) == .close)
    }

    @Test func jitterIsNotADrag() {
        var tracker = KzMenuPressTracker(pressLocation: origin, openedAt: 0)
        tracker.mouseDragged(to: CGPoint(x: 12, y: 11))
        #expect(tracker.mouseUp(insideMenu: false, at: 0.1) == .none)
    }

    /// In a menu that's already open: press inside, drag out, release outside. Native tracking closes.
    @Test func pressInsideDragOutsideAndReleaseCloses() {
        var tracker = KzMenuPressTracker(pressLocation: origin, openedAt: 0)
        #expect(tracker.mouseUp(insideMenu: false, at: 0.1) == .none) // the opening click
        tracker.mouseDown(at: CGPoint(x: 150, y: 150), inside: true)
        tracker.mouseDragged(to: CGPoint(x: 400, y: 400))
        #expect(tracker.mouseUp(insideMenu: false, at: 2) == .close)
    }

    @Test func pressInsideAndReleaseInsideChooses() {
        var tracker = KzMenuPressTracker(pressLocation: origin, openedAt: 0)
        _ = tracker.mouseUp(insideMenu: false, at: 0.1)
        tracker.mouseDown(at: CGPoint(x: 150, y: 150), inside: true)
        #expect(tracker.mouseUp(insideMenu: true, at: 0.2) == .choose)
        // Each press starts over: the next release outside after a fresh in-place click closes too.
        tracker.mouseDown(at: CGPoint(x: 150, y: 150), inside: true)
        #expect(tracker.mouseUp(insideMenu: false, at: 0.3) == .close)
    }
}
