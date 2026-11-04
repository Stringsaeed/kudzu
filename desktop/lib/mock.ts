// In-browser stand-in for the Rust backend, used only outside Tauri (design work, demos).

import { renderLocal } from "./render-local"
import type { AppInfo, AppState, ClipEntry, ClipKind, EngineStatus, Group, Library, Settings, Snippet } from "./types"

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
  clipboardEnabled: true,
  clipboardRetentionDays: 7,
  clipboardHotkey: "Control+Alt+V",
  clipboardIgnoredApps: ["com.1password.1password", "com.apple.keychainaccess", "com.apple.Passwords"],
  clipboardPastePlain: false,
}

// MARK: Clipboard history seed

const sources = {
  safari: { bundleId: "com.apple.Safari", name: "Safari", path: "/Applications/Safari.app" },
  code: { bundleId: "com.microsoft.VSCode", name: "Visual Studio Code", path: "/Applications/Visual Studio Code.app" },
  slack: { bundleId: "com.tinyspeck.slackmacgap", name: "Slack", path: "/Applications/Slack.app" },
  finder: { bundleId: "com.apple.finder", name: "Finder", path: "/System/Library/CoreServices/Finder.app" },
  figma: { bundleId: "com.figma.Desktop", name: "Figma", path: "/Applications/Figma.app" },
  terminal: { bundleId: "com.apple.Terminal", name: "Terminal", path: "/System/Applications/Utilities/Terminal.app" },
  mail: { bundleId: "com.apple.mail", name: "Mail", path: "/System/Applications/Mail.app" },
  notes: { bundleId: "com.apple.Notes", name: "Notes", path: "/System/Applications/Notes.app" },
  preview: { bundleId: "com.apple.Preview", name: "Preview", path: "/System/Applications/Preview.app" },
}

/** A soft gradient "photo" or a fake screenshot, as an SVG data URL. */
function mockImage(w: number, h: number, hue: number, screenshot: boolean): string {
  const body = screenshot
    ? `<rect width="${w}" height="${h}" fill="hsl(${hue} 14% 12%)"/><rect x="${w * 0.04}" y="${h * 0.06}" width="${w * 0.22}" height="${h * 0.88}" rx="18" fill="hsl(${hue} 14% 17%)"/>` +
      Array.from({ length: 6 }, (_, i) => `<rect x="${w * 0.07}" y="${h * (0.14 + i * 0.1)}" width="${w * (0.12 + (i % 3) * 0.03)}" height="${h * 0.03}" rx="6" fill="hsl(${hue} 10% ${i === 1 ? 55 : 30}%)"/>`).join("") +
      `<rect x="${w * 0.3}" y="${h * 0.06}" width="${w * 0.66}" height="${h * 0.4}" rx="22" fill="hsl(${hue} 60% 55%)"/><rect x="${w * 0.3}" y="${h * 0.52}" width="${w * 0.32}" height="${h * 0.42}" rx="22" fill="hsl(${hue + 40} 30% 22%)"/><rect x="${w * 0.64}" y="${h * 0.52}" width="${w * 0.32}" height="${h * 0.42}" rx="22" fill="hsl(${hue - 30} 30% 22%)"/>`
    : `<defs><linearGradient id="g" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stop-color="hsl(${hue} 70% 62%)"/><stop offset="1" stop-color="hsl(${hue + 70} 60% 35%)"/></linearGradient></defs><rect width="${w}" height="${h}" fill="url(#g)"/><circle cx="${w * 0.72}" cy="${h * 0.3}" r="${h * 0.12}" fill="hsl(${hue + 20} 90% 85% / .8)"/><path d="M0 ${h * 0.78} Q ${w * 0.3} ${h * 0.55} ${w * 0.55} ${h * 0.72} T ${w} ${h * 0.62} V ${h} H 0 Z" fill="hsl(${hue + 120} 35% 18% / .75)"/>`
  return `data:image/svg+xml;utf8,${encodeURIComponent(`<svg xmlns="http://www.w3.org/2000/svg" width="${w}" height="${h}" viewBox="0 0 ${w} ${h}">${body}</svg>`)}`
}

function seedClips(media: Map<string, string>): ClipEntry[] {
  const now = Date.now()
  const hours = (h: number) => new Date(now - h * 3_600_000).toISOString()
  const mk = (kind: ClipKind, at: number, extra: Partial<ClipEntry> = {}, repeats: number[] = []): ClipEntry => {
    const times = [...repeats, at].sort((a, b) => b - a).map(hours)
    const text = extra.text
    return {
      id: uuid(),
      kind,
      textLen: text?.length ?? 0,
      lineCount: text ? text.split("\n").length : 0,
      rich: false,
      types: ["public.utf8-plain-text"],
      size: text?.length ?? 0,
      hash: uuid(),
      firstCopiedAt: times[0],
      copiedAt: times[times.length - 1],
      copyCount: times.length,
      timings: times,
      useCount: 0,
      pinned: false,
      ...extra,
    }
  }
  const image = (at: number, w: number, h: number, hue: number, shot: boolean, source = sources.preview, extra: Partial<ClipEntry> = {}) => {
    const e = mk("image", at, { image: { width: w, height: h }, types: ["public.png", "public.tiff"], size: Math.round(w * h * 0.9), source, ...extra })
    const url = mockImage(w, h, hue, shot)
    media.set(`thumb/${e.id}`, url)
    media.set(`image/${e.id}`, url)
    return e
  }
  const entries: ClipEntry[] = [
    mk("text", 0.01, { text: "Can we move the design review to Thursday at 3? Happy to send a new invite.", source: sources.slack, rich: true, types: ["public.utf8-plain-text", "public.html"] }),
    mk("link", 0.2, { text: "https://github.com/stringsaeed/kudzu/pull/128", source: sources.safari, types: ["public.utf8-plain-text", "public.url"] }, [5, 26]),
    mk("code", 0.6, {
      text: `export function debounce<T extends (...args: never[]) => void>(fn: T, ms = 120) {
  let t: ReturnType<typeof setTimeout> | undefined
  return (...args: Parameters<T>) => {
    clearTimeout(t)
    t = setTimeout(() => fn(...args), ms)
  }
}`,
      source: sources.code,
    }),
    mk("color", 1.1, { text: "#5B7CFA", color: "#5b7cfa", source: sources.figma }, [2, 30]),
    image(1.6, 2880, 1800, 230, true, sources.preview),
    mk("code", 2.3, { text: "pnpm tauri build --config src-tauri/tauri.dev.conf.json --bundles app", source: sources.terminal }, [3, 50, 51]),
    mk("file", 3.4, { files: ["/Users/saeed/Documents/Contracts/Kudzu — Services Agreement.pdf"], types: ["public.file-url", "public.utf8-plain-text"], size: 412, source: sources.finder, text: "/Users/saeed/Documents/Contracts/Kudzu — Services Agreement.pdf" }),
    mk("text", 5.2, {
      text: `Hi Maya,

Thanks for the quick turnaround on the contract. I've signed and attached it — let me know if you need anything else from our side before kickoff on Monday.

Best,
Saeed`,
      source: sources.mail,
      rich: true,
      types: ["public.utf8-plain-text", "public.rtf", "public.html"],
      pinned: true,
    }),
    mk("color", 7, { text: "oklch(0.72 0.16 65)", color: "oklch(0.72 0.16 65)", source: sources.code }),
    mk("link", 9, { text: "https://www.figma.com/design/x7Gq/Kudzu-Clipboard?node-id=12-2048", source: sources.figma }),
    image(20, 1600, 1067, 18, false, sources.safari, { types: ["public.jpeg", "public.tiff"] }),
    mk("text", 23, { text: "221B Baker Street, London NW1 6XE", source: sources.notes, pinned: true }, [80, 120]),
    mk("file", 26, {
      files: ["/Users/saeed/Desktop/hero@2x.png", "/Users/saeed/Desktop/hero-dark@2x.png", "/Users/saeed/Desktop/icon.svg"],
      types: ["public.file-url"],
      size: 220,
      source: sources.finder,
      text: `hero@2x.png
hero-dark@2x.png
icon.svg`,
    }),
    mk("other", 29, { types: ["com.figma.clipboard", "public.html", "public.png"], size: 182_311, source: sources.figma }),
    mk("code", 31, { text: `SELECT id, email, created_at
FROM users
WHERE created_at > now() - interval '7 days'
ORDER BY created_at DESC;`, source: sources.code }),
    mk("text", 47, { text: "INV-2026-0412", source: sources.mail }, [48, 60]),
    mk("link", 52, { text: "mailto:hello@kudzu.app", source: sources.mail }),
    image(70, 1170, 2532, 280, true, sources.slack),
    mk("text", 75, { text: "Ship it! 🚀 Great work everyone — the new onboarding flow cut drop-off by 18%.", source: sources.slack }),
    mk("color", 98, { text: "rgb(34, 197, 94)", color: "rgb(34, 197, 94)", source: sources.safari }),
    mk("code", 120, { text: `{
  "name": "kudzu-desktop",
  "private": true,
  "packageManager": "pnpm@12.16.1"
}`, source: sources.code }),
    mk("text", 140, { text: "The quick brown fox jumps over the lazy dog.", source: sources.notes }),
    mk("link", 150, { text: "https://developer.apple.com/documentation/appkit/nspasteboard", source: sources.safari }),
  ]
  // Lots of older plain copies, to exercise virtualization and day sections.
  const filler = ["Looks good to me 👍", "On my way", "+44 20 7946 0958", "Meeting notes — Q4 planning", "TODO: follow up with legal", "ref: KZ-482", "4111 1111 1111 1111"]
  for (let i = 0; i < 60; i++) entries.push(mk("text", 26 + i * 2.3, { text: `${filler[i % filler.length]}${i > 6 ? ` (${i})` : ""}`, source: [sources.slack, sources.notes, sources.mail][i % 3] }))
  // ?clips=5000 fills the history to its cap, for performance checks.
  const extra = Number(new URLSearchParams(location.search).get("clips") ?? 0)
  const words = "alpha beta gamma delta invoice meeting design review contract notes kudzu snippet paste copy deploy release".split(" ")
  for (let i = 0; i < extra; i++) {
    const text = Array.from({ length: 6 + (i % 40) }, (_, j) => words[(i * 7 + j * 3) % words.length]).join(" ") + ` #${i}`
    entries.push(mk(i % 9 === 0 ? "code" : "text", 0.5 + (i / Math.max(1, extra)) * 160, { text, source: [sources.slack, sources.notes, sources.mail, sources.code][i % 4] }))
  }
  for (const e of entries) e.size ||= e.text?.length ?? 0
  return entries.sort((a, b) => (a.copiedAt < b.copiedAt ? 1 : -1))
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
  private mediaUrls = new Map<string, string>()
  private clips: ClipEntry[] = []

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
    this.clips = location.search.includes("emptyclips") ? [] : seedClips(this.mediaUrls)
    // Copying in the preview records it, like the real watcher.
    document.addEventListener("copy", () => {
      const text = String(window.getSelection() ?? "").trim()
      if (text && this.state.settings.clipboardEnabled) setTimeout(() => this.recordCopy(text), 0)
    })
  }

  /** Mock `kzclip://` URLs. */
  media(kind: string, id: string): string | null {
    return this.mediaUrls.get(`${kind}/${id}`) ?? null
  }

  private recordCopy(text: string) {
    const now = new Date().toISOString()
    const existing = this.clips.find((c) => c.text === text)
    const entry: ClipEntry = existing
      ? { ...existing, copiedAt: now, copyCount: existing.copyCount + 1, timings: [...existing.timings, now].slice(-64) }
      : {
          id: uuid(),
          kind: /^https?:\/\//.test(text) && !/\s/.test(text) ? "link" : /^#[0-9a-f]{3,8}$/i.test(text) ? "color" : "text",
          text,
          textLen: text.length,
          lineCount: text.split("\n").length,
          color: /^#[0-9a-f]{3,8}$/i.test(text) ? text.toLowerCase() : undefined,
          rich: false,
          types: ["public.utf8-plain-text"],
          size: text.length,
          source: { bundleId: "sh.saeed.kudzu", name: "Kudzu" },
          hash: uuid(),
          firstCopiedAt: now,
          copiedAt: now,
          copyCount: 1,
          timings: [now],
          useCount: 0,
          pinned: false,
        }
    this.clips = [entry, ...this.clips.filter((c) => c.id !== entry.id)]
    this.emit("kudzu://clipboard", { type: "upsert", entry: structuredClone(entry) })
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
      case "clipboard_list":
        return r({ entries: this.clips, diskUsage: 38_400_000, canPaste: true })
      case "clipboard_text":
        return r(this.clips.find((c) => c.id === a.id)?.text ?? null)
      case "clipboard_copy":
      case "clipboard_paste": {
        const c = this.clips.find((x) => x.id === a.id)
        if (c) {
          const now = new Date().toISOString()
          const used = { ...c, copiedAt: now, lastUsedAt: now, useCount: c.useCount + 1 }
          this.clips = [used, ...this.clips.filter((x) => x.id !== c.id)]
          this.emit("kudzu://clipboard", { type: "upsert", entry: structuredClone(used) })
          if (c.text) void navigator.clipboard?.writeText(c.text).catch(() => {})
        }
        return r(cmd === "clipboard_paste" ? "pasted" : undefined)
      }
      case "clipboard_delete": {
        const ids = new Set(a.ids as string[])
        const gone = this.clips.filter((c) => ids.has(c.id))
        this.clips = this.clips.filter((c) => !ids.has(c.id))
        this.emit("kudzu://clipboard", { type: "remove", ids: [...ids] })
        return r(gone)
      }
      case "clipboard_restore":
        this.clips = [...this.clips, ...(a.entries as ClipEntry[])].sort((x, y) => (x.copiedAt < y.copiedAt ? 1 : -1))
        this.emit("kudzu://clipboard", { type: "reset", entries: structuredClone(this.clips) })
        return r(undefined)
      case "clipboard_pin": {
        this.clips = this.clips.map((c) => (c.id === a.id ? { ...c, pinned: a.pinned as boolean } : c))
        const e = this.clips.find((c) => c.id === a.id)
        if (e) this.emit("kudzu://clipboard", { type: "upsert", entry: structuredClone(e) })
        return r(e ?? null)
      }
      case "clipboard_clear": {
        const before = this.clips.length
        this.clips = a.includePinned ? [] : this.clips.filter((c) => c.pinned)
        this.emit("kudzu://clipboard", { type: "reset", entries: structuredClone(this.clips) })
        return r(before - this.clips.length)
      }
      case "clipboard_to_snippet": {
        const c = this.clips.find((x) => x.id === a.id)
        this.emit("kudzu://new-snippet", { content: c?.text ?? "" })
        return r(undefined)
      }
      case "set_clipboard_hotkey":
        this.state.settings = { ...this.state.settings, clipboardHotkey: a.accelerator as string }
        this.commitSettings()
        return r(this.state.settings)
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
