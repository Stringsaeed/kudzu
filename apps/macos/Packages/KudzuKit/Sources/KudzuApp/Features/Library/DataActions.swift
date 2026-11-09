// Import and export through the system's open and save panels (`data-actions.ts`). The browser
// preview's "works in the desktop app" toasts have no counterpart: the native app always has file
// access.

import AppKit
import KudzuCore
import KudzuDesign
import UniformTypeIdentifiers

/// `runImport` / `runExport` and the panels behind them. Shared with Settings and onboarding.
@MainActor
public enum DataActions {
    /// Where an import comes from (`ImportKind`).
    public enum ImportKind: Sendable {
        /// Any snippet file; the backend works out Kudzu, Raycast, Espanso or TextExpander.
        case file
        case raycast
        case textexpander
        case espansoFolder
    }

    /// Short export instructions per source (`IMPORT_HINTS`).
    public static let raycastHint = "In Raycast, run Export Snippets, then choose the .json file."
    public static let textExpanderHint = "In TextExpander, export a group as .textexpander or CSV."

    /// The open panel's title and allowed extensions per kind (`IMPORT_DIALOGS`).
    static func dialog(_ kind: ImportKind) -> (title: String, extensions: [String]) {
        switch kind {
        case .file, .espansoFolder: ("Import snippets", ["json", "yml", "yaml", "textexpander", "csv"])
        case .raycast: ("Choose your Raycast snippets export", ["json"])
        case .textexpander: ("Choose your TextExpander export", ["textexpander", "csv"])
        }
    }

    /// Shows the folder panel for an Espanso config folder; `nil` if cancelled.
    public static func pickEspansoFolder() -> String? {
        folder(title: "Choose your Espanso config folder")
    }

    /// Shows the file panel for `kind` and returns the chosen path, or `nil` if cancelled.
    public static func pickImportFile(_ kind: ImportKind) -> String? {
        let d = dialog(kind)
        let panel = NSOpenPanel()
        panel.title = d.title
        panel.message = d.title
        panel.canChooseFiles = true
        panel.canChooseDirectories = false
        panel.allowsMultipleSelection = false
        // `.textexpander` has no system type; fall back to its extension-derived dynamic type.
        panel.allowedContentTypes = d.extensions.map { UTType(filenameExtension: $0) ?? .data }
        return panel.runModal() == .OK ? panel.url?.path : nil
    }

    /// Asks for a snippet file (or Espanso folder) and imports it, reporting with a toast.
    public static func runImport(_ kind: ImportKind = .file, model: AppModel) async {
        do {
            if kind == .espansoFolder {
                guard let dir = pickEspansoFolder() else { return }
                report(try await model.backend.importEspanso(path: dir))
                return
            }
            guard let file = pickImportFile(kind) else { return }
            report(try await model.backend.importFile(path: file))
        } catch {
            toast.error("Import failed", description: String(describing: error))
        }
    }

    /// Exports the library as Kudzu JSON (save panel) or Espanso match files (folder panel).
    public static func runExport(_ kind: ExportKind, model: AppModel) async {
        do {
            switch kind {
            case .json:
                let panel = NSSavePanel()
                panel.title = "Export library"
                panel.nameFieldStringValue = "kudzu-\(stamp()).json"
                panel.allowedContentTypes = [.json]
                guard panel.runModal() == .OK, let path = panel.url?.path else { return }
                try await model.backend.exportJson(path: path)
                toast.success("Library exported", description: path)
            case .espanso:
                guard let dir = folder(title: "Choose a folder for the Espanso match files") else { return }
                let n = try await model.backend.exportEspanso(dir: dir)
                let copy = LibraryCopy.exportedEspanso(n, dir: dir)
                toast.success(copy.title, description: copy.description)
            }
        } catch {
            toast.error("Export failed", description: String(describing: error))
        }
    }

    public enum ExportKind: Sendable { case json, espanso }

    // MARK: Helpers

    private static func report(_ r: ImportSummary) {
        let copy = LibraryCopy.imported(r)
        toast.success(copy.title, description: copy.description)
    }

    private static func folder(title: String) -> String? {
        let panel = NSOpenPanel()
        panel.title = title
        panel.message = title
        panel.canChooseFiles = false
        panel.canChooseDirectories = true
        panel.canCreateDirectories = true
        panel.allowsMultipleSelection = false
        return panel.runModal() == .OK ? panel.url?.path : nil
    }

    /// `new Date().toISOString().slice(0, 10)` (UTC date).
    static func stamp(_ date: Date = Date()) -> String {
        var calendar = Calendar(identifier: .gregorian)
        calendar.timeZone = TimeZone(identifier: "UTC") ?? .gmt
        let c = calendar.dateComponents([.year, .month, .day], from: date)
        return String(format: "%04d-%02d-%02d", c.year ?? 0, c.month ?? 0, c.day ?? 0)
    }
}
