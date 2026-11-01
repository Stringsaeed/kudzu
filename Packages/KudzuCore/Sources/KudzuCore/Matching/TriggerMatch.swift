import Foundation

/// A trigger that just fired.
///
/// What the app deletes and re-emits depends on the fire mode:
///
/// - `.immediate`: the document ends with the trigger. Backspace `deleteCount`
///   characters, insert the expansion. `separator` is `nil`.
/// - `.wholeWord`: the trigger was followed by a separator (`separator`). `deleteCount` counts
///   the **trigger only**. Two valid strategies:
///   1. *Swallow the separator keystroke* (the matcher answers synchronously, so the event tap can
///      drop it): backspace `deleteCount`, insert the expansion, then type `separator`.
///   2. *Let it through*: backspace `deleteCountIncludingSeparator`, insert the expansion, then type
///      `separator`.
///
///   Either way the separator ends up after the expansion, which is what "whole word keeps the
///   separator" means.
public struct TriggerMatch: Hashable, Sendable {
    public let snippetID: UUID
    /// The trigger as stored on the snippet.
    public let trigger: String
    /// The trigger exactly as the user typed it (case may differ from `trigger` for `.propagate`).
    public let typedTrigger: String
    /// Characters (grapheme clusters, one backspace each) making up the typed trigger. Never includes the separator.
    public let deleteCount: Int
    /// The separator that completed a `.wholeWord` match, as typed; `nil` for `.immediate`.
    public let separator: String?
    /// Resolved insertion method for the frontmost app.
    public let insertion: InsertionMethod
    /// Case style to apply to the expansion; always `.asIs` for `.exact` snippets.
    public let caseStyle: CaseStyle
    public let fireMode: FireMode

    public init(
        snippetID: UUID,
        trigger: String,
        typedTrigger: String,
        deleteCount: Int,
        separator: String?,
        insertion: InsertionMethod,
        caseStyle: CaseStyle,
        fireMode: FireMode
    ) {
        self.snippetID = snippetID
        self.trigger = trigger
        self.typedTrigger = typedTrigger
        self.deleteCount = deleteCount
        self.separator = separator
        self.insertion = insertion
        self.caseStyle = caseStyle
        self.fireMode = fireMode
    }

    /// Backspaces needed when the separator keystroke already reached the document.
    public var deleteCountIncludingSeparator: Int {
        deleteCount + (separator == nil ? 0 : 1)
    }
}
