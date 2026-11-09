import SwiftUI

/// The permission companion window (`permission-center.tsx`). The shell hosts it with the shared
/// ``AppModel`` in the environment (`@Environment(AppModel.self)`).
struct PermissionsRootView: View {
    var body: some View {
        PermissionCenterView()
    }
}
