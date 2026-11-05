// Mirrors kudzu-core `types.rs` (the Kudzu JSON contract) and the app's IPC payloads.

export type CaseMode = "propagate" | "exact"
export type FireMode = "immediate" | "wholeWord"
export type InsertionMethod = "automatic" | "type" | "paste"
export type AppRuleMode = "allow" | "deny"

export interface AppRule {
  mode: AppRuleMode
  bundleIDs: string[]
  insertion?: InsertionMethod
}

export interface Snippet {
  id: string
  name: string
  triggers: string[]
  content: string
  groupID?: string
  fireMode: FireMode
  caseMode: CaseMode
  insertion?: InsertionMethod
  rule?: AppRule
  isFavorite: boolean
  isEnabled: boolean
  createdAt: string
  updatedAt: string
}

export interface Group {
  id: string
  name: string
  parentID?: string
  sortIndex: number
  isEnabled: boolean
  insertion?: InsertionMethod
  rule?: AppRule
}

export interface UsageRecord {
  snippetID: string
  deviceID: string
  count: number
  charsSaved: number
  lastUsedAt?: string
}

export interface Library {
  formatVersion: number
  groups: Group[]
  snippets: Snippet[]
  usage: UsageRecord[]
}

export type ThemePreference = "dark" | "light" | "system"

export interface Settings {
  typingSpeedWPM: number
  defaultTriggerPrefix: string
  /** Accelerator string, e.g. "Control+Alt+Space". */
  paletteHotkey: string
  paused: boolean
  defaultInsertion: InsertionMethod
  /** Apps where nothing ever expands. */
  deniedBundleIDs: string[]
  /** Per-app insertion overrides. */
  appInsertions: Record<string, InsertionMethod>
  launchAtLogin: boolean
  onboardingCompleted: boolean
  theme: ThemePreference
  /** Show the dock icon while the main window is open (macOS). */
  showInDock: boolean
  /** Look for new versions in the background. */
  autoCheckUpdates: boolean
  /** Sync the library through the user's private iCloud. */
  icloudSync: boolean
  /** "beta" also receives pre-releases. */
  updateChannel: "stable" | "beta"
  /** App version that last ran with this data (set by the backend). */
  lastRunVersion: string | null
  /** Record clipboard history. */
  clipboardEnabled: boolean
  /** Days unpinned clipboard history is kept; 0 = until the size cap. */
  clipboardRetentionDays: number
  /** Accelerator for the clipboard history picker. */
  clipboardHotkey: string
  /** Apps whose copies are never recorded. */
  clipboardIgnoredApps: string[]
  /** Paste formatted text as plain text by default (⇧ flips it). */
  clipboardPastePlain: boolean
}

// Mirrors kudzu-core `clipboard.rs`.
export type ClipKind = "text" | "code" | "link" | "color" | "image" | "file" | "other"

export interface ClipSource {
  bundleId: string
  name: string
  path?: string | null
}

export interface ClipEntry {
  id: string
  kind: ClipKind
  /** Plain text excerpt (≤ 2,048 characters); the full text comes from `clipboardText`. */
  text?: string
  textLen: number
  lineCount: number
  files?: string[]
  image?: { width: number; height: number }
  color?: string
  rich: boolean
  types: string[]
  size: number
  source?: ClipSource
  hash: string
  firstCopiedAt: string
  copiedAt: string
  copyCount: number
  /** Most recent copy times, oldest first. */
  timings: string[]
  lastUsedAt?: string
  useCount: number
  pinned: boolean
}

export interface ClipboardInfo {
  entries: ClipEntry[]
  diskUsage: number
  canPaste: boolean
}

export type ClipEvent =
  | { type: "upsert"; entry: ClipEntry }
  | { type: "remove"; ids: string[] }
  | { type: "reset"; entries: ClipEntry[] }

/** Mirrors the Rust `SyncStatus` (tagged by `state`). */
export type SyncStatus =
  | { state: "off" }
  | { state: "unavailable"; reason: string }
  | { state: "noAccount"; reason: string }
  | { state: "starting" }
  | { state: "syncing" }
  | { state: "upToDate"; lastSync: string }
  | { state: "needsUpdate" }
  | { state: "error"; message: string }

/** Mirrors the Rust `UpdateState` (tagged by `phase`). */
export type UpdateState =
  | { phase: "idle" }
  | { phase: "checking" }
  | { phase: "upToDate"; checkedAt: string }
  | { phase: "downloading"; version: string; downloaded: number; total: number | null }
  /** Downloaded and verified; installs on quit or on "Restart to Update". */
  | { phase: "ready"; version: string; notes: string | null; date: string | null }
  | { phase: "restarting"; version: string }
  | { phase: "error"; message: string; needsMove: boolean }

export type TapMode = "off" | "listenOnly" | "active"

export interface PermissionStatus {
  accessibility: boolean
  inputMonitoring: boolean
}

export interface EngineStatus {
  canPostEvents: boolean
  canExpand: boolean
  paused: boolean
  tap: TapMode
  permissions: PermissionStatus
  secureInput: boolean
  secureInputOwner?: string | null
  summary: string
}

export interface AppInfo {
  bundleId: string
  name: string
  pid?: number | null
  path?: string | null
}

export type FormFieldKind = "text" | "multiline" | "choice"

export interface FormFieldSpec {
  name: string
  kind: FormFieldKind
  options: string[]
  defaultValue?: string | null
}

export interface FormRequest {
  token: number
  snippetId: string
  snippetName: string
  fields: FormFieldSpec[]
  initialValues: Record<string, string>
  content: string
  targetApp?: AppInfo | null
}

export interface Diagnostic {
  severity: "warning" | "error"
  message: string
  range: { start: number; end: number }
}

export interface RenderResult {
  text: string
  cursorOffset?: number | null
  diagnostics: Diagnostic[]
}

export interface TriggerConflict {
  kind: "duplicate" | "shadowing"
  snippetId: string
  trigger: string
  otherSnippetId: string
  otherTrigger: string
}

export interface BackupInfo {
  path: string
  filename: string
  createdAt: string
  reason: string
}

export interface ImportWarning {
  file?: string | null
  trigger?: string | null
  message: string
}

export interface ImportSummary {
  importedSnippets: number
  importedGroups: number
  warnings: ImportWarning[]
}

export interface EspansoDetection {
  path: string
  snippetCount: number
  groupCount: number
}

export interface AppState {
  library: Library
  settings: Settings
  status: EngineStatus
  platform: "macos" | "windows" | "linux" | "web"
}
