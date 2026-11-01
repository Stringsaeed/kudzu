import AppKit
import SwiftUI

/// How a menu was opened. It decides the first highlight, and whether a mouse-up can choose.
enum KzMenuTrigger {
    case mouse
    case keyboard

    /// Reads the event being handled right now: a key press means keyboard.
    @MainActor static var current: KzMenuTrigger {
        switch NSApp.currentEvent?.type {
        case .keyDown, .keyUp: .keyboard
        default: .mouse
        }
    }
}

/// Where a menu opens.
enum KzMenuAnchoring {
    /// Attached to a view's frame on screen.
    case anchor(CGRect, KzMenuPlacement)
    /// At a point on screen (context menus).
    case point(CGPoint)
}

/// The state SwiftUI draws for one menu level.
@MainActor @Observable
final class KzMenuLevelModel {
    var highlighted: Int?
    var flashOff = false
    var scrollOffset: CGFloat = 0
}

/// One open menu: its panels (the menu and any open submenus), event routing while it's open,
/// focus restoration and VoiceOver. Only one menu is open at a time, as on the Mac.
///
/// The menu never takes key focus from the window it was opened from (like NSMenu's tracking):
/// a local event monitor routes keys and clicks to it, so the window keeps its first responder.
/// When no window of ours is key (a right-click in an inactive window), the menu panel itself
/// becomes key, without activating the app, so the keyboard still works.
@MainActor
final class KzMenuSession: NSObject {
    /// The open menu, if any.
    private(set) static var current: KzMenuSession?

    @MainActor fileprivate final class Level {
        let menu: KzMenu
        let layout: KzMenuLayout
        var navigator: KzMenuNavigator
        let model = KzMenuLevelModel()
        let panel = KzMenuPanel()
        let container = KzMenuContainerView()
        let parentRow: Int?
        var glassFrame: CGRect = .zero

        init(menu: KzMenu, parentRow: Int?) {
            self.menu = menu
            self.layout = KzMenuLayout(menu)
            self.navigator = KzMenuNavigator(elements: menu.elements)
            self.parentRow = parentRow
        }

        func item(_ row: Int?) -> KzMenuItem? {
            guard let row, menu.elements.indices.contains(row) else { return nil }
            return menu.elements[row].item
        }
    }

    /// Release within this time of opening chooses the item under the pointer, even without moving.
    static let holdToChooseDelay: TimeInterval = 0.4
    /// Hovering a submenu item this long opens it.
    static let submenuOpenDelay: Duration = .milliseconds(120)
    /// Leaving an open submenu's item for another this long switches submenus (time to cross over).
    static let submenuSwitchDelay: Duration = .milliseconds(250)

    private let root: KzMenu
    private var levels: [Level] = []
    private weak var parentWindow: NSWindow?
    private let focus: KzFocusMemory
    private let trigger: KzMenuTrigger
    private let onClose: @MainActor () -> Void
    private var localMonitor: Any?
    private var globalMonitor: Any?
    private let openedAt = ProcessInfo.processInfo.systemUptime
    private let openLocation = NSEvent.mouseLocation
    private var pointerHasMoved = false
    private var pointerInside = false
    private var dragged = false
    private var pressedInside = false
    private var isClosed = false
    private var isActivating = false
    private var submenuTask: Task<Void, Never>?
    private static let margins = KzFloatingChrome.margins(for: .float)

    private init(menu: KzMenu, parentWindow: NSWindow?, trigger: KzMenuTrigger, onClose: @escaping @MainActor () -> Void) {
        self.root = menu
        self.parentWindow = parentWindow
        self.focus = KzFocusMemory.capture()
        self.trigger = trigger
        self.onClose = onClose
        super.init()
    }

    /// Opens `menu`, closing any menu that's already open.
    @discardableResult
    static func present(_ menu: KzMenu, at anchoring: KzMenuAnchoring, parentWindow: NSWindow?, trigger: KzMenuTrigger,
                        onClose: @escaping @MainActor () -> Void) -> KzMenuSession {
        current?.close()
        let session = KzMenuSession(menu: menu, parentWindow: parentWindow, trigger: trigger, onClose: onClose)
        current = session
        session.open(at: anchoring)
        return session
    }

    /// True while the menu is on screen.
    var isOpen: Bool { !isClosed }

    // MARK: - Opening

    private func open(at anchoring: KzMenuAnchoring) {
        let level = Level(menu: root, parentRow: nil)
        let reference: CGPoint
        switch anchoring {
        case .anchor(let rect, _): reference = CGPoint(x: rect.midX, y: rect.midY)
        case .point(let point): reference = point
        }
        let visible = KzFloatingChrome.visibleFrame(containing: reference)
        let size = Self.glassSize(for: level, visible: visible)

        var initialHighlight: Int?
        let placement: KzFloatingPlacement
        switch anchoring {
        case .point(let point):
            placement = .point(point)
        case .anchor(let rect, .pullDown(let alignment)):
            placement = .edge(.bottom, anchor: rect, gap: 4, alignment: alignment)
        case .anchor(let rect, .popUp(let labelInset)):
            let checked = root.elements.firstIndex { $0.item?.isChecked == true && $0.item?.isEnabled == true }
            if let checked, level.layout.contentHeight <= size.height {
                let content = CGPoint(x: KzMenuLayout.labelInset, y: level.layout.top(of: checked) + KzMenuLayout.itemHeight / 2)
                placement = .aligning(contentPoint: content, screenPoint: CGPoint(x: rect.minX + labelInset, y: rect.midY))
            } else {
                placement = .edge(.bottom, anchor: rect, gap: 4, alignment: .leading)
            }
            initialHighlight = checked
        }
        if initialHighlight == nil, trigger == .keyboard {
            initialHighlight = level.navigator.selectableRows.first
        }

        level.glassFrame = placement.frame(for: size, in: visible)
        levels = [level]
        show(level, size: size)
        setHighlight(initialHighlight, in: 0)

        installMonitors()
        if NSApp.keyWindow == nil {
            level.panel.allowsKey = true
            level.panel.makeKey()
        }
        NSAccessibility.post(element: level.container, notification: Self.menuOpened)
    }

    /// The glass size: the rows' ideal width, and their height up to the screen's.
    private static func glassSize(for level: Level, visible: CGRect) -> CGSize {
        let measure = NSHostingView(rootView: KzMenuRows(menu: level.menu, highlighted: nil).fixedSize())
        let width = ceil(measure.fittingSize.width)
        let maxHeight = visible.height - KzFloatingPlacement.screenMargin * 2
        return CGSize(width: max(width, KzMenuLayout.minWidth), height: min(level.layout.contentHeight, maxHeight))
    }

    private func show(_ level: Level, size: CGSize) {
        let margins = Self.margins
        let panel = level.panel
        let windowFrame = KzFloatingChrome.windowFrame(for: level.glassFrame, margins: margins)
        panel.appearance = parentWindow?.effectiveAppearance
        panel.setFrame(windowFrame, display: false)

        let container = level.container
        container.frame = CGRect(origin: .zero, size: windowFrame.size)
        container.session = self
        container.level = levels.firstIndex { $0 === level } ?? 0
        container.title = level.menu.title
        container.configureItems(level.menu)
        let host = NSHostingView(rootView: KzMenuLevelView(menu: level.menu, model: level.model, size: size, margins: margins))
        host.sizingOptions = []
        host.frame = container.bounds
        host.autoresizingMask = [.width, .height]
        container.subviews = [host]
        panel.contentView = container

        if let parentWindow {
            parentWindow.addChildWindow(panel, ordered: .above)
        }
        panel.level = .popUpMenu
        panel.orderFront(nil)
        KzFloatingChrome.fadeIn(panel)
    }

    // MARK: - Closing

    /// Closes the menu (and its submenus) without running anything.
    func close() {
        close(restoringFocus: true)
    }

    private func close(restoringFocus: Bool) {
        guard !isClosed else { return }
        isClosed = true
        submenuTask?.cancel()
        removeMonitors()
        for level in levels.reversed() {
            NSAccessibility.post(element: level.container, notification: Self.menuClosed)
            level.panel.parent?.removeChildWindow(level.panel)
            level.panel.allowsKey = false
            KzFloatingChrome.fadeOut(level.panel)
        }
        if Self.current === self { Self.current = nil }
        if restoringFocus { focus.restore() }
        onClose()
    }

    private func closeLevels(after index: Int) {
        while levels.count > index + 1 {
            let level = levels.removeLast()
            level.panel.parent?.removeChildWindow(level.panel)
            level.panel.orderOut(nil)
        }
    }

    // MARK: - Monitors

    private func installMonitors() {
        let mask: NSEvent.EventTypeMask = [
            .keyDown, .leftMouseDown, .rightMouseDown, .otherMouseDown, .leftMouseUp, .rightMouseUp, .otherMouseUp,
            .leftMouseDragged, .rightMouseDragged, .otherMouseDragged, .mouseMoved, .scrollWheel,
        ]
        localMonitor = NSEvent.addLocalMonitorForEvents(matching: mask) { [weak self] event in
            guard let self else { return event }
            return self.handle(event)
        }
        // Clicks in other apps while Kudzu isn't active (a palette or an inactive window).
        globalMonitor = NSEvent.addGlobalMonitorForEvents(matching: [.leftMouseDown, .rightMouseDown, .otherMouseDown]) { [weak self] _ in
            self?.close(restoringFocus: false)
        }

        let center = NotificationCenter.default
        center.addObserver(self, selector: #selector(environmentChanged), name: NSApplication.didResignActiveNotification, object: nil)
        center.addObserver(self, selector: #selector(environmentChanged), name: NSApplication.didChangeScreenParametersNotification, object: nil)
        if let parentWindow {
            for name in [NSWindow.didMoveNotification, NSWindow.didResizeNotification, NSWindow.willCloseNotification,
                         NSWindow.didMiniaturizeNotification, NSWindow.didChangeScreenNotification] {
                center.addObserver(self, selector: #selector(environmentChanged), name: name, object: parentWindow)
            }
        }
        center.addObserver(self, selector: #selector(windowDidResignKey(_:)), name: NSWindow.didResignKeyNotification, object: nil)
        center.addObserver(self, selector: #selector(windowDidBecomeKey(_:)), name: NSWindow.didBecomeKeyNotification, object: nil)
        let workspace = NSWorkspace.shared.notificationCenter
        workspace.addObserver(self, selector: #selector(applicationActivated(_:)), name: NSWorkspace.didActivateApplicationNotification, object: nil)
        workspace.addObserver(self, selector: #selector(environmentChanged), name: NSWorkspace.activeSpaceDidChangeNotification, object: nil)
    }

    private func removeMonitors() {
        if let localMonitor { NSEvent.removeMonitor(localMonitor) }
        if let globalMonitor { NSEvent.removeMonitor(globalMonitor) }
        localMonitor = nil
        globalMonitor = nil
        NotificationCenter.default.removeObserver(self)
        NSWorkspace.shared.notificationCenter.removeObserver(self)
    }

    // When focus moves elsewhere (another window or app), the menu closes without pulling focus back.

    @objc private func environmentChanged() {
        close(restoringFocus: false)
    }

    @objc private func windowDidResignKey(_ notification: Notification) {
        guard let window = notification.object as? NSWindow, window === focus.window else { return }
        close(restoringFocus: false)
    }

    @objc private func windowDidBecomeKey(_ notification: Notification) {
        guard let window = notification.object as? NSWindow else { return }
        if window === focus.window || levels.contains(where: { $0.panel === window }) { return }
        close(restoringFocus: false)
    }

    @objc private func applicationActivated(_ notification: Notification) {
        let app = notification.userInfo?[NSWorkspace.applicationUserInfoKey] as? NSRunningApplication
        if app?.processIdentifier != ProcessInfo.processInfo.processIdentifier { close(restoringFocus: false) }
    }

    // MARK: - Events

    private func handle(_ event: NSEvent) -> NSEvent? {
        guard !isClosed else { return event }
        switch event.type {
        case .keyDown:
            handleKey(event)
            return nil
        case .leftMouseDown, .rightMouseDown, .otherMouseDown:
            let point = Self.screenPoint(of: event)
            guard levelIndex(containing: point) != nil else {
                // Like NSMenu, the click that dismisses the menu goes nowhere else.
                close()
                return nil
            }
            pressedInside = true
            pointerMoved(to: point, immediate: true)
            return nil
        case .leftMouseUp, .rightMouseUp, .otherMouseUp:
            mouseUp(at: Self.screenPoint(of: event))
            return nil
        case .leftMouseDragged, .rightMouseDragged, .otherMouseDragged:
            let point = Self.screenPoint(of: event)
            if hypot(point.x - openLocation.x, point.y - openLocation.y) > 3 { dragged = true }
            pointerMoved(to: point)
            return nil
        case .mouseMoved:
            pointerMoved(to: Self.screenPoint(of: event))
            return event
        case .scrollWheel:
            let point = Self.screenPoint(of: event)
            if let index = levelIndex(containing: point) {
                let delta = event.hasPreciseScrollingDeltas ? event.scrollingDeltaY : event.scrollingDeltaY * KzMenuLayout.itemHeight
                scroll(index, by: -delta)
            }
            return nil
        default:
            return event
        }
    }

    private func mouseUp(at point: CGPoint) {
        guard !isActivating else { return }
        guard let index = levelIndex(containing: point) else {
            // Pressed on the trigger, dragged off and let go somewhere else: nothing chosen.
            if dragged && !pressedInside { close() }
            return
        }
        let elapsed = ProcessInfo.processInfo.systemUptime - openedAt
        let chooses = pressedInside || dragged || elapsed > Self.holdToChooseDelay
        guard chooses, let row = row(at: point, in: levels[index]) else { return }
        activateRow(row, in: index)
    }

    /// Hover and drag: highlight the item under the pointer and open or switch submenus.
    func pointerMoved(to point: CGPoint, immediate: Bool = false) {
        guard !isClosed, !isActivating else { return }
        if hypot(point.x - openLocation.x, point.y - openLocation.y) > 3 { pointerHasMoved = true }
        guard pointerHasMoved || immediate else { return }
        guard let index = levelIndex(containing: point) else {
            // Leaving the menus: the deepest one loses its highlight (a parent keeps the item whose
            // submenu is open). A keyboard highlight survives the pointer moving around outside.
            if pointerInside { setHighlight(nil, in: levels.count - 1) }
            pointerInside = false
            return
        }
        pointerInside = true
        let level = levels[index]
        let row = row(at: point, in: level).flatMap { level.navigator.isSelectable($0) ? $0 : nil }
        if index < levels.count - 1 {
            // Back in a parent while its submenu is open. Give the pointer time to cross over before switching.
            if row == levels[index + 1].parentRow {
                submenuTask?.cancel()
                setHighlight(row, in: index)
                return
            }
            setHighlight(row, in: index)
            scheduleSubmenuChange(level: index, row: row, delay: immediate ? .zero : Self.submenuSwitchDelay)
        } else {
            if index > 0, let parentRow = level.parentRow { setHighlight(parentRow, in: index - 1) }
            setHighlight(row, in: index)
            if let row, level.item(row)?.hasSubmenu == true {
                scheduleSubmenuChange(level: index, row: row, delay: immediate ? .zero : Self.submenuOpenDelay)
            } else {
                submenuTask?.cancel()
            }
        }
    }

    private func scheduleSubmenuChange(level index: Int, row: Int?, delay: Duration) {
        submenuTask?.cancel()
        submenuTask = Task { [weak self] in
            if delay > .zero { try? await Task.sleep(for: delay) }
            guard !Task.isCancelled, let self, !self.isClosed, index < self.levels.count else { return }
            if let row, self.levels[index].item(row)?.hasSubmenu == true {
                self.openSubmenu(row, in: index, highlightFirst: false)
            } else {
                self.closeLevels(after: index)
            }
        }
    }

    private func scroll(_ index: Int, by delta: CGFloat) {
        let level = levels[index]
        let maxOffset = max(0, level.layout.contentHeight - level.glassFrame.height)
        level.model.scrollOffset = min(max(0, level.model.scrollOffset + delta), maxOffset)
    }

    // MARK: - Keys

    private func handleKey(_ event: NSEvent) {
        guard !isActivating, !levels.isEmpty else { return }
        let key = KzMenuKey.decode(keyCode: event.keyCode, charactersIgnoringModifiers: event.charactersIgnoringModifiers,
                                   modifiers: Self.modifiers(event.modifierFlags))
        if case .shortcut(let characters, let modifiers) = key {
            let match = root.allItems.first {
                $0.isEnabled && !$0.hasSubmenu && $0.shortcut?.matches(charactersIgnoringModifiers: characters, modifiers: modifiers) == true
            }
            if let match { activate(match, flashing: location(of: match)) }
            return
        }
        let index = levels.lastIndex { $0.navigator.highlighted != nil } ?? levels.count - 1
        let command = levels[index].navigator.handle(key, at: ProcessInfo.processInfo.systemUptime)
        switch command {
        case .none:
            break
        case .highlight:
            closeLevels(after: index)
            setHighlight(levels[index].navigator.highlighted, in: index)
        case .activate(let row):
            activateRow(row, in: index)
        case .openSubmenu(let row):
            openSubmenu(row, in: index, highlightFirst: true)
        case .closeSubmenu:
            if index > 0 { closeLevels(after: index - 1) } else if levels.count > 1 { closeLevels(after: 0) }
        case .dismiss:
            close()
        }
    }

    // MARK: - Highlight and submenus

    private func setHighlight(_ row: Int?, in index: Int) {
        guard levels.indices.contains(index) else { return }
        let level = levels[index]
        level.navigator.highlighted = row
        guard level.model.highlighted != row else { return }
        level.model.highlighted = row
        if let row {
            level.model.scrollOffset = level.layout.scrollOffset(revealing: row, viewportHeight: level.glassFrame.height,
                                                                 currentOffset: level.model.scrollOffset)
        }
        level.container.highlightChanged(to: row)
    }

    private func openSubmenu(_ row: Int, in index: Int, highlightFirst: Bool) {
        guard levels.indices.contains(index), let item = levels[index].item(row), item.isEnabled, let submenu = item.submenu else { return }
        if levels.count > index + 1, levels[index + 1].parentRow == row {
            if highlightFirst { setHighlight(levels[index + 1].navigator.selectableRows.first, in: index + 1) }
            return
        }
        closeLevels(after: index)
        setHighlight(row, in: index)
        let parent = levels[index]
        let level = Level(menu: submenu, parentRow: row)
        let visible = KzFloatingChrome.visibleFrame(containing: CGPoint(x: parent.glassFrame.midX, y: parent.glassFrame.midY))
        let size = Self.glassSize(for: level, visible: visible)
        let itemTop = parent.glassFrame.maxY - (parent.layout.top(of: row) - parent.model.scrollOffset)
        level.glassFrame = KzFloatingPlacement.submenu(parent: parent.glassFrame, top: itemTop + KzMenuLayout.padding)
            .frame(for: size, in: visible)
        levels.append(level)
        show(level, size: size)
        if highlightFirst { setHighlight(level.navigator.selectableRows.first, in: levels.count - 1) }
        NSAccessibility.post(element: level.container, notification: Self.menuOpened)
    }

    // MARK: - Activation

    private func activateRow(_ row: Int, in index: Int) {
        guard let item = levels[index].item(row), item.isEnabled else { return }
        if item.hasSubmenu {
            openSubmenu(row, in: index, highlightFirst: false)
        } else {
            activate(item, flashing: (index, row))
        }
    }

    /// Activates from VoiceOver's press action.
    func accessibilityPress(row: Int, level index: Int) -> Bool {
        guard levels.indices.contains(index), let item = levels[index].item(row), item.isEnabled else { return false }
        if item.hasSubmenu {
            openSubmenu(row, in: index, highlightFirst: true)
        } else {
            activate(item, flashing: (index, row))
        }
        return true
    }

    private func activate(_ item: KzMenuItem, flashing location: (level: Int, row: Int)?) {
        guard !isActivating else { return }
        isActivating = true
        submenuTask?.cancel()
        if let location { NSAccessibility.post(element: levels[location.level].container, notification: Self.menuItemSelected) }
        let action = item.action
        KzMenuActivation.run(
            flash: { done in self.flash(location, done: done) },
            close: { self.close(restoringFocus: false) },
            restoreFocus: { self.focus.restore() },
            perform: {
                // After SwiftUI has seen the menu close.
                Task { @MainActor in action() }
            }
        )
    }

    /// NSMenu's confirmation blink: the highlight goes off and on once.
    private func flash(_ location: (level: Int, row: Int)?, done: @escaping @MainActor () -> Void) {
        guard let location, levels.indices.contains(location.level) else { return done() }
        let model = levels[location.level].model
        model.highlighted = location.row
        model.flashOff = true
        Task { @MainActor in
            try? await Task.sleep(for: .milliseconds(50))
            model.flashOff = false
            try? await Task.sleep(for: .milliseconds(60))
            done()
        }
    }

    private func location(of item: KzMenuItem) -> (level: Int, row: Int)? {
        for (index, level) in levels.enumerated() {
            if let row = level.menu.elements.firstIndex(where: { $0.item?.id == item.id }) { return (index, row) }
        }
        return nil
    }

    // MARK: - Geometry

    private func levelIndex(containing point: CGPoint) -> Int? {
        levels.lastIndex { $0.glassFrame.contains(point) }
    }

    private func row(at point: CGPoint, in level: Level) -> Int? {
        let local = CGPoint(x: point.x - level.glassFrame.minX, y: level.glassFrame.maxY - point.y + level.model.scrollOffset)
        return level.layout.row(at: local, width: level.glassFrame.width)
    }

    /// The screen frame of a row, for VoiceOver.
    func screenFrame(row: Int, level index: Int) -> CGRect {
        guard levels.indices.contains(index) else { return .zero }
        let level = levels[index]
        let frame = level.layout.frame(of: row, width: level.glassFrame.width)
        return CGRect(x: level.glassFrame.minX + frame.minX,
                      y: level.glassFrame.maxY - (frame.maxY - level.model.scrollOffset),
                      width: frame.width, height: frame.height)
    }

    private static func screenPoint(of event: NSEvent) -> CGPoint {
        guard let window = event.window else { return event.locationInWindow }
        return window.convertPoint(toScreen: event.locationInWindow)
    }

    static func modifiers(_ flags: NSEvent.ModifierFlags) -> EventModifiers {
        var modifiers: EventModifiers = []
        if flags.contains(.command) { modifiers.insert(.command) }
        if flags.contains(.option) { modifiers.insert(.option) }
        if flags.contains(.control) { modifiers.insert(.control) }
        if flags.contains(.shift) { modifiers.insert(.shift) }
        return modifiers
    }

    static let menuOpened = NSAccessibility.Notification(rawValue: "AXMenuOpened")
    static let menuClosed = NSAccessibility.Notification(rawValue: "AXMenuClosed")
    static let menuItemSelected = NSAccessibility.Notification(rawValue: "AXMenuItemSelected")
}

// MARK: - Panel and views

/// A borderless, non-activating panel for one menu level, above everything like a native menu.
final class KzMenuPanel: NSPanel {
    /// Only becomes key when the session needs the keyboard and no window of ours is key.
    var allowsKey = false

    init() {
        super.init(contentRect: .zero, styleMask: [.borderless, .nonactivatingPanel], backing: .buffered, defer: true)
        isFloatingPanel = true
        level = .popUpMenu
        hidesOnDeactivate = false
        isReleasedWhenClosed = false
        isOpaque = false
        backgroundColor = .clear
        hasShadow = false
        animationBehavior = .none
        collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary, .transient, .ignoresCycle]
        becomesKeyOnlyIfNeeded = true
    }

    override var canBecomeKey: Bool { allowsKey }
    override var canBecomeMain: Bool { false }
}

/// The SwiftUI content of one menu level: the rows on glass, scrolled and clipped to the viewport.
private struct KzMenuLevelView: View {
    let menu: KzMenu
    let model: KzMenuLevelModel
    let size: CGSize
    let margins: NSEdgeInsets

    var body: some View {
        let shape = RoundedRectangle(cornerRadius: KzRadius.lg)
        KzMenuRows(menu: menu, highlighted: model.flashOff ? nil : model.highlighted)
            .frame(width: size.width)
            .fixedSize(horizontal: false, vertical: true)
            .offset(y: -model.scrollOffset)
            .frame(width: size.width, height: size.height, alignment: .top)
            .clipShape(shape)
            .kzGlass(.strong, in: shape, blending: .behindWindow, shadow: .float)
            .padding(EdgeInsets(top: margins.top, leading: margins.left, bottom: margins.bottom, trailing: margins.right))
            .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
            .accessibilityHidden(true)
    }
}

/// The AppKit container of a menu level: VoiceOver's menu (role, items, check states and
/// shortcuts) and hover tracking that works whether or not the app is active.
final class KzMenuContainerView: NSView {
    weak var session: KzMenuSession?
    var level = 0
    var title: String?
    private var elements: [Int: KzMenuItemAccessibilityElement] = [:]
    private var order: [Int] = []
    private var trackingArea: NSTrackingArea?

    func configureItems(_ menu: KzMenu) {
        elements = [:]
        order = []
        for (row, element) in menu.elements.enumerated() {
            guard let item = element.item else { continue }
            let accessibility = KzMenuItemAccessibilityElement()
            accessibility.container = self
            accessibility.row = row
            accessibility.setAccessibilityRole(.menuItem)
            accessibility.setAccessibilityParent(self)
            accessibility.setAccessibilityLabel(item.label)
            accessibility.setAccessibilityEnabled(item.isEnabled)
            if let checked = item.isChecked {
                accessibility.setAccessibilityValue(checked ? "checked" : "unchecked")
            }
            var help: [String] = []
            if item.hasSubmenu { help.append("submenu") }
            if let shortcut = item.shortcut { help.append(shortcut.kzSpokenName) }
            if !help.isEmpty { accessibility.setAccessibilityHelp(help.joined(separator: ", ")) }
            elements[row] = accessibility
            order.append(row)
        }
    }

    func highlightChanged(to row: Int?) {
        for (index, element) in elements { element.setAccessibilitySelected(index == row) }
        NSAccessibility.post(element: self, notification: .selectedChildrenChanged)
        if let row, let element = elements[row] {
            NSAccessibility.post(element: element, notification: .focusedUIElementChanged)
        }
    }

    func screenFrame(of row: Int) -> CGRect {
        session?.screenFrame(row: row, level: level) ?? .zero
    }

    func press(row: Int) -> Bool {
        session?.accessibilityPress(row: row, level: level) ?? false
    }

    // Accessibility: the container is the menu; the hosted SwiftUI is hidden.
    override func isAccessibilityElement() -> Bool { true }
    override func accessibilityRole() -> NSAccessibility.Role? { .menu }
    override func accessibilityLabel() -> String? { title }
    override func accessibilityChildren() -> [Any]? { order.compactMap { elements[$0] } }
    override func accessibilitySelectedChildren() -> [Any]? {
        elements.values.filter { $0.isAccessibilitySelected() }
    }
    override func accessibilityPerformCancel() -> Bool {
        session?.close()
        return true
    }

    // Hover, including while the app is inactive.
    override func updateTrackingAreas() {
        super.updateTrackingAreas()
        if let trackingArea { removeTrackingArea(trackingArea) }
        let area = NSTrackingArea(rect: .zero, options: [.mouseMoved, .mouseEnteredAndExited, .activeAlways, .inVisibleRect], owner: self)
        addTrackingArea(area)
        trackingArea = area
    }

    override func mouseMoved(with event: NSEvent) { track(event) }
    override func mouseEntered(with event: NSEvent) { track(event) }
    override func mouseExited(with event: NSEvent) { track(event) }
    override func acceptsFirstMouse(for event: NSEvent?) -> Bool { true }

    private func track(_ event: NSEvent) {
        guard let window else { return }
        session?.pointerMoved(to: window.convertPoint(toScreen: event.locationInWindow))
    }
}

/// A menu item as VoiceOver sees it.
final class KzMenuItemAccessibilityElement: NSAccessibilityElement {
    weak var container: KzMenuContainerView?
    var row = 0

    // VoiceOver calls these on the main thread.
    override func accessibilityFrame() -> NSRect {
        let container = container, row = row
        return MainActor.assumeIsolated { container?.screenFrame(of: row) ?? .zero }
    }

    override func accessibilityPerformPress() -> Bool {
        let container = container, row = row
        return MainActor.assumeIsolated { container?.press(row: row) ?? false }
    }
}
