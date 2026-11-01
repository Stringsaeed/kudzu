import AppKit
import KudzuUI
import SwiftUI

/// Entry point. `KudzuGallery --snapshot [dir] [--page id]` renders pages to PNG and exits
/// without opening a window; otherwise the gallery app runs normally.
@main
enum GalleryMain {
    @MainActor
    static func main() {
        KzFonts.register()
        let arguments = CommandLine.arguments
        if let index = arguments.firstIndex(of: "--snapshot") {
            exit(GallerySnapshots.run(arguments: Array(arguments[(index + 1)...])))
        }
        GalleryApp.main()
    }
}

struct GalleryApp: App {
    var body: some Scene {
        WindowGroup("Kudzu Gallery") {
            GalleryShell()
        }
        .defaultSize(width: 1180, height: 780)
    }
}

/// Light, Dark or follow the system, applied app-wide so dynamic NSColors resolve too.
enum GalleryAppearance: String, CaseIterable, Identifiable {
    case system = "System"
    case light = "Light"
    case dark = "Dark"

    var id: String { rawValue }

    var nsAppearance: NSAppearance? {
        switch self {
        case .system: nil
        case .light: NSAppearance(named: .aqua)
        case .dark: NSAppearance(named: .darkAqua)
        }
    }
}

struct GalleryShell: View {
    @State private var selection: String? = Catalog.entries.first?.id
    @AppStorage("gallery.appearance") private var appearance: GalleryAppearance = .system

    var body: some View {
        NavigationSplitView {
            List(selection: $selection) {
                ForEach(GallerySection.allCases, id: \.self) { section in
                    Section(section.rawValue) {
                        ForEach(Catalog.entries.filter { $0.section == section }) { page in
                            Label {
                                Text(page.title).kzTextStyle(.bodyStrong, matchLineBox: false)
                            } icon: {
                                KzIcon(page.icon, size: 16)
                            }
                            .tag(page.id)
                        }
                    }
                }
            }
            .navigationSplitViewColumnWidth(min: 200, ideal: KzSize.sidebar)
            .safeAreaInset(edge: .bottom) {
                Picker("Appearance", selection: $appearance) {
                    ForEach(GalleryAppearance.allCases) { Text($0.rawValue).tag($0) }
                }
                .pickerStyle(.segmented)
                .labelsHidden()
                .padding(KzSpace.s3)
            }
        } detail: {
            ScrollView {
                if let id = selection, let page = Catalog.entry(id: id) {
                    page.makeView()
                        .id(id)
                }
            }
            .background(KzColor.surface)
        }
        .onAppear { NSApp.appearance = appearance.nsAppearance }
        .onChange(of: appearance) { _, value in NSApp.appearance = value.nsAppearance }
    }
}

/// Renders catalogue pages offscreen, in Light and Dark at 2x.
@MainActor
enum GallerySnapshots {
    /// Usage: `--snapshot [directory] [--page id]...`; the directory defaults to `Packages/KudzuUI/Snapshots/gallery`.
    static func run(arguments: [String]) -> Int32 {
        var directory = "Packages/KudzuUI/Snapshots/gallery"
        var only: Set<String> = []
        var iterator = arguments.makeIterator()
        while let argument = iterator.next() {
            if argument == "--page", let id = iterator.next() {
                only.insert(id)
            } else if !argument.hasPrefix("-") {
                directory = argument
            }
        }
        let url = URL(fileURLWithPath: directory, isDirectory: true)
        let pages = Catalog.entries.filter { only.isEmpty || only.contains($0.id) }
        if pages.isEmpty {
            FileHandle.standardError.write(Data("No pages match \(only.sorted()). Ids: \(Catalog.entries.map(\.id).joined(separator: ", "))\n".utf8))
            return 1
        }
        for page in pages {
            do {
                let written = try KzSnapshot.write(page.makeView(), named: page.id, to: url, width: page.snapshotWidth)
                for file in written { print(file.path) }
            } catch {
                FileHandle.standardError.write(Data("\(page.id): \(error)\n".utf8))
                return 1
            }
        }
        return 0
    }
}
