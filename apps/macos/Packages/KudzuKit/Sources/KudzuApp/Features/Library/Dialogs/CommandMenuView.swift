// ⌘K (`command-menu.tsx`). A native field and list display the already-filtered rows.
// Snippet hits still come from `SearchIndex` (usage-boosted, capped at 5 or 8). The action
// catalogue is `CommandMenuLogic`; an empty query hides per-group rows the way the web did.

import KudzuCore
import KudzuDesign
import SwiftUI

struct CommandMenuSheet: View {
    @Environment(AppModel.self) private var model
    @Environment(LibraryActions.self) private var actions
    @Environment(\.dismiss) private var dismiss
    @State private var query = ""

    var body: some View {
        let catalogue = CommandMenuLogic.actions(
            selected: model.selectedSnippet,
            paused: model.status.paused,
            theme: model.settings.theme,
            clipboardEnabled: model.settings.clipboardEnabled,
            groups: model.library.groups
        )
        let rows = LibraryCommandRows.rows(query: query, library: model.library, usage: model.usage, actions: catalogue)
        LibraryCommandList(query: $query, rows: rows, icon: { icon($0.snippetID == nil ? $0.id : "snippet") }) { row in
            if let id = row.snippetID {
                model.setView(.all)
                model.select(id)
                model.commandOpen = false
                dismiss()
            } else {
                run(row.id)
            }
        }
        .onExitCommand { model.commandOpen = false; dismiss() }
        .frame(width: 600)
        .accessibilityLabel("Search and commands")
        .accessibilityHint("Find snippets, jump anywhere, or run an action.")
    }

    private func run(_ id: String) {
        let selected = model.selectedId
        switch id {
        case "fav": if let selected { actions.toggleFavorite(selected) }
        case "en": if let selected { actions.toggleEnabled(selected) }
        case "dup": if let selected { actions.duplicate(selected) }
        case "del": if let selected { actions.requestDelete([selected]) }
        case "new": actions.newSnippet()
        case "newg": actions.newGroup(rename: LibraryGroupCreationSource.commandMenu.startsRename)
        case "pause": actions.togglePaused()
        case "imp": later { await DataActions.runImport(.file, model: model) }
        case "exp": later { await DataActions.runExport(.json, model: model) }
        case "theme":
            let next: Theme = model.settings.theme == .dark ? .light : .dark
            Task { try? await model.updateSettings(SettingsPatch { $0.theme = next }) }
        case "keys": model.shortcutsOpen = true
        case "all": model.setView(.all)
        case "favs": model.setView(.favorites)
        case "recent": model.setView(.recent)
        case "ins": model.setView(.insights)
        case "clip": model.setView(.clipboard(filter: nil))
        case "clipset": model.setView(.settings(section: .clipboard))
        case "clippause":
            let enabled = model.settings.clipboardEnabled
            Task { try? await model.updateSettings(SettingsPatch { $0.clipboardEnabled = !enabled }) }
        case "set": model.setView(.settings(section: nil))
        case "apps": model.setView(.settings(section: .apps))
        case "data": model.setView(.settings(section: .data))
        default:
            if id.hasPrefix("gs-"), let group = group(id, dropping: 3) {
                actions.openGroupSettings(group)
            } else if id.hasPrefix("g-"), let group = group(id, dropping: 2) {
                model.setView(.group(id: group.id))
            }
        }
        model.commandOpen = false
        dismiss()
    }

    /// File panels wait until the sheet has closed, or AppKit presents them behind it.
    private func later(_ work: @escaping @MainActor () async -> Void) {
        Task { @MainActor in
            try? await Task.sleep(for: .milliseconds(250))
            await work()
        }
    }

    private func group(_ id: String, dropping prefix: Int) -> SnippetGroup? {
        guard let uuid = UUID(uuidString: String(id.dropFirst(prefix))) else { return nil }
        return model.library.groups.first { $0.id == uuid }
    }

    private func icon(_ id: String) -> Image {
        switch id {
        case "snippet": return Icon.fileText
        case "fav", "favs": return Icon.star
        case "en": return Icon.circleOff
        case "dup": return Icon.copy
        case "del": return Icon.trash2
        case "new": return Icon.plus
        case "newg": return Icon.folderPlus
        case "pause": return model.status.paused ? Icon.play : Icon.pause
        case "imp": return Icon.download
        case "exp": return Icon.upload
        case "theme": return model.settings.theme == .dark ? Icon.sun : Icon.moon
        case "keys": return Icon.keyboard
        case "all": return Icon.layers
        case "recent": return Icon.clock3
        case "ins": return Icon.barChart3
        case "clip": return Icon.clipboardList
        case "clippause": return model.settings.clipboardEnabled ? Icon.pause : Icon.play
        default:
            if id.hasPrefix("gs-") { return Icon.slidersHorizontal }
            if id.hasPrefix("g-") { return Icon.folder }
            return Icon.settings
        }
    }
}
