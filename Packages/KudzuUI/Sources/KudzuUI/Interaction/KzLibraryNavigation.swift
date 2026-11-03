import AppKit
import Foundation
import SwiftUI

/// A pane of the main window the keyboard can land on.
public enum KzLibraryPane: Equatable, Sendable {
    case sidebar
    case search
    case results
    case editorName
    case editorTriggers
    case editorContent
}

/// What currently owns the keyboard, derived from the first responder.
public enum KzFocusedControl: Equatable, Sendable {
    case sidebar
    case search
    case results
    case editorName
    case editorTrigger
    case editorContent
    case rename
    case otherText
    case other
}

/// Keys the main window routes. Plain typing never arrives here.
public enum KzLibraryKey: Equatable, Sendable {
    case escape
    case tab
    case shiftTab
    case down
    case up
    case `return`
    case delete
}

/// What the window should do with one routed key.
public enum KzLibraryAction: Equatable, Sendable {
    /// Let the focused control handle the key. Text editing, IME composition, and open menus use this.
    case passthrough
    /// Move keyboard focus to a pane.
    case focus(KzLibraryPane)
    /// Empty the search field and move to the results.
    case clearSearch
    /// Drop uncommitted trigger text only. Saved snippet edits stay.
    case cancelTriggerDraft
    /// Delete the selected snippets. Only when the results list itself is focused.
    case deleteSelection
}

/// Facts the router needs. The window fills this in; the decision stays free of AppKit.
public struct KzLibraryKeyContext: Equatable, Sendable {
    public var control: KzFocusedControl
    public var inSettings: Bool
    public var queryIsEmpty: Bool
    /// A single visible snippet can take Return into the editor.
    public var hasSingleSelection: Bool
    /// The trigger field is holding text that is not a saved trigger yet.
    public var triggerDraftPending: Bool
    /// An input method is composing, or a menu, popover, sheet, or dialog is open.
    public var transientOpen: Bool

    public init(
        control: KzFocusedControl,
        inSettings: Bool = false,
        queryIsEmpty: Bool = true,
        hasSingleSelection: Bool = false,
        triggerDraftPending: Bool = false,
        transientOpen: Bool = false
    ) {
        self.control = control
        self.inSettings = inSettings
        self.queryIsEmpty = queryIsEmpty
        self.hasSingleSelection = hasSingleSelection
        self.triggerDraftPending = triggerDraftPending
        self.transientOpen = transientOpen
    }
}

/// Routes main-window keys. Escape never discards snippet edits: it closes the innermost transient
/// surface (by passing the key through), cancels an uncommitted trigger, clears the search, or
/// returns focus to the results. While settings is open the modal owns Escape and Tab, so every key
/// passes through.
///
/// Find (⌘F), New (⌘N), Duplicate (⌘D), and Save (⌘S) are command shortcuts, not routed keys. They
/// are refused while settings is open so they cannot reach the library under the modal.
public enum KzLibraryKeyRouter {
    public static func route(_ key: KzLibraryKey, in context: KzLibraryKeyContext) -> KzLibraryAction {
        // A sheet, menu, or the settings modal owns the key. Settings in particular keeps Escape
        // and Tab; this router must not leave the modal or move the library underneath it.
        if context.transientOpen || context.inSettings { return .passthrough }
        switch key {
        case .escape:
            return escape(context)
        case .tab:
            return tab(context)
        case .shiftTab:
            return shiftTab(context)
        case .down:
            return context.control == .search ? .focus(.results) : .passthrough
        case .up, .return:
            if key == .return, context.control == .results, context.hasSingleSelection {
                return .focus(.editorContent)
            }
            return .passthrough
        case .delete:
            return context.control == .results ? .deleteSelection : .passthrough
        }
    }

    private static func escape(_ context: KzLibraryKeyContext) -> KzLibraryAction {
        if context.control == .rename { return .passthrough }
        if context.control == .editorTrigger, context.triggerDraftPending { return .cancelTriggerDraft }
        if !context.queryIsEmpty { return .clearSearch }
        if context.control != .results { return .focus(.results) }
        return .passthrough
    }

    private static func tab(_ context: KzLibraryKeyContext) -> KzLibraryAction {
        switch context.control {
        case .sidebar: return .focus(.search)
        case .search: return .focus(.results)
        case .results: return .focus(.editorName)
        case .editorName: return .focus(.editorTriggers)
        case .editorTrigger, .editorContent, .rename, .otherText, .other: return .passthrough
        }
    }

    private static func shiftTab(_ context: KzLibraryKeyContext) -> KzLibraryAction {
        switch context.control {
        case .search: return .focus(.sidebar)
        case .results: return .focus(.search)
        case .editorName: return .focus(.results)
        case .editorContent: return .focus(.editorTriggers)
        case .sidebar: return .focus(.editorContent)
        case .editorTrigger, .rename, .otherText, .other: return .passthrough
        }
    }
}

/// Turns a key code into a routed key. Command, option, and control pass through, as does shift
/// except on Tab, so text views keep their editing shortcuts.
public enum KzLibraryKeyParser {
    public static func key(code: UInt16, shift: Bool, command: Bool, option: Bool, control: Bool) -> KzLibraryKey? {
        if command || option || control { return nil }
        if shift, code != Key.tab { return nil }
        switch code {
        case Key.escape: return .escape
        case Key.tab: return shift ? .shiftTab : .tab
        case Key.down: return .down
        case Key.up: return .up
        case Key.return, Key.keypadEnter: return .return
        case Key.delete, Key.forwardDelete: return .delete
        default: return nil
        }
    }

    /// Hardware key codes. Named so the tests can pin them without Carbon.
    public enum Key {
        public static let escape: UInt16 = 53
        public static let tab: UInt16 = 48
        public static let down: UInt16 = 125
        public static let up: UInt16 = 126
        public static let `return`: UInt16 = 36
        public static let keypadEnter: UInt16 = 76
        public static let delete: UInt16 = 51
        public static let forwardDelete: UInt16 = 117
    }
}

/// Maps an accessibility identifier, or a table / chip text view, to the control the router sees.
public enum KzFocusIdentity {
    public static func control(identifier: String?, isTable: Bool, isChipText: Bool) -> KzFocusedControl {
        if isChipText { return .editorContent }
        if isTable { return .results }
        switch identifier {
        case "kudzu.search": return .search
        case "kudzu.trigger": return .editorTrigger
        case "kudzu.editor.name": return .editorName
        case "kudzu.rename": return .rename
        case "kudzu.sidebar": return .sidebar
        case .some: return .otherText
        case .none: return .other
        }
    }
}

/// Keeps the snippet selection inside the rows the list is actually showing.
public enum KzSelectionReconciler {
    /// Drops selected ids the filtered list no longer shows. Does not invent a selection.
    public static func reconciled(_ selection: Set<UUID>, visible: some Sequence<UUID>) -> Set<UUID> {
        selection.intersection(Set(visible))
    }

    /// The selection to use when focus enters the results: the visible part of the current
    /// selection, or the first row when none of it is visible.
    public static func selectionForEnteringResults(current: Set<UUID>, visible: [UUID]) -> Set<UUID> {
        let kept = reconciled(current, visible: visible)
        if !kept.isEmpty { return kept }
        if let first = visible.first { return [first] }
        return []
    }
}

/// Decides whether a selection change may leave the snippet on screen.
public enum KzNavigationGate {
    public enum Decision: Equatable, Sendable {
        /// Apply this selection now.
        case apply(Set<UUID>)
        /// Keep the current selection and remember the one the user asked for.
        case hold(Set<UUID>)
    }

    /// Same selection applies immediately. A different one applies only when `allowsLeave` is true
    /// (the editor has no unsaved draft, or the user already saved or discarded it).
    public static func decide(current: Set<UUID>, proposed: Set<UUID>, allowsLeave: Bool) -> Decision {
        if proposed == current || allowsLeave { return .apply(proposed) }
        return .hold(proposed)
    }
}

/// Keeps the main window a normal, resizable window on its own Space, with system full screen.
public enum KzMainWindowPolicy {
    /// Behaviors that make a window float over other apps or follow every Space.
    public static let disallowed: NSWindow.CollectionBehavior = [
        .canJoinAllSpaces, .moveToActiveSpace, .fullScreenAuxiliary, .stationary, .transient,
        .ignoresCycle, .fullScreenNone, .fullScreenDisallowsTiling,
    ]
    /// A document window: it lives on one Space, joins the window cycle, and can enter full screen.
    public static let required: NSWindow.CollectionBehavior = [
        .managed, .fullScreenPrimary, .participatesInCycle, .fullScreenAllowsTiling,
    ]

    /// Applies the policy without moving or resizing the window.
    @MainActor
    public static func apply(to window: NSWindow) {
        if window.level != .normal { window.level = .normal }
        if !window.styleMask.contains(.resizable) { window.styleMask.insert(.resizable) }
        let behavior = window.collectionBehavior.subtracting(disallowed).union(required)
        if window.collectionBehavior != behavior { window.collectionBehavior = behavior }
        let minimum = KzAppShell<EmptyView, EmptyView, EmptyView, EmptyView, EmptyView>.minimumSize
        if window.contentMinSize.width < minimum.width || window.contentMinSize.height < minimum.height {
            window.contentMinSize = minimum
        }
    }
}
