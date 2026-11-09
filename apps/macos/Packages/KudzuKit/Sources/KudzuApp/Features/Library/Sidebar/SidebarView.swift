// The library sidebar (`sidebar.tsx`): a native sidebar `List` (selection, keyboard, collapsible
// sections with Finder's hover chevron, disclosure chevrons for subgroups) with the snippets tree, clipboard filters, Insights and Settings, and the status row.
// Section collapse is remembered; group collapse lasts for the session, as it did on the web.

import AppKit
import KudzuCore
import KudzuDesign
import SwiftUI

struct SidebarView: View {
    @Environment(AppModel.self) private var model
    @Environment(LibraryActions.self) private var actions
    @Environment(LibraryFocus.self) private var focus
    @State private var closedSections = SidebarSections.decode(UserDefaults.standard.string(forKey: SidebarSections.storageKey))
    @State private var collapsedGroups: Set<UUID> = []

    var body: some View {
        let library = model.library
        let tree = Groups.buildTree(library.groups)
        let counts = Groups.recursiveCounts(library.groups, snippets: library.snippets)
        let disabled = library.snippets.filter { !$0.isEnabled }.count
        let clipCounts = ClipSidebar.counts(model.clipboard.entries)
        VStack(spacing: 0) {
            brand
            List(selection: selection) {
                // Sections, not disclosure groups: their items line up with the heading, as the web's do.
                Section(isExpanded: sectionOpen("snippets")) {
                    nav(.all, icon: Icon.layers, label: "All snippets", count: library.snippets.count, shortcut: "mod+1")
                    nav(.favorites, icon: Icon.star, label: "Favorites", count: library.snippets.filter(\.isFavorite).count, shortcut: "mod+2")
                    nav(.recent, icon: Icon.clock3, label: "Recently used", count: model.usage.values.filter { $0.last > 0 }.count, shortcut: "mod+3")
                    if disabled > 0 {
                        nav(.disabled, icon: Icon.circleOff, label: "Turned off", count: disabled)
                    }
                    groupsHeader
                        .dropDestination(for: LibraryDrag.self) { items, _ in actions.drop(items, on: nil) }
                    if tree.isEmpty {
                        emptyGroups
                    } else {
                        GroupTreeRows(nodes: tree, counts: counts, collapsed: $collapsedGroups)
                    }
                } header: {
                    sectionTitle("Snippets")
                }
                Section(isExpanded: sectionOpen("clipboard")) {
                    nav(.clipboard(filter: nil), icon: Icon.clipboardList, label: "History", count: clipCounts["all"], shortcut: "mod+5")
                    ForEach(ClipSidebar.visibleFilters(counts: clipCounts, active: activeClip), id: \.value) { filter in
                        nav(.clipboard(filter: ClipFilter(rawValue: filter.value)), icon: clipIcon(filter.value), label: filter.label, count: clipCounts[filter.value])
                    }
                } header: {
                    HStack(spacing: 6) {
                        sectionTitle("Clipboard")
                        Spacer(minLength: 0)
                        if !model.settings.clipboardEnabled { PausedBadge() }
                    }
                }
            }
            .listStyle(.sidebar)
            .scrollContentBackground(.hidden)
            .onKeyPress(phases: .down) { press in
                guard actions.renaming == nil, case let .group(id) = model.view,
                      !press.modifiers.contains(.command), !press.modifiers.contains(.control),
                      GroupRenameGesture.startsRename(
                        key: press.key == .return ? "\r" : press.characters,
                        option: press.modifiers.contains(.option)
                      ) else { return .ignored }
                actions.renaming = id
                return .handled
            }
            footer
            StatusIndicatorView()
        }
        .accessibilityElement(children: .contain)
        .accessibilityLabel("Sidebar")
    }

    private var brand: some View {
        HStack(spacing: 10) {
            Button {
                model.setView(.all)
                focus.focusList()
            } label: {
                HStack(spacing: 10) {
                    brandMark
                        .frame(width: 36, height: 36)
                        .accessibilityHidden(true)
                    Wordmark(.small).frame(width: 76)
                }
            }
            .buttonStyle(.plain)
            .accessibilityLabel("Kudzu — all snippets")
            Spacer(minLength: 0)
            if model.isDev { Badge("Dev") }
        }
        .padding(.horizontal, 16)
        .frame(height: 72)
    }

    /// The app icon when the process is the Kudzu bundle; the blue "K" in previews and tests,
    /// where `NSApp.applicationIconImage` is a generic executable icon.
    @ViewBuilder private var brandMark: some View {
        if Bundle.main.object(forInfoDictionaryKey: "CFBundleIconName") != nil
            || Bundle.main.object(forInfoDictionaryKey: "CFBundleIconFile") != nil,
           let icon = NSApp?.applicationIconImage {
            Image(nsImage: icon).resizable().interpolation(.high)
        } else {
            RoundedRectangle(cornerRadius: 8, style: .continuous)
                .fill(Wordmark.brandBlue)
                .overlay { Text("K").font(.system(size: 20, weight: .bold)).foregroundStyle(.white) }
        }
    }

    private var groupsHeader: some View {
        HStack(spacing: 4) {
            Text("Groups")
                .font(.system(size: 11, weight: .medium))
                .foregroundStyle(Color.kz.faint)
                .frame(maxWidth: .infinity, alignment: .leading)
            Button { actions.newGroup() } label: { Icon.plus.icon(14) }
                .buttonStyle(.kz(.ghost, size: .iconXs))
                .accessibilityLabel("New group")
                .kzHelp("New group", shortcut: "mod+shift+n")
        }
        .listRowSeparator(.hidden)
    }

    private var emptyGroups: some View {
        Button { actions.newGroup() } label: {
            Text("Organize snippets into groups — like Work, Email or Code.")
                .font(.system(size: 12))
                .foregroundStyle(Color.kz.mutedForeground)
                .multilineTextAlignment(.leading)
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding(.horizontal, 8)
                .padding(.vertical, 8)
                .overlay {
                    RoundedRectangle(cornerRadius: Radius.md)
                        .strokeBorder(Color.kz.border, style: StrokeStyle(lineWidth: 1, dash: [4, 3]))
                }
        }
        .buttonStyle(.plain)
        .listRowSeparator(.hidden)
    }

    private var footer: some View {
        VStack(spacing: 0) {
            Hairline() // `border-t border-hairline`
            List(selection: selection) {
                nav(.insights, icon: Icon.barChart3, label: "Insights", shortcut: "mod+4")
                nav(.settings(section: nil), icon: Icon.settings, label: "Settings", shortcut: "mod+,")
            }
            .listStyle(.sidebar)
            .scrollContentBackground(.hidden)
            .scrollDisabled(true)
            .frame(height: 72)
            .padding(.bottom, 12)
            // Match the scrolling list's legacy gutter so both shortcut columns line up.
            .padding(.trailing, NSScroller.preferredScrollerStyle == .legacy
                     ? NSScroller.scrollerWidth(for: .regular, scrollerStyle: .legacy) : 0)
        }
    }

    private func nav(_ route: Route, icon: Image, label: String, count: Int? = nil, shortcut: String? = nil) -> some View {
        SidebarRow(icon: icon, label: label, count: count, shortcut: shortcut, selected: LibraryRoutes.sidebarTag(model.view) == route)
            .tag(route)
            .listRowSeparator(.hidden)
            .listRowInsets(EdgeInsets(top: 2, leading: 0, bottom: 2, trailing: 0))
    }

    private func sectionTitle(_ title: String) -> some View {
        Text(title)
            .font(.system(size: 11, weight: .semibold))
            .tracking(0.4)
            .foregroundStyle(Color.kz.faint)
            .textCase(nil)
    }

    private func sectionOpen(_ id: String) -> Binding<Bool> {
        Binding(get: { !closedSections.contains(id) }, set: { open in
            if open { closedSections.remove(id) } else { closedSections.insert(id) }
            UserDefaults.standard.set(SidebarSections.encode(closedSections), forKey: SidebarSections.storageKey)
        })
    }

    /// The clipboard filter the sidebar should highlight, or nil off the clipboard route.
    private var activeClip: String? {
        guard case let .clipboard(filter) = model.view else { return nil }
        return (filter ?? .all).rawValue
    }

    private var selection: Binding<Route?> {
        Binding(
            get: { LibraryRoutes.sidebarTag(model.view) },
            set: { route in
                guard let route else { return }
                model.setView(route)
                focus.focusList()
            }
        )
    }

    private func clipIcon(_ value: String) -> Image {
        switch value {
        case "pinned": return Icon.pin
        case "image": return Icon.image
        case "link": return Icon.globe
        case "file": return Icon.file
        case "code": return Icon.braces
        case "color": return Icon.palette
        default: return Icon.alignLeft
        }
    }
}
