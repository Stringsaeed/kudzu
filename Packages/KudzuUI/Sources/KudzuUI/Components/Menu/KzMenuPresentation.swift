import AppKit
import Combine
import SwiftUI

/// How a menu attaches to the view that opens it.
public enum KzMenuPlacement: Equatable, Sendable {
    /// A pull-down: below the view, aligned to its leading (or trailing) edge, flipping above near
    /// the bottom of the screen. Sort, Insert tag and "more" menus.
    case pullDown(KzFloatingPlacement.Alignment)
    /// A pop-up, like `NSPopUpButton`: the checked item's label sits exactly over the view's label,
    /// which starts `labelInset` points in from the view's leading edge. Select uses this. Without
    /// a checked item (or when the menu is taller than the screen) it's a pull-down.
    case popUp(labelInset: CGFloat)

    /// A pull-down aligned to the leading edge.
    public static let below = KzMenuPlacement.pullDown(.leading)
    /// A pop-up for `Select` (its label is inset 10pt).
    public static let select = KzMenuPlacement.popUp(labelInset: 10)
}

/// Owns the anchor view behind a SwiftUI view and the menu it opened.
@MainActor
final class KzMenuAnchorController: ObservableObject {
    let anchor = KzAnchorNSView()
    private weak var session: KzMenuSession?
    private var mouseDownMonitor: Any?
    /// Set by `KzMenuButton`: opens the menu on mouse-down, so press-drag-release works.
    var onMouseDown: (@MainActor () -> Void)?

    init() {
        anchor.windowDidChange = { [weak self] window in
            MainActor.assumeIsolated { self?.windowChanged(window) }
        }
    }

    var isOpen: Bool { session?.isOpen ?? false }

    func present(_ menu: KzMenu, placement: KzMenuPlacement, trigger: KzMenuTrigger, onClose: @escaping @MainActor () -> Void) {
        guard let frame = anchor.screenFrame else {
            onClose()
            return
        }
        session = KzMenuSession.present(menu, at: .anchor(frame, placement), parentWindow: anchor.window, trigger: trigger, onClose: onClose)
    }

    func dismiss() {
        session?.close()
    }

    private func windowChanged(_ window: NSWindow?) {
        if let mouseDownMonitor { NSEvent.removeMonitor(mouseDownMonitor) }
        mouseDownMonitor = nil
        guard window != nil else {
            dismiss()
            return
        }
        mouseDownMonitor = NSEvent.addLocalMonitorForEvents(matching: .leftMouseDown) { [weak self] event in
            guard let self, let onMouseDown = self.onMouseDown, KzMenuSession.current == nil, self.anchor.isEnabled,
                  event.modifierFlags.intersection([.command, .control, .option, .shift]).isEmpty,
                  self.anchor.contains(event) else { return event }
            onMouseDown()
            // The button never sees this press, so it doesn't also run its action on mouse-up.
            return nil
        }
    }
}

// MARK: - .kzMenu(isPresented:)

private struct KzMenuModifier: ViewModifier {
    @Binding var isPresented: Bool
    let placement: KzMenuPlacement
    let menu: () -> KzMenu
    @StateObject private var controller = KzMenuAnchorController()
    @Environment(\.isEnabled) private var isEnabled

    func body(content: Content) -> some View {
        content
            .background(KzAnchorBackground(view: controller.anchor, isEnabled: isEnabled))
            .onChange(of: isPresented, initial: true) { _, open in
                if open {
                    present()
                } else {
                    controller.dismiss()
                }
            }
            .onDisappear { controller.dismiss() }
    }

    private func present() {
        guard !controller.isOpen else { return }
        let binding = $isPresented
        controller.present(menu(), placement: placement, trigger: .current) {
            if binding.wrappedValue { binding.wrappedValue = false }
        }
    }
}

extension View {
    /// Shows a Kudzu menu attached to this view while `isPresented` is true; it's set back to false
    /// when the menu closes. The items are built when the menu opens.
    ///
    /// Opening from a button's action means the menu appears on mouse-up. Use `KzMenuButton` for
    /// controls that should open on mouse-down and support press-drag-release, like native pop-ups.
    public func kzMenu(isPresented: Binding<Bool>, title: String? = nil, placement: KzMenuPlacement = .below,
                       @KzMenuBuilder content: @escaping () -> [KzMenuElement]) -> some View {
        modifier(KzMenuModifier(isPresented: isPresented, placement: placement, menu: { KzMenu(title: title, elements: content()) }))
    }
}

// MARK: - KzMenuButton

/// A button that opens a Kudzu menu, like `NSPopUpButton`: the menu opens on mouse-down, so you
/// can press, drag to an item and release; a click leaves it open; Space opens it from the keyboard
/// with the first (or checked) item highlighted.
///
/// Style the button as usual; the label gets `isOpen` to draw its open state. `KzSelect`,
/// `KzIconButton(…menu:)` and `KzButton(…menu:)` are built on this; use it directly for other labels:
///
/// ```swift
/// KzMenuButton(placement: .select) {
///     for option in options { KzMenuItem(option.name, isChecked: option == value) { value = option } }
/// } label: { isOpen in
///     KzSelectLabel(value.name, isOpen: isOpen)
/// }
/// .buttonStyle(KzSelectStyle())
/// ```
public struct KzMenuButton<Label: View>: View {
    private let title: String?
    private let placement: KzMenuPlacement
    private let content: () -> [KzMenuElement]
    private let label: (Bool) -> Label
    @StateObject private var controller = KzMenuAnchorController()
    @State private var isOpen = false
    @Environment(\.isEnabled) private var isEnabled

    /// Creates a menu button. `label` receives whether the menu is open.
    public init(title: String? = nil, placement: KzMenuPlacement = .below,
                @KzMenuBuilder menu: @escaping () -> [KzMenuElement],
                @ViewBuilder label: @escaping (_ isOpen: Bool) -> Label) {
        self.title = title
        self.placement = placement
        self.content = menu
        self.label = label
    }

    public var body: some View {
        Button {
            if controller.isOpen { controller.dismiss() } else { open(trigger: .current) }
        } label: {
            label(isOpen)
        }
        .background(KzAnchorBackground(view: controller.anchor, isEnabled: isEnabled) {
            controller.onMouseDown = { open(trigger: .mouse) }
        })
        .onDisappear { controller.dismiss() }
    }

    private func open(trigger: KzMenuTrigger) {
        let isOpen = $isOpen
        isOpen.wrappedValue = true
        controller.present(KzMenu(title: title, elements: content()), placement: placement, trigger: trigger) {
            isOpen.wrappedValue = false
        }
    }
}

// MARK: - Context menus

/// One `.kzContextMenu` region.
@MainActor
final class KzContextMenuRegion: ObservableObject {
    let anchor = KzAnchorNSView()
    var makeMenu: (@MainActor () -> KzMenu)?
    var isPresented: Binding<Bool>?
    private weak var session: KzMenuSession?

    init() {
        anchor.windowDidChange = { [weak self] window in
            MainActor.assumeIsolated {
                guard let self else { return }
                if window == nil {
                    KzContextMenuRegistry.shared.remove(self)
                    self.session?.close()
                } else {
                    KzContextMenuRegistry.shared.add(self)
                }
            }
        }
    }

    func open(at point: CGPoint) {
        guard let makeMenu else { return }
        let binding = isPresented
        binding?.wrappedValue = true
        session = KzMenuSession.present(makeMenu(), at: .point(point), parentWindow: anchor.window, trigger: .mouse) {
            binding?.wrappedValue = false
        }
    }
}

/// Routes right-clicks and Control-clicks to the innermost `.kzContextMenu` region under the pointer.
///
/// A local event monitor sees the click before any view, so left clicks, selection and dragging
/// in the region are untouched. Text views and fields keep their own (native) context menu.
@MainActor
final class KzContextMenuRegistry {
    static let shared = KzContextMenuRegistry()

    private struct Entry {
        weak var region: KzContextMenuRegion?
    }

    private var entries: [Entry] = []
    private var monitor: Any?

    func add(_ region: KzContextMenuRegion) {
        entries.removeAll { $0.region == nil || $0.region === region }
        entries.append(Entry(region: region))
        if monitor == nil {
            monitor = NSEvent.addLocalMonitorForEvents(matching: [.rightMouseDown, .leftMouseDown]) { [weak self] event in
                self?.handle(event) ?? event
            }
        }
    }

    func remove(_ region: KzContextMenuRegion) {
        entries.removeAll { $0.region == nil || $0.region === region }
        if entries.isEmpty, let monitor {
            NSEvent.removeMonitor(monitor)
            self.monitor = nil
        }
    }

    private func handle(_ event: NSEvent) -> NSEvent? {
        let isContextClick = event.type == .rightMouseDown
            || (event.type == .leftMouseDown && event.modifierFlags.contains(.control))
        guard isContextClick, KzMenuSession.current == nil, let window = event.window,
              let region = region(at: event.locationInWindow, in: window) else { return event }
        region.open(at: window.convertPoint(toScreen: event.locationInWindow))
        return nil
    }

    /// The innermost enabled region under `point` (window coordinates), unless a text view is there.
    ///
    /// In a list, the whole row is the target, as in `NSTableView`: a click in the row's side gutter,
    /// outside the row's content, goes to the outermost region in that row.
    func region(at point: CGPoint, in window: NSWindow) -> KzContextMenuRegion? {
        guard !Self.hitsTextView(at: point, in: window) else { return nil }
        let regions = entries.compactMap(\.region).filter { $0.anchor.isEnabled && $0.anchor.window === window }
        let area = { (region: KzContextMenuRegion) in region.anchor.visibleBounds.width * region.anchor.visibleBounds.height }
        let under = regions.filter { $0.anchor.contains(point, in: window) }
        if !under.isEmpty {
            // Nested regions: the smallest one under the pointer is the innermost.
            return under.min { area($0) < area($1) }
        }
        guard let row = Self.tableRow(at: point, in: window) else { return nil }
        return regions.filter { $0.anchor.isDescendant(of: row) && !$0.anchor.isHiddenOrHasHiddenAncestor }.max { area($0) < area($1) }
    }

    /// The table row (a `List` row) under `point`, if any.
    private static func tableRow(at point: CGPoint, in window: NSWindow) -> NSTableRowView? {
        guard let frameView = window.contentView?.superview else { return nil }
        var view = frameView.hitTest(frameView.convert(point, from: nil))
        while let current = view {
            if let row = current as? NSTableRowView { return row }
            if let table = current as? NSTableView {
                let row = table.row(at: table.convert(point, from: nil))
                return row >= 0 ? table.rowView(atRow: row, makeIfNecessary: false) : nil
            }
            view = current.superview
        }
        return nil
    }

    /// Text views keep Cut, Copy, Paste, spelling and Services: the native menu is the right one there.
    private static func hitsTextView(at point: CGPoint, in window: NSWindow) -> Bool {
        guard let content = window.contentView, let frameView = content.superview else { return false }
        var view = frameView.hitTest(frameView.convert(point, from: nil))
        while let current = view {
            if current is NSTextView || (current as? NSTextField)?.isEditable == true || (current as? NSTextField)?.isSelectable == true {
                return true
            }
            if current === content { break }
            view = current.superview
        }
        return false
    }
}

private struct KzContextMenuModifier: ViewModifier {
    let title: String?
    let isPresented: Binding<Bool>?
    let content: () -> [KzMenuElement]
    @StateObject private var region = KzContextMenuRegion()
    @Environment(\.isEnabled) private var isEnabled

    func body(content view: Content) -> some View {
        view.background(KzAnchorBackground(view: region.anchor, isEnabled: isEnabled) {
            region.makeMenu = { KzMenu(title: title, elements: content()) }
            region.isPresented = isPresented
        })
    }
}

extension View {
    /// Adds a Kudzu context menu: right-click or Control-click opens it at the pointer.
    ///
    /// Normal clicks, selection and drags in the view are unaffected. The innermost region wins
    /// when they're nested, and text fields and editors keep their native text menu. Pass
    /// `isPresented` to draw the clicked row's outline while the menu is open.
    public func kzContextMenu(title: String? = nil, isPresented: Binding<Bool>? = nil,
                              @KzMenuBuilder _ content: @escaping () -> [KzMenuElement]) -> some View {
        modifier(KzContextMenuModifier(title: title, isPresented: isPresented, content: content))
    }
}

// MARK: - AppKit

extension KzMenu {
    /// Opens the menu at `point` in `view`'s coordinates (a context menu from an AppKit view).
    @MainActor
    public func popUp(at point: NSPoint, in view: NSView, onClose: @escaping @MainActor () -> Void = {}) {
        guard let window = view.window else { return }
        let screenPoint = window.convertPoint(toScreen: view.convert(point, to: nil))
        KzMenuSession.present(self, at: .point(screenPoint), parentWindow: window, trigger: KzMenuTrigger.current, onClose: onClose)
    }

    /// Opens the menu attached to `rect` in `view`'s coordinates.
    @MainActor
    public func popUp(from rect: NSRect, in view: NSView, placement: KzMenuPlacement = .below, onClose: @escaping @MainActor () -> Void = {}) {
        guard let window = view.window else { return }
        let screenRect = window.convertToScreen(view.convert(rect, to: nil))
        KzMenuSession.present(self, at: .anchor(screenRect, placement), parentWindow: window, trigger: KzMenuTrigger.current, onClose: onClose)
    }

    /// True while a Kudzu menu is open anywhere in the app.
    @MainActor public static var isOpen: Bool { KzMenuSession.current != nil }

    /// Closes the open Kudzu menu, if any, without running an item.
    @MainActor public static func dismissOpenMenu() {
        KzMenuSession.current?.close()
    }
}
