// Typed bridge to the Rust side. Every window talks to the backend only through this module.
// Outside Tauri (plain browser during design work) a small in-memory mock answers instead,
// so the UI is fully explorable with `pnpm dev`.

import { invoke as tauriInvoke } from "@tauri-apps/api/core"
import { listen as tauriListen, type UnlistenFn } from "@tauri-apps/api/event"

import { renderLocal } from "./render-local"
import { mockBackend } from "./mock"
import type {
  AppInfo,
  AppState,
  BackupInfo,
  ClipboardInfo,
  ClipEntry,
  EngineStatus,
  EspansoDetection,
  FormRequest,
  Group,
  ImportSummary,
  Library,
  RenderResult,
  Settings,
  Snippet,
  SyncStatus,
  TriggerConflict,
  UpdateState,
} from "./types"

export const isTauri = (): boolean =>
  typeof window !== "undefined" && "__TAURI_INTERNALS__" in window

async function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (isTauri()) return tauriInvoke<T>(cmd, args)
  return mockBackend.invoke<T>(cmd, args ?? {})
}

/** Subscribes to a backend event. Returns an unsubscribe function. */
export function on<T>(event: string, handler: (payload: T) => void): () => void {
  if (!isTauri()) return mockBackend.listen(event, handler as (p: unknown) => void)
  let unlisten: UnlistenFn | undefined
  let cancelled = false
  tauriListen<T>(event, (e) => handler(e.payload)).then((fn) => {
    if (cancelled) fn()
    else unlisten = fn
  })
  return () => {
    cancelled = true
    unlisten?.()
  }
}

export const Events = {
  library: "kudzu://library",
  settings: "kudzu://settings",
  status: "kudzu://status",
  paletteShow: "kudzu://palette-show",
  formRequest: "kudzu://form-request",
  navigate: "kudzu://navigate",
  newSnippet: "kudzu://new-snippet",
  command: "kudzu://command",
  update: "kudzu://update",
  sync: "kudzu://sync",
  clipboard: "kudzu://clipboard",
  clipboardShow: "kudzu://clipboard-show",
} as const

export const api = {
  getState: () => call<AppState>("get_state"),

  // Library
  createSnippet: (draft: Partial<Snippet>) => call<Snippet>("create_snippet", { draft }),
  saveSnippet: (snippet: Snippet) => call<Snippet>("save_snippet", { snippet }),
  deleteSnippets: (ids: string[]) => call<void>("delete_snippets", { ids }),
  restoreSnippets: (snippets: Snippet[]) => call<void>("restore_snippets", { snippets }),
  duplicateSnippet: (id: string) => call<Snippet>("duplicate_snippet", { id }),
  moveSnippets: (ids: string[], groupId: string | null) => call<void>("move_snippets", { ids, groupId }),
  saveGroup: (group: Group) => call<Group>("save_group", { group }),
  createGroup: (name: string, parentId: string | null) => call<Group>("create_group", { name, parentId }),
  deleteGroup: (id: string, deleteSnippets: boolean) => call<void>("delete_group", { id, deleteSnippets }),
  moveGroup: (id: string, parentId: string | null, index: number) =>
    call<void>("move_group", { id, parentId, index }),
  findConflicts: () => call<TriggerConflict[]>("find_conflicts"),

  // Settings & engine
  updateSettings: (patch: Partial<Settings>) => call<Settings>("update_settings", { patch }),
  setPaused: (paused: boolean) => call<EngineStatus>("set_paused", { paused }),
  requestPermission: (kind: "accessibility" | "inputMonitoring") => call<void>("request_permission", { kind }),
  openSystemSettings: (pane: "accessibility" | "inputMonitoring" | "loginItems") =>
    call<void>("open_system_settings", { pane }),
  engineStatus: () => call<EngineStatus>("engine_status"),

  // Templates
  renderPreview: (content: string, snippetId: string | null, formValues: Record<string, string>) =>
    call<RenderResult>("render_preview", { content, snippetId, formValues }),

  // Apps
  listApps: () => call<AppInfo[]>("list_apps"),
  appIcon: (path: string) => call<string | null>("app_icon", { path }),

  // Import / export / backups
  detectEspanso: () => call<EspansoDetection[]>("detect_espanso"),
  importEspanso: (path: string) => call<ImportSummary>("import_espanso", { path }),
  importJson: (path: string) => call<ImportSummary>("import_json", { path }),
  exportJson: (path: string) => call<void>("export_json", { path }),
  exportEspanso: (dir: string) => call<number>("export_espanso", { dir }),
  listBackups: () => call<BackupInfo[]>("list_backups"),
  backupNow: () => call<BackupInfo>("backup_now"),
  restoreBackup: (path: string) => call<void>("restore_backup", { path }),
  revealDataFolder: () => call<void>("reveal_data_folder"),

  // Windows
  showMain: (snippetId?: string) => call<void>("show_main", { snippetId: snippetId ?? null }),
  hidePalette: () => call<void>("hide_palette"),
  paletteInsert: (snippetId: string) => call<void>("palette_insert", { snippetId }),
  paletteCopy: (snippetId: string) => call<void>("palette_copy", { snippetId }),
  paletteCreate: (query: string) => call<void>("palette_create", { query }),
  setPaletteHotkey: (accelerator: string) => call<Settings>("set_palette_hotkey", { accelerator }),
  pendingForm: () => call<FormRequest | null>("pending_form"),
  submitForm: (token: number, values: Record<string, string>) => call<void>("submit_form", { token, values }),
  cancelForm: (token: number) => call<void>("cancel_form", { token }),
  resizeForm: (height: number) => call<void>("resize_form", { height }),

  // Updates
  appVersion: () => call<string>("app_version"),
  updateState: () => call<UpdateState>("update_state"),
  checkForUpdates: () => call<UpdateState>("check_for_updates"),
  restartToUpdate: () => call<void>("restart_to_update"),

  // iCloud sync
  syncStatus: () => call<SyncStatus>("sync_status"),
  syncNow: () => call<void>("sync_now"),

  // Clipboard history
  clipboardList: () => call<ClipboardInfo>("clipboard_list"),
  clipboardText: (id: string) => call<string | null>("clipboard_text", { id }),
  clipboardCopy: (id: string, plain = false) => call<void>("clipboard_copy", { id, plain }),
  /** From the picker: "pasted" into the previous app, or only "copied" (no Accessibility). */
  clipboardPaste: (id: string, plain = false) => call<"pasted" | "copied">("clipboard_paste", { id, plain }),
  clipboardDelete: (ids: string[]) => call<ClipEntry[]>("clipboard_delete", { ids }),
  clipboardRestore: (entries: ClipEntry[]) => call<void>("clipboard_restore", { entries }),
  clipboardPin: (id: string, pinned: boolean) => call<ClipEntry | null>("clipboard_pin", { id, pinned }),
  clipboardClear: (includePinned: boolean) => call<number>("clipboard_clear", { includePinned }),
  clipboardOpen: (id: string, index?: number) => call<void>("clipboard_open", { id, index: index ?? null }),
  clipboardReveal: (id: string, index?: number) => call<void>("clipboard_reveal", { id, index: index ?? null }),
  clipboardToSnippet: (id: string) => call<void>("clipboard_to_snippet", { id }),
  showClipboard: () => call<void>("show_clipboard"),
  hideClipboard: () => call<void>("hide_clipboard"),
  showMainView: (view: unknown) => call<void>("show_main_view", { view }),
  setClipboardHotkey: (accelerator: string) => call<Settings>("set_clipboard_hotkey", { accelerator }),
}

/**
 * URL of clipboard media served by the backend's `kzclip` protocol (thumbnails, images, file
 * previews and icons). Cached by the webview forever: ids never change content.
 */
export function clipMediaUrl(kind: "thumb" | "image" | "file" | "icon", id: string, index = 0): string | null {
  if (!isTauri()) return mockBackend.media(kind, id)
  const path = `${kind}/${id}${kind === "file" || kind === "icon" ? `/${index}` : ""}`
  // WebKit (macOS, Linux) uses the scheme directly; WebView2 maps custom schemes to http://<scheme>.localhost.
  return /Windows/.test(navigator.userAgent) ? `http://kzclip.localhost/${path}` : `kzclip://localhost/${path}`
}

export type Api = typeof api

/** Renders a preview through Rust when available, else with the local JS renderer. */
export async function renderPreview(
  content: string,
  snippetId: string | null,
  formValues: Record<string, string>,
  resolve: (id: string) => string | undefined,
): Promise<RenderResult> {
  if (isTauri()) {
    try {
      return await api.renderPreview(content, snippetId, formValues)
    } catch {
      /* fall through to local */
    }
  }
  return renderLocal(content, { formValues, resolve, clipboard: "clipboard text" })
}

export type { Library }
