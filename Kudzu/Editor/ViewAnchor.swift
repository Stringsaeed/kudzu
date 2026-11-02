import AppKit
import KudzuUI
import SwiftUI

/// A handle on the AppKit view behind a spot in a SwiftUI hierarchy, for reaching nearby AppKit
/// controls that SwiftUI can't address: the trigger field to focus, the chip text view to insert into.
@MainActor
final class ViewAnchor {
    fileprivate weak var view: NSView?

    /// The nearest view of `type` matching `predicate`: searches the anchor's parent's subtree, then each
    /// ancestor's in turn, so a control next to the anchor wins over one elsewhere in the window.
    func nearest<T: NSView>(_ type: T.Type, where predicate: (T) -> Bool = { _ in true }) -> T? {
        var ancestor = view?.superview
        while let current = ancestor {
            if let found = Self.first(type, in: current, where: predicate) { return found }
            ancestor = current.superview
        }
        return nil
    }

    private static func first<T: NSView>(_ type: T.Type, in view: NSView, where predicate: (T) -> Bool) -> T? {
        if let match = view as? T, predicate(match) { return match }
        for subview in view.subviews {
            if let found = first(type, in: subview, where: predicate) { return found }
        }
        return nil
    }
}

extension View {
    /// Places `anchor` behind this view.
    func viewAnchor(_ anchor: ViewAnchor) -> some View {
        modifier(ViewAnchorModifier(anchor: anchor))
    }
}

private struct ViewAnchorModifier: ViewModifier {
    let anchor: ViewAnchor
    @Environment(\.kzSnapshotRendering) private var snapshotRendering

    func body(content: Content) -> some View {
        // Offscreen renderers draw AppKit views as placeholders.
        if snapshotRendering {
            content
        } else {
            content.background { ViewAnchorRepresentable(anchor: anchor).frame(width: 0, height: 0).accessibilityHidden(true) }
        }
    }
}

private struct ViewAnchorRepresentable: NSViewRepresentable {
    let anchor: ViewAnchor

    func makeNSView(context: Context) -> NSView {
        let view = NSView()
        anchor.view = view
        return view
    }

    func updateNSView(_ view: NSView, context: Context) {
        anchor.view = view
    }
}
