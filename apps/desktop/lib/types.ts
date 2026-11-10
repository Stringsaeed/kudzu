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

export type FormatStyle = { kind: "bold" | "italic" | "underline" } | { kind: "link"; url: string } | { kind: "image"; dataUrl: string; width: number }
export interface FormatSpan { start: number; end: number; style: FormatStyle }
export interface Formatting { source: string; spans: FormatSpan[] }

export interface Snippet {
  id: string
  name: string
  triggers: string[]
  content: string
  formatting?: Formatting
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
  /** Show passwords, keys and tokens in history without hovering. */
  clipboardShowSecrets: boolean
  /** Answer AI agents over MCP (local socket). */
  agentAccess: boolean
  /** Agents may create and edit snippets. */
  agentWrite: boolean
  /** Agents may search clipboard history; secrets are always withheld. */
  agentClipboard: boolean
}

/** How to connect an MCP client to this copy of Kudzu (mirrors Rust `AgentSetup`). */
export interface AgentSetup {
  running: boolean
  /** Server name to register, e.g. "kudzu". */
  name: string
  command: string
  args: string[]
  /** Ready-to-paste `claude mcp add …`. */
  claudeCommand: string
  /** `mcpServers` JSON for other MCP clients. */
  configJson: string
  skillPath: string
  skill: "missing" | "current" | "outdated"
  /** The skill's folder inside the app bundle (release builds only). */
  bundledSkillDir: string | null
}

/** Mirrors kudzu-core `secrets::SecretKind`. */
export type SecretKind = "privateKey" | "apiKey" | "token" | "password" | "credential"

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
  /** What the text looks like when it holds a password, key or token. */
  secret?: SecretKind
  /** The user's correction: false = "not a secret", true = "hide it". */
  secretOverride?: boolean
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

// Mirrors kudzu-core `activity.rs`: recent expansion attempts ("Why didn't this expand?").
// Held in memory by the backend only. An entry never contains typed or expanded text.
export type ActivitySource = "typed" | "quickSearch" | "form"
export type RuleLevel = "snippet" | "group" | "appSetting" | "default"
export type ActivityPermission = "accessibility" | "inputMonitoring"
export type InsertFailure = "notAllowed" | "eventCreation" | "pasteUnavailable" | "targetGone"

export type ActivityOutcome =
  | { kind: "expanded" | "undone" | "paused" | "snippetOff" | "formCancelled" | "interrupted" | "listenerLost" | "engineStopped" }
  | { kind: "groupOff"; groupId: string }
  | { kind: "appRule"; level: RuleLevel; groupId?: string | null }
  | { kind: "secureInput"; owner?: string | null }
  | { kind: "missingPermission"; permission: ActivityPermission }
  | { kind: "insertionFailed"; error: InsertFailure }
  | { kind: "tapDisabled"; cause: "timeout" | "userInput"; reenabled: boolean }

export type ActivityFix =
  | { kind: "resume" | "turnOnSnippet" | "openSnippet" | "openAppSettings" | "explainSecureInput" }
  | { kind: "openGroup"; groupId: string }
  | { kind: "openPermissions"; permission: ActivityPermission }

export interface ActivityExplanation {
  /** Badge text. */
  label: string
  tone: "ok" | "info" | "skipped" | "blocked" | "failed"
  /** One plain sentence; absent when it simply expanded. */
  sentence?: string | null
  fix?: ActivityFix | null
}

export interface ActivityEntry {
  id: number
  /** ISO-8601 time of the attempt. */
  at: string
  /** Absent for engine health entries. */
  source?: ActivitySource | null
  snippetId?: string | null
  /** The snippet's stored trigger, never the text as typed. */
  trigger?: string | null
  app?: { bundleId: string; name: string; path?: string | null } | null
  outcome: ActivityOutcome
  explanation: ActivityExplanation
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
  /** Text selected in the target app, when the snippet was picked in Quick Search. */
  selection?: string | null
}

export interface Diagnostic {
  severity: "warning" | "error"
  message: string
  range: { start: number; end: number }
}

/** A key press or pause from `{{key}}` / `{{delay}}`, after the first `offset` UTF-16 units of the text. */
export type RenderAction =
  | { offset: number; kind: "key"; key: string; count: number }
  | { offset: number; kind: "pause"; ms: number }

export interface RenderResult {
  text: string
  formatting?: FormatSpan[] | null
  cursorOffset?: number | null
  diagnostics: Diagnostic[]
  /** In insertion order; never part of `text`. */
  actions?: RenderAction[]
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
  /** What deduplication merged away; `importedSnippets - mergedSnippets` is roughly what was new. */
  dedupeReport?: { mergedSnippets: number }
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
