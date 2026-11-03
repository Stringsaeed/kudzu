// In-browser stand-in for the Rust backend, used only outside Tauri (design work, demos).

import { renderLocal } from "./render-local"
import type { AppInfo, AppState, EngineStatus, Group, Library, Settings, Snippet } from "./types"

type Handler = (payload: unknown) => void

const uuid = () => crypto.randomUUID()
const iso = (d = new Date()) => d.toISOString().replace(/\.\d{3}Z$/, "Z")
const daysAgo = (n: number) => iso(new Date(Date.now() - n * 86_400_000))

function seed(): Library {
  const work: Group = { id: uuid(), name: "Work", sortIndex: 0, isEnabled: true }
  const email: Group = { id: uuid(), name: "Email", parentID: work.id, sortIndex: 0, isEnabled: true }
  const dev: Group = { id: uuid(), name: "Developer", sortIndex: 1, isEnabled: true }
  const personal: Group = { id: uuid(), name: "Personal", sortIndex: 2, isEnabled: true }
  const mk = (
    name: string,
    triggers: string[],
    content: string,
    extra: Partial<Snippet> = {},
  ): Snippet => ({
    id: uuid(),
    name,
    triggers,
    content,
    fireMode: "immediate",
    caseMode: "propagate",
    isFavorite: false,
    isEnabled: true,
    createdAt: daysAgo(40),
    updatedAt: daysAgo(Math.floor(Math.random() * 20)),
    ...extra,
  })
  const sig = mk("Email signature", [";sig"], "Best regards,\nSaeed\n{{date format=\"MMMM d, yyyy\"}}", {
    groupID: email.id,
    isFavorite: true,
  })
  const snippets: Snippet[] = [
    sig,
    mk("Meeting follow-up", [";fu", ";followup"], "Hi {{field name=\"Name\"}},\n\nThanks for the time today. As discussed, {{cursor}}\n\n{{snippet id=\"" + sig.id + "\"}}", { groupID: email.id }),
    mk("Out of office", [";ooo"], "I'm out of the office until {{date format=\"EEEE, MMMM d\" offset=\"+1w\"}} with limited access to email.", { groupID: email.id }),
    mk("Today", [";date"], "{{date}}", { isFavorite: true }),
    mk("Current time", [";now"], "{{time format=\"h:mm a\"}}"),
    mk("Shrug", [";shrug"], "¯\\_(ツ)_/¯", { isFavorite: true }),
    mk("Paste as quote", [";quote"], "> {{clipboard trim=true}}"),
    mk("Bug report", [";bug"], "**Environment:** {{choice name=\"Platform\" options=\"macOS|Windows|Linux\"}}\n**Steps:**\n1. {{cursor}}\n**Expected:**\n**Actual:**", { groupID: dev.id }),
    mk("Console log", [";cl"], "console.log({{clipboard}})", { groupID: dev.id, fireMode: "wholeWord", caseMode: "exact" }),
    mk("Git commit co-author", [";coa"], "Co-authored-by: {{field name=\"Author\"}} <{{field name=\"Email\"}}>", { groupID: dev.id }),
    mk("Home address", [";addr"], "221B Baker Street\nLondon NW1 6XE\nUnited Kingdom", { groupID: personal.id }),
    mk("Phone number", [";ph"], "+44 20 7946 0958", { groupID: personal.id }),
    mk("Thanks", [";ty"], "Thank you so much — really appreciate it!"),
    mk("Calendar link", [";cal"], "You can grab a time that works here: https://cal.com/saeed/30min", { groupID: work.id }),
    mk("Lorem ipsum", [";lorem"], "Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua.", { isEnabled: false }),
  ]
  const usage = snippets.slice(0, 12).map((s, i) => ({
    snippetID: s.id,
    deviceID: "mock",
    count: Math.max(1, Math.round(180 / (i + 1.4))),
    charsSaved: Math.round((s.content.length - s.triggers[0].length) * (180 / (i + 1.4))),
    lastUsedAt: daysAgo(i * 1.7),
  }))
  return { formatVersion: 1, groups: [work, email, dev, personal], snippets, usage }
}

const defaultSettings: Settings = {
  typingSpeedWPM: 40,
  defaultTriggerPrefix: ";",
  paletteHotkey: "Control+Alt+Space",
  paused: false,
  defaultInsertion: "automatic",
  deniedBundleIDs: ["com.1password.1password", "com.apple.keychainaccess"],
  appInsertions: { "com.apple.Terminal": "paste", "com.googlecode.iterm2": "paste" },
  launchAtLogin: true,
  onboardingCompleted: true,
  theme: "dark",
  showInDock: true,
  autoCheckUpdates: true,
  icloudSync: true,
  updateChannel: "stable",
  lastRunVersion: "0.1.0",
}

const apps: AppInfo[] = [
  ["com.apple.mail", "Mail"],
  ["com.apple.Safari", "Safari"],
  ["com.google.Chrome", "Google Chrome"],
  ["com.tinyspeck.slackmacgap", "Slack"],
  ["com.microsoft.VSCode", "Visual Studio Code"],
  ["com.apple.Terminal", "Terminal"],
  ["com.googlecode.iterm2", "iTerm"],
  ["notion.id", "Notion"],
  ["com.linear", "Linear"],
  ["com.1password.1password", "1Password"],
  ["com.apple.keychainaccess", "Keychain Access"],
  ["com.apple.Notes", "Notes"],
].map(([bundleId, name]) => ({ bundleId, name, path: `/Applications/${name}.app` }))

class MockBackend {
  private state: AppState
  private listeners = new Map<string, Set<Handler>>()

  constructor() {
    const stored = typeof localStorage !== "undefined" ? localStorage.getItem("kudzu-mock") : null
    const library = stored ? (JSON.parse(stored) as Library) : seed()
    const settings = { ...defaultSettings, ...JSON.parse((typeof localStorage !== "undefined" && localStorage.getItem("kudzu-mock-settings")) || "{}") }
    if (typeof location !== "undefined" && location.search.includes("light")) settings.theme = "light"
    this.state = {
      library,
      settings,
      status: this.status(settings.paused),
      platform: "web",
    }
  }

  private status(paused: boolean): EngineStatus {
    return {
      paused,
      tap: "active",
      permissions: { accessibility: true, inputMonitoring: true },
      secureInput: false,
      secureInputOwner: null,
      summary: paused ? "Paused" : "Ready",
    }
  }

  listen(event: string, handler: Handler) {
    if (!this.listeners.has(event)) this.listeners.set(event, new Set())
    this.listeners.get(event)!.add(handler)
    return () => void this.listeners.get(event)?.delete(handler)
  }

  private emit(event: string, payload: unknown) {
    this.listeners.get(event)?.forEach((h) => h(payload))
  }

  private commit() {
    localStorage.setItem("kudzu-mock", JSON.stringify(this.state.library))
    this.emit("kudzu://library", structuredClone(this.state.library))
  }

  private commitSettings() {
    localStorage.setItem("kudzu-mock-settings", JSON.stringify(this.state.settings))
    this.emit("kudzu://settings", { ...this.state.settings })
  }

  async invoke<T>(cmd: string, a: Record<string, unknown>): Promise<T> {
    const lib = this.state.library
    const r = (v: unknown) => structuredClone(v) as T
    switch (cmd) {
      case "get_state":
        return r(this.state)
      case "create_snippet": {
        const now = iso()
        const s: Snippet = {
          id: uuid(),
          name: "",
          triggers: [],
          content: "",
          fireMode: "immediate",
          caseMode: "propagate",
          isFavorite: false,
          isEnabled: true,
          createdAt: now,
          updatedAt: now,
          ...(a.draft as Partial<Snippet>),
        }
        lib.snippets.unshift(s)
        this.commit()
        return r(s)
      }
      case "save_snippet": {
        const s = { ...(a.snippet as Snippet), updatedAt: iso() }
        lib.snippets = lib.snippets.map((x) => (x.id === s.id ? s : x))
        this.commit()
        return r(s)
      }
      case "delete_snippets": {
        const ids = new Set(a.ids as string[])
        lib.snippets = lib.snippets.filter((s) => !ids.has(s.id))
        this.commit()
        return r(undefined)
      }
      case "restore_snippets":
        lib.snippets.push(...(a.snippets as Snippet[]))
        this.commit()
        return r(undefined)
      case "duplicate_snippet": {
        const src = lib.snippets.find((s) => s.id === a.id)!
        const copy = { ...src, id: uuid(), name: `${src.name} copy`, triggers: src.triggers.map((t) => `${t}2`), createdAt: iso(), updatedAt: iso() }
        lib.snippets.unshift(copy)
        this.commit()
        return r(copy)
      }
      case "move_snippets": {
        const ids = new Set(a.ids as string[])
        lib.snippets = lib.snippets.map((s) => (ids.has(s.id) ? { ...s, groupID: (a.groupId as string) ?? undefined, updatedAt: iso() } : s))
        this.commit()
        return r(undefined)
      }
      case "create_group": {
        const g: Group = { id: uuid(), name: a.name as string, parentID: (a.parentId as string) ?? undefined, sortIndex: lib.groups.length, isEnabled: true }
        lib.groups.push(g)
        this.commit()
        return r(g)
      }
      case "save_group": {
        const g = a.group as Group
        lib.groups = lib.groups.map((x) => (x.id === g.id ? g : x))
        this.commit()
        return r(g)
      }
      case "delete_group": {
        const id = a.id as string
        const doomed = new Set([id])
        let grew = true
        while (grew) {
          grew = false
          for (const g of lib.groups)
            if (g.parentID && doomed.has(g.parentID) && !doomed.has(g.id)) {
              doomed.add(g.id)
              grew = true
            }
        }
        lib.groups = lib.groups.filter((g) => !doomed.has(g.id))
        lib.snippets = a.deleteSnippets
          ? lib.snippets.filter((s) => !s.groupID || !doomed.has(s.groupID))
          : lib.snippets.map((s) => (s.groupID && doomed.has(s.groupID) ? { ...s, groupID: undefined } : s))
        this.commit()
        return r(undefined)
      }
      case "move_group": {
        lib.groups = lib.groups.map((g) => (g.id === a.id ? { ...g, parentID: (a.parentId as string) ?? undefined, sortIndex: a.index as number } : g))
        this.commit()
        return r(undefined)
      }
      case "find_conflicts":
        return r([])
      case "update_settings":
      case "set_palette_hotkey": {
        const patch = cmd === "update_settings" ? (a.patch as Partial<Settings>) : { paletteHotkey: a.accelerator as string }
        this.state.settings = { ...this.state.settings, ...patch }
        this.commitSettings()
        if ("icloudSync" in patch)
          this.emit("kudzu://sync", patch.icloudSync ? { state: "upToDate", lastSync: new Date().toISOString() } : { state: "off" })
        return r(this.state.settings)
      }
      case "set_paused":
        this.state.settings.paused = a.paused as boolean
        this.state.status = this.status(this.state.settings.paused)
        this.commitSettings()
        this.emit("kudzu://status", this.state.status)
        return r(this.state.status)
      case "engine_status":
        return r(this.state.status)
      case "render_preview":
        return r(
          renderLocal(a.content as string, {
            formValues: a.formValues as Record<string, string>,
            clipboard: "copied text",
            resolve: (id) => lib.snippets.find((s) => s.id === id)?.content,
          }),
        )
      case "list_apps":
        return r(apps)
      case "app_icon":
        return r(null)
      case "detect_espanso":
        return r([])
      case "list_backups":
        return r([
          { path: "/backups/a.json", filename: "kudzu-2026-11-03-daily.json", createdAt: daysAgo(0), reason: "daily" },
          { path: "/backups/b.json", filename: "kudzu-2026-11-02-daily.json", createdAt: daysAgo(1), reason: "daily" },
          { path: "/backups/c.json", filename: "kudzu-2026-11-01-preImport.json", createdAt: daysAgo(2), reason: "preImport" },
        ])
      // Updates: ?update shows an available version; otherwise "up to date" after a short check.
      case "app_version":
        return r("0.1.0")
      case "update_state":
      case "check_for_updates": {
        const next = location.search.includes("update")
          ? { phase: "ready", version: "0.2.0", notes: "## [0.2.0](https://example.com) (2026-11-04)\n\n### Features\n\n* **palette:** faster search tray ([4143658](https://example.com))\n* new keycap shortcuts ([c818cef](https://example.com))\n\n### Fixes\n\n* keep the titlebar transparent", date: new Date().toISOString() }
          : cmd === "update_state"
            ? { phase: "idle" }
            : { phase: "upToDate", checkedAt: new Date().toISOString() }
        return new Promise((res) => setTimeout(() => res(next as T), cmd === "update_state" ? 0 : 700))
      }
      case "restart_to_update":
        return r(undefined)
      case "sync_status":
        return r(
          !this.state.settings.icloudSync
            ? { state: "off" }
            : location.search.includes("nosync")
              ? { state: "unavailable", reason: "This copy of Kudzu isn't signed for iCloud. Release builds sync; development builds stay on this Mac." }
              : { state: "upToDate", lastSync: new Date(Date.now() - 2 * 60_000).toISOString() },
        )
      case "sync_now":
        return r(undefined)
      case "pending_form": {
        const s = lib.snippets.find((x) => x.content.includes("{{field"))
        if (!s || !location.search.includes("demo")) return r(null)
        return r({
          token: 1,
          snippetId: s.id,
          snippetName: s.name,
          content: s.content,
          fields: [...s.content.matchAll(/\{\{(field|choice) name="([^"]+)"(?: options="([^"]+)")?/g)].map((m) => ({
            name: m[2],
            kind: m[1] === "choice" ? "choice" : "text",
            options: m[3] ? m[3].split("|") : [],
            defaultValue: null,
          })),
          initialValues: {},
          targetApp: { bundleId: "com.apple.mail", name: "Mail" },
        })
      }
      default:
        console.info(`[mock] ${cmd}`, a)
        return r(undefined)
    }
  }
}

export const mockBackend = typeof window === "undefined" ? (null as unknown as MockBackend) : new MockBackend()
