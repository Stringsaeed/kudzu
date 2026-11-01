import Foundation
import KudzuUI
import SwiftUI

/// Where snapshot PNGs go: `Packages/KudzuUI/Snapshots/` (git-ignored).
let snapshotDirectory: URL = URL(fileURLWithPath: #filePath)
    .deletingLastPathComponent() // …/Tests/KudzuUITests/Support
    .deletingLastPathComponent() // …/Tests/KudzuUITests
    .deletingLastPathComponent() // …/Tests
    .deletingLastPathComponent() // …/Packages/KudzuUI
    .appendingPathComponent("Snapshots")

/// Renders `view` in Light and Dark at 2x into `Packages/KudzuUI/Snapshots/<name>-{light,dark}@2x.png`.
///
/// Open the PNGs to review visual changes. Pass a `width` for views that should fill a column.
@MainActor
@discardableResult
func snapshot<V: View>(
    _ name: String,
    width: CGFloat? = nil,
    background: KzColorToken = .surface,
    @ViewBuilder _ view: () -> V
) throws -> [URL] {
    try KzSnapshot.write(view().padding(KzSpace.s4), named: name, to: snapshotDirectory, width: width, background: background)
}

/// The repository root, for tests that read design-system sources.
let repoRoot: URL = snapshotDirectory
    .deletingLastPathComponent() // …/Packages/KudzuUI
    .deletingLastPathComponent() // …/Packages
    .deletingLastPathComponent() // repo root

/// Suspends so queued main-queue work (AppKit bridges that configure themselves a beat after creation)
/// can run. A synchronous main-actor test can't drain the main queue it is itself running on.
@MainActor
func settle(_ seconds: Double = 0.8) async {
    try? await Task.sleep(for: .seconds(seconds))
}
