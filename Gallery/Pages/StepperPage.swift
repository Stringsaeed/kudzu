import KudzuUI
import SwiftUI

struct StepperPage: GalleryPage {
    static let id = "stepper"
    static let title = "Stepper"
    static let section = GallerySection.components
    static let icon = KzIconName.plus

    // Placeholder: replace with specimens of every variant and state (see Packages/KudzuUI/README.md).
    var body: some View {
        ComponentPlaceholder(name: Self.title, summary: "A number with − and + buttons, for small numeric settings like typing speed.")
    }
}
