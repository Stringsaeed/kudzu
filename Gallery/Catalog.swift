import KudzuUI
import SwiftUI

/// A gallery page. Conform a view to this in `Pages/<Name>Page.swift`, then list it in `Catalog.pages`.
@MainActor
protocol GalleryPage: View {
    /// Stable identifier, used for selection and snapshot file names (kebab-case).
    static var id: String { get }
    /// Sidebar title.
    static var title: String { get }
    /// Sidebar section.
    static var section: GallerySection { get }
    /// Sidebar icon.
    static var icon: KzIconName { get }
    /// Width the page is rendered at by `--snapshot` (pages lay out for this width).
    static var snapshotWidth: CGFloat { get }
    init()
}

extension GalleryPage {
    static var snapshotWidth: CGFloat { 880 }
}

/// Sidebar sections, in display order.
enum GallerySection: String, CaseIterable {
    case foundations = "Foundations"
    case components = "Components"
}

/// Every page in the gallery. Adding a page is one line here plus its file in `Pages/`.
@MainActor
enum Catalog {
    static let pages: [any GalleryPage.Type] = [
        // Foundations
        ColorsPage.self,
        TypePage.self,
        SpacingPage.self,
        RadiiPage.self,
        ShadowsPage.self,
        IconsPage.self,
        GlassPage.self,
        InteractionPage.self,

        // Components: each file in Pages/ starts as a placeholder; replace its body when you build the component.
        AppShellPage.self,
        BadgePage.self,
        ButtonPage.self,
        DisclosurePage.self,
        EditorPage.self,
        EmptyStatePage.self,
        FieldPage.self,
        IconButtonPage.self,
        KbdPage.self,
        ListHeaderPage.self,
        ListRowPage.self,
        MenuPage.self,
        SearchFieldPage.self,
        SegmentedPage.self,
        SelectPage.self,
        SettingsGroupPage.self,
        SettingsRowPage.self,
        SidebarItemPage.self,
        SidebarSectionPage.self,
        SnippetRowPage.self,
        StatTilePage.self,
        StepperPage.self,
        TextInputPage.self,
        TogglePage.self,
        ToolbarGroupPage.self,
        TriggerChipPage.self,
        TriggerInputPage.self,
        UsageBarPage.self,
    ]

    /// The catalogue as plain values, for lists and the snapshot renderer.
    static let entries: [GalleryEntry] = pages.map { GalleryEntry($0) }

    static func entry(id: String) -> GalleryEntry? {
        entries.first { $0.id == id }
    }
}

/// One page's metadata plus a way to build its view.
@MainActor
struct GalleryEntry: Identifiable {
    let id: String
    let title: String
    let section: GallerySection
    let icon: KzIconName
    let snapshotWidth: CGFloat
    let makeView: @MainActor () -> AnyView

    init(_ page: any GalleryPage.Type) {
        id = page.id
        title = page.title
        section = page.section
        icon = page.icon
        snapshotWidth = page.snapshotWidth
        makeView = { AnyView(page.init()) }
    }
}
