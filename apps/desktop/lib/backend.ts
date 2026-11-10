// Typed bridge to the Rust side. Every window talks to the backend only through this module.
// Outside Tauri (plain browser during design work) a small in-memory mock answers instead,
// so the UI is fully explorable with `pnpm dev`.

import { invoke as tauriInvoke } from "@tauri-apps/api/core"
import { listen as tauriListen, type UnlistenFn } from "@tauri-apps/api/event"

import { formattedHtml } from "./formatting"
import { renderLocal } from "./render-local"
import { mockBackend } from "./mock"
import type {
  ActivityEntry,
  AgentSetup,
  AppInfo,
  AppState,
  BackupInfo,
  ClipboardInfo,
  ClipEntry,
  EngineStatus,
  EspansoDetection,
  FormRequest,
  Formatting,
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
  permissionShow: "kudzu://permission-show",
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
  /** One new `ActivityEntry`. */
  activity: "kudzu://activity",
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
  showPermissions: (kind: "accessibility" | "inputMonitoring" = "accessibility") =>
    // A full navigation gives the browser preview the same isolated state as a new native window.
    // oxlint-disable-next-line nextjs/no-location-assign-relative-destination
    isTauri() ? call<void>("show_permissions", { kind }) : Promise.resolve(location.assign(`/permissions?kind=${kind}`)),
  permissionContext: () => call<{ kind: "accessibility" | "inputMonitoring"; appName: string; accessibilityName: string }>("permission_context"),
  installPermissionIcon: (bounds: { x: number; y: number; width: number; height: number }) => call<boolean>("install_permission_icon", bounds),
  beginPermission: (kind: "accessibility" | "inputMonitoring") => call<void>("begin_permission", { kind }),
  stopPermissionGuide: () => call<void>("stop_permission_guide"),
  restartPermissionApp: () => call<void>("restart_permission_app"),
  // oxlint-disable-next-line nextjs/no-location-assign-relative-destination -- return from the isolated window preview
  closePermissions: () => isTauri() ? call<void>("close_permissions") : Promise.resolve(location.assign("/?onboarding")),
  revealPermissionApp: () => call<void>("reveal_permission_app"),
  updateSettings: (patch: Partial<Settings>) => call<Settings>("update_settings", { patch }),
  setPaused: (paused: boolean) => call<EngineStatus>("set_paused", { paused }),
  requestPermission: (kind: "accessibility" | "inputMonitoring") => call<void>("request_permission", { kind }),
  openSystemSettings: (pane: "accessibility" | "inputMonitoring" | "loginItems") =>
    call<void>("open_system_settings", { pane }),
  engineStatus: () => call<EngineStatus>("engine_status"),
  /** Recent expansion attempts, newest first. Memory only; cleared when Kudzu quits. */
  activityList: () => call<ActivityEntry[]>("activity_list"),
  activityClear: () => call<void>("activity_clear"),

  // Templates
  renderPreview: (content: string, snippetId: string | null, formValues: Record<string, string>, formatting?: Formatting, selection?: string | null) =>
    call<RenderResult>("render_preview", { content, snippetId, formValues, formatting: formatting ?? null, selection: selection ?? null }),
  copyPreview: (content: string, snippetId: string | null, formatting?: Formatting) =>
    call<void>("copy_preview", { content, snippetId, formatting: formatting ?? null }),

  // Apps
  listApps: () => call<AppInfo[]>("list_apps"),
  appIcon: (path: string) => call<string | null>("app_icon", { path }),

  // Import / export / backups
  detectEspanso: () => call<EspansoDetection[]>("detect_espanso"),
  importEspanso: (path: string) => call<ImportSummary>("import_espanso", { path }),
  importFile: (path: string) => call<ImportSummary>("import_file", { path }),
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

  // AI agents (MCP)
  agentSetup: () => call<AgentSetup>("agent_setup"),
  /** Writes the Claude Code skill; resolves to its path. */
  installAgentSkill: () => call<string>("install_agent_skill"),

  // Clipboard history
  clipboardList: () => call<ClipboardInfo>("clipboard_list"),
  clipboardText: (id: string) => call<string | null>("clipboard_text", { id }),
  clipboardCopy: (id: string, plain = false) => call<void>("clipboard_copy", { id, plain }),
  /** From the picker: "pasted" into the previous app, or only "copied" (no Accessibility). */
  clipboardPaste: (id: string, plain = false) => call<"pasted" | "copied">("clipboard_paste", { id, plain }),
  clipboardDelete: (ids: string[]) => call<ClipEntry[]>("clipboard_delete", { ids }),
  clipboardRestore: (entries: ClipEntry[]) => call<void>("clipboard_restore", { entries }),
  clipboardPin: (id: string, pinned: boolean) => call<ClipEntry | null>("clipboard_pin", { id, pinned }),
  /** false = "not a secret", true = "hide it", null = back to detection. */
  clipboardSetSecret: (id: string, secret: boolean | null) => call<ClipEntry | null>("clipboard_set_secret", { id, secret }),
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
  formatting?: Formatting,
  resolveFormatting?: (id: string) => Formatting | undefined,
  /** What `{{selection}}` shows: the captured selection (Quick Search, form) or a sample (editor). */
  selection?: string | null,
): Promise<RenderResult> {
  if (isTauri()) {
    try {
      return await api.renderPreview(content, snippetId, formValues, formatting, selection)
    } catch {
      /* fall through to local */
    }
  }
  return renderLocal(content, {
    formValues,
    resolve,
    formatting,
    resolveFormatting,
    clipboard: "clipboard text",
    clipboardHistory: ["previous copy", "an older copy"],
    selection: selection ?? undefined,
  })
}

/** Stand-in for `{{selection}}` in the editor's preview, where nothing is selected. */
export const SAMPLE_SELECTION = "selected text"

export type { Library }

export async function copyPreview(content: string, snippetId: string, formatting: Formatting | undefined, resolve: (id: string) => string | undefined, resolveFormatting?: (id: string) => Formatting | undefined) {
  if (isTauri()) return api.copyPreview(content, snippetId, formatting)
  const result = renderLocal(content, { formatting, resolve, resolveFormatting, clipboard: "clipboard text" })
  if (result.formatting) {
    await navigator.clipboard.write([new ClipboardItem({
      "text/plain": new Blob([result.text], { type: "text/plain" }),
      "text/html": new Blob([formattedHtml(result.text, result.formatting)], { type: "text/html" }),
    })])
  } else await navigator.clipboard.writeText(result.text)
}
