// `AppPicker` (`app-picker.tsx`): the chosen apps as removable chips plus an "Add app" button that
// opens a searchable list of installed apps. Choosing toggles an app and keeps the list open; a
// query that looks like a bundle id (`com.example.app`) can be added by hand when nothing matches.
// Used by the editor's app rules, group settings (T7) and Settings › Apps (T11).

import KudzuCore
import KudzuDesign
import SwiftUI

public struct AppPicker: View {
    @Binding var value: [String]
    var addLabel: String
    @Environment(AppModel.self) private var model
    @State private var apps = InstalledApps.shared
    @State private var open = false
    @State private var query = ""

    /// - Parameters:
    ///   - value: chosen bundle ids, in order.
    ///   - addLabel: the add button's title ("Allow an app", "Block an app", …).
    public init(value: Binding<[String]>, addLabel: String = "Add app") {
        _value = value
        self.addLabel = addLabel
    }

    public var body: some View {
        FlowLayout(spacing: 6, lineSpacing: 6) {
            ForEach(value, id: \.self) { id in
                AppChip(bundleId: id, app: apps.apps.first { $0.bundleId == id }, name: InstalledApps.appName(apps.apps, id)) {
                    withAnimation(.kz.springPop()) { value = AppPicker.toggled(value, id) }
                }
                .transition(KZTransition.pop)
            }
            Button { open = true } label: { ButtonLabel(addLabel, icon: Icon.plus) }
                .buttonStyle(.kz(.ghost, size: .sm))
                .foregroundStyle(Color.kz.mutedForeground)
                .overlay {
                    RoundedRectangle(cornerRadius: ButtonSize.sm.radius)
                        .strokeBorder(Color.kz.border, style: StrokeStyle(lineWidth: 1, dash: [3, 2]))
                }
                .popover(isPresented: $open, arrowEdge: .bottom) { list }
        }
        .task { apps.load(from: model.backend) }
        .onChange(of: open) { if !open { query = "" } }
    }

    private var list: some View {
        let chosen = Set(value)
        let items = apps.apps.map { app in
            PickItem(id: app.bundleId, value: "\(app.name) \(app.bundleId)", group: nil) {
                value = AppPicker.toggled(value, app.bundleId)
            }
        }
        return PickList(query: $query, placeholder: "Search apps or type a bundle ID…", items: items) { item, _ in
            let app = apps.apps.first { $0.bundleId == item.id }
            HStack(spacing: 8) {
                AppIcon(path: app?.path, name: app?.name ?? item.id, size: 18)
                Text(app?.name ?? item.id).lineLimit(1).truncationMode(.tail)
                Spacer(minLength: 0)
                if chosen.contains(item.id) { Icon.check.icon(14).foregroundStyle(Color.kz.iris) }
            }
        } empty: {
            if AppPicker.isBundleID(query) {
                Button("Add “\(query)”") {
                    value = AppPicker.toggled(value, query)
                    query = ""
                }
                .buttonStyle(.kz(.link, size: .sm))
            } else {
                Text("No apps found.")
            }
        }
        .frame(width: 288)
    }

    // MARK: Logic

    /// Adds `id`, or removes it when already chosen.
    static func toggled(_ value: [String], _ id: String) -> [String] {
        value.contains(id) ? value.filter { $0 != id } : value + [id]
    }

    /// `/^[\w-]+(\.[\w-]+)+$/`: a reverse-DNS bundle identifier typed by hand.
    static func isBundleID(_ text: String) -> Bool {
        let parts = text.split(separator: ".", omittingEmptySubsequences: false)
        guard parts.count >= 2 else { return false }
        return parts.allSatisfy { part in
            !part.isEmpty && part.unicodeScalars.allSatisfy { $0 == "_" || $0 == "-" || ($0.isASCII && CharacterSet.alphanumerics.contains($0)) }
        }
    }
}

/// One chosen app: `h-7 rounded-md bg-surface-raised pl-1.5 pr-1 text-xs shadow-raised`.
struct AppChip: View {
    var bundleId: String
    var app: AppInfo?
    var name: String
    var onRemove: () -> Void

    var body: some View {
        HStack(spacing: 6) {
            AppIconView(app: app, name: name, size: 16)
            Text(name).font(.system(size: 12)).lineLimit(1).truncationMode(.tail).frame(maxWidth: 160, alignment: .leading)
            Button(action: onRemove) { Icon.x.icon(12) }
                .buttonStyle(.kz(.ghost, size: .iconXs))
                .frame(width: 20, height: 20)
                .foregroundStyle(Color.kz.mutedForeground)
                .accessibilityLabel("Remove \(name)")
        }
        .padding(.leading, 6)
        .padding(.trailing, 4)
        .frame(height: 28)
        .depth(.raised, fill: Color.kz.surfaceRaised, in: RoundedRectangle(cornerRadius: Radius.md))
        .help(bundleId)
    }
}

/// `AppIcon` for an `AppInfo` (`app-icon.tsx`): the bundle's icon, or a monogram tile.
public struct AppIconView: View {
    var app: AppInfo?
    var name: String
    var size: CGFloat

    public init(app: AppInfo?, name: String? = nil, size: CGFloat = 18) {
        self.app = app
        self.name = name ?? app?.name ?? "?"
        self.size = size
    }

    public var body: some View { AppIcon(path: app?.path, name: name, size: size) }
}
