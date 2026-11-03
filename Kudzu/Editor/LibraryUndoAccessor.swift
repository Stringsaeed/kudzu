import AppKit
import SwiftUI

/// Binds the key window's `UndoManager` to `LibraryModel` for library-level undo/redo.
struct LibraryUndoAccessor: NSViewRepresentable {
    let library: LibraryModel

    func makeCoordinator() -> Coordinator { Coordinator(library: library) }

    func makeNSView(context: Context) -> NSView {
        let view = TrackingView()
        view.onWindowChange = { [weak coordinator = context.coordinator] window in
            coordinator?.attach(to: window)
        }
        return view
    }

    func updateNSView(_ nsView: NSView, context: Context) {
        context.coordinator.library = library
    }

    private final class TrackingView: NSView {
        var onWindowChange: ((NSWindow?) -> Void)?
        override func viewDidMoveToWindow() {
            super.viewDidMoveToWindow()
            onWindowChange?(window)
        }
    }

    @MainActor
    final class Coordinator {
        var library: LibraryModel

        init(library: LibraryModel) {
            self.library = library
        }

        func attach(to window: NSWindow?) {
            library.undoManager = window?.undoManager
        }
    }
}
