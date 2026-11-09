# Editor parity — T8

- `editor/snippet-editor.tsx` — public `SnippetEditorPane` reads AppModel selection, native group/more menus, name → trigger focus, favorite/on-off shortcuts, 350 ms autosave and Cmd-S, copy, duplicate, delete confirmation, conflict links, disabled notices, live formatted preview and usage footer. Dirty drafts survive pane removal, failed saves retain selection, and explicit discard restores the saved baseline. A record deleted before a save starts is not flushed; deletion concurrent with an already-started upsert is not protected by the editor's pre-save presence check and needs coordination with the backend and deletion owner. Page/star/trigger motion uses KudzuDesign.
- `editor/behavior.tsx` — matched: native Form/LabeledContent and segmented Pickers for firing, case, insertion and app rules, cycle-safe nearest-ancestor inheritance, AppPicker, allow-list insertion overrides and formatted/plain insertion explanations. Imported explicit automatic insertion remains selectable.
- `editor/trigger-input.tsx` — matched: kept TriggerInput/TriggerEntry; whitespace/paste/Enter/Tab commits, prefix focus/blur, backspace edits the last trigger, Escape clears only pending trigger text, atomic validation, warning chips, wrapping. Fixed the invisible trailing plus consuming a second layout row.
- `editor/tag-tools.tsx` — matched: native popovers for searchable variable/nested-snippet insertion and chip configuration, slash replacement, exclusion of self references, UTF-16 replacement ranges, date/time kind/ICU presets/custom format/offset/locale, clipboard trim, cursor help, field/default/multiline, escaped choice options/default, nested-snippet search, unknown-tag preservation and removal. Every editable tag shows an actual backend-rendered example refreshed on changes and every 15 seconds. Date shifts use a native menu plus a custom field; accepted native presentation differs from the web preset buttons.
- `editor/template-editor.tsx` — matched by wiring the existing public KudzuEditor.TemplateEditor/TemplateEditorHandle without editing that module: native NSTextView, atomic chips, chip/slash callbacks, source/formatting bindings, resolver, undo and keyboard formatting. Popovers anchor using the handle's local caret geometry.
- `editor/formatting-toolbar.tsx` — matched through TemplateEditor(allowFormatting: true), which owns the native selector/toolbar, styles, link popover, list actions, clear action and Cmd-B/I/U. No duplicate feature toolbar.
- `editor/image-tools.tsx` — matched through that toolbar's existing ImageTools: native file chooser, PNG/JPEG/WebP compaction, preview, alternative text, display width, size validation and insertion through TemplateEditorHandle. No KudzuEditor edits.
- `app-picker.tsx` — matched: kept AppPicker/InstalledApps/FlowLayout/PickList, with removable app chips, installed-app search, manual reverse-DNS IDs, ordered toggling and persistent multi-selection. Fixed missing ViewBuilder annotations on PickList's retained row/empty closures.

## Integration

T12 hosts `SnippetEditorPane().environment(model)` in the detail column; no RootView or shell edits were made. Prefer `SnippetEditorPane(onRequestDelete: libraryActions.requestDelete)` to reuse T7's deletion focus and Undo toast. The standalone fallback confirms and deletes through AppModel. Window/list actions that consume or mutate the selected record should await `model.flushEditor()` and proceed only on true. Shell route changes must retain the AppModel, which owns the feature session via weak model keys. The pane also flushes on window blur/disappearance and guards the record's presence before saving.

## Verification

- All owned editor sources plus the real AppModel/MockBackend sources compile as an isolated Swift 6/macOS 14 module with no warnings.
- Ten Swift Testing tests pass: inheritance/cycle handling, app-rule descriptions, trigger atomic paste/backspace/prefix behavior, choice escaping/unknown attributes, conflict directions, save-before-selection, delete-before-flush, failed-save/discard and serialized edits during save. `EditorStandalone.swift` provides an opt-in runner (`KUDZU_EDITOR_STANDALONE`) for these same tests when other features block SwiftPM.
- `swift build` and `swift test --filter Editor` were attempted in the shared package. The full target was blocked by concurrent work outside T8: SettingsLogic tuple labels/ReleaseNotes first, then LibrarySlots public defaults referencing an internal SlotPlaceholder and AppModel+Settings references to ToastStore. Those files were not edited.
- Light/dark plain, formatted, and empty root PNGs: `/tmp/kz-t8/renders/{plain,formatted,empty}-{light,dark}.png`; corresponding `-imagerenderer.png` files retain the requested ImageRenderer output. ImageRenderer omits the NSViewRepresentable and scroll content; the companion PNGs use AppKit's actual NSHostingView/NSTextView capture. The companion PNGs were inspected and used to fix toolbar wrapping, behavior alignment, and the extra trigger row.
- SwiftUI previews cover light/dark plain, formatted and empty editor states. These renders and tests do not establish live shell integration or end-to-end interaction QA.

## Files touched

All feature paths below are under `apps/macos/Packages/KudzuKit/Sources/KudzuApp/Features/Editor/`:

- Added `SnippetEditorPane.swift`, `EditorEmpty.swift`, `EditorSession.swift`, `EditorLogic.swift`, `BehaviorSection.swift`, `InsertMenu.swift`, `ChipEditor.swift`, `EditorPreview.swift`.
- Kept and fixed `TriggerInput.swift` and `Support/PickList.swift`.
- Kept unchanged `AppPicker.swift`, `InstalledApps.swift`, `TriggerEntry.swift`, `Support/FlowLayout.swift`.
- Added `apps/macos/Packages/KudzuKit/Sources/KudzuApp/Model/AppModel+Editor.swift`.
- Added `apps/macos/Packages/KudzuKit/Tests/KudzuAppTests/EditorLogicTests.swift`, `EditorSessionTests.swift`, `EditorStandalone.swift`.
- Added this file: `apps/macos/docs/parity/editor.md`.

Nothing was staged, committed or pushed. No other feature, foundational module, shell, package manifest or integration entry point was edited.
