import Foundation
import KudzuCore

/// The palette's backend calls (`pick` in `palette.tsx`). The backend hides the palette itself
/// where the Tauri commands did: `palette_insert`, `palette_create` and `show_main` hide it before
/// acting, `palette_copy` after the copy lands, `hide_palette` hands focus back to the target app.
extension AppModel {
    public func perform(_ action: PaletteAction) async {
        switch action {
        case let .insert(id): try? await backend.paletteInsert(snippetId: id)
        case let .copy(id): try? await backend.paletteCopy(snippetId: id)
        case let .edit(id): try? await backend.showMain(snippetId: id)
        case let .create(query): try? await backend.paletteCreate(query: query)
        case .hide: try? await backend.hidePalette()
        }
    }
}
