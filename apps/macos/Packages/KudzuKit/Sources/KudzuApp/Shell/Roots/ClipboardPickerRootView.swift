import SwiftUI

/// The ⌃⌥V clipboard picker panel. The shell hosts it with the shared ``AppModel`` in the
/// environment (`@Environment(AppModel.self)`).
struct ClipboardPickerRootView: View {
    var body: some View {
        ClipboardPickerView()
    }
}
