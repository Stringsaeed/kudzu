import SwiftUI

/// The caret form panel. The shell hosts it with the shared ``AppModel`` in the environment
/// (`@Environment(AppModel.self)`).
struct FormRootView: View {
    var body: some View {
        FormView()
    }
}
