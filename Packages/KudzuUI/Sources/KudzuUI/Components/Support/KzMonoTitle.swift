import SwiftUI

extension View {
    /// Sets text as the snippet's name in the detail pane: Geist Mono 600, 20/26, -0.01em tracking.
    ///
    /// The design system sets triggers and the snippet name in mono ("Triggers are the hero"); this is
    /// `title-lg` in the mono family, as the Window preview's `.demo-title`. Apply it to the `Text`
    /// itself, and colour it `ink`.
    public func kzMonoTitle() -> some View {
        kzText(.adhoc(family: .mono, size: 20, lineHeight: 26, weight: 600, trackingEm: -0.01))
    }
}
