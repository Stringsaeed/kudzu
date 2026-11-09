import KudzuCore
import KudzuDesign
import SwiftUI

/// Apps settings section (`settings.tsx` `AppsSettings`).
struct AppsSettingsView: View {
    @Environment(AppModel.self) private var model
    @State private var apps = InstalledApps.shared
    @State private var adding: [String] = []

    private var entries: [AppInsertionsLogic.Entry] {
        AppInsertionsLogic.sortedEntries(model.settings.appInsertions) { bundleId in
            InstalledApps.appName(apps.apps, bundleId)
        }
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 32) {
            SettingsCardGroup(
                "Never expand in",
                description: "Password managers are excluded by default. Snippet and group rules can't override this list."
            ) {
                AppPicker(
                    value: Binding(
                        get: { model.settings.deniedBundleIDs },
                        set: { next in Task { await model.patchSettings { $0.deniedBundleIDs = next } } }
                    ),
                    addLabel: "Exclude an app"
                )
                .padding(.vertical, 12)
            }
            .stagger(0)

            // One plain row per app (`flex items-center gap-3 py-2.5`), then the add picker, all
            // divided by hairlines like the card's other rows.
            SettingsCardGroup(
                "How to insert, per app",
                description: "Terminals work best with paste. Snippets that choose their own method win."
            ) {
                ForEach(entries) { entry in
                    overrideRow(entry)
                    Hairline()
                }

                AppPicker(value: $adding, addLabel: "Add an app")
                    .padding(.vertical, 12)
                    .onChange(of: adding) { _, ids in
                        let fresh = ids.filter { model.settings.appInsertions[$0] == nil }
                        guard !fresh.isEmpty else { return }
                        let updated = AppInsertionsLogic.adding(fresh, to: model.settings.appInsertions)
                        adding = []
                        Task { await model.patchSettings { $0.appInsertions = updated } }
                    }
            }
            .stagger(1)
        }
        .task {
            apps.load(from: model.backend)
        }
    }

    private func overrideRow(_ entry: AppInsertionsLogic.Entry) -> some View {
        let app = apps.apps.first { $0.bundleId == entry.bundleId }
        let name = InstalledApps.appName(apps.apps, entry.bundleId)

        return HStack(spacing: 12) {
            AppIconView(app: app, name: name, size: 22)

            Text(name)
                .font(.system(size: 13))
                .foregroundStyle(.kz.foreground)
                .lineLimit(1)
                .frame(maxWidth: .infinity, alignment: .leading)

            Segmented(
                "Insertion in \(name)",
                selection: Binding(
                    get: { entry.method },
                    set: { next in
                        let updated = AppInsertionsLogic.setting(
                            entry.bundleId,
                            method: next,
                            in: model.settings.appInsertions
                        )
                        Task { await model.patchSettings { $0.appInsertions = updated } }
                    }
                ),
                size: .sm,
                options: [
                    SegmentOption("Auto", value: InsertionMethod.automatic),
                    SegmentOption("Type", value: InsertionMethod.type),
                    SegmentOption("Paste", value: InsertionMethod.paste),
                ]
            )

            Button {
                let updated = AppInsertionsLogic.removing(
                    entry.bundleId,
                    from: model.settings.appInsertions
                )
                Task { await model.patchSettings { $0.appInsertions = updated } }
            } label: {
                Icon.x.icon()
            }
            .buttonStyle(.kz(.ghost, size: .iconXs))
            .accessibilityLabel("Remove \(name)")
        }
        .padding(.vertical, 10)
    }

}
