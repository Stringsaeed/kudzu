import SwiftUI

/// The ⌃⌥Space quick-search panel. The shell hosts it with the shared ``AppModel`` in the
/// environment (`@Environment(AppModel.self)`).
struct PaletteRootView: View {
    var body: some View {
        PaletteView()
    }
}
