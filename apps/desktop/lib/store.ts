"use client"

// Main-window state. The backend owns the library; this store mirrors it (kept fresh by
// backend events) and holds view state. Mutations are optimistic then confirmed.

import { create } from "zustand"

import { api, Events, on } from "./backend"
import { usageById, type SnippetUsage } from "./stats"
import type { ClipFilter } from "./clipboard"
import type { AppState, EngineStatus, Group, Library, Settings, Snippet, TriggerConflict } from "./types"

export type View =
  | { kind: "all" }
  | { kind: "favorites" }
  | { kind: "recent" }
  | { kind: "disabled" }
  | { kind: "group"; id: string }
  | { kind: "insights" }
  | { kind: "clipboard"; filter?: ClipFilter }
  | { kind: "settings"; section?: SettingsSection }

export type SettingsSection = "general" | "expansion" | "clipboard" | "apps" | "data" | "shortcuts" | "about"

export type Focus = "sidebar" | "list" | "editor"

interface Store {
  ready: boolean
  platform: AppState["platform"]
  library: Library
  settings: Settings
  status: EngineStatus
  usage: Map<string, SnippetUsage>
  conflicts: TriggerConflict[]

  view: View
  selectedId: string | null
  filter: string
  commandOpen: boolean
  shortcutsOpen: boolean
  /** Snippet id whose name field should take focus on next render (new snippets). */
  focusNameOf: string | null

  init: () => () => void
  setView: (view: View) => void
  select: (id: string | null) => void
  setFilter: (q: string) => void
  setCommandOpen: (open: boolean) => void
  setShortcutsOpen: (open: boolean) => void
  consumeFocusName: () => void

  upsertLocal: (snippet: Snippet) => void
  newSnippet: (draft?: Partial<Snippet>) => Promise<Snippet>
  saveSnippet: (snippet: Snippet) => Promise<void>
  deleteSnippets: (ids: string[]) => Promise<Snippet[]>
  restoreSnippets: (snippets: Snippet[]) => Promise<void>
  duplicate: (id: string) => Promise<void>
  toggleFavorite: (id: string) => Promise<void>
  toggleEnabled: (id: string) => Promise<void>
  moveSnippets: (ids: string[], groupId: string | null) => Promise<void>
  createGroup: (name: string, parentId: string | null) => Promise<Group>
  saveGroup: (group: Group) => Promise<void>
  deleteGroup: (id: string, deleteSnippets: boolean) => Promise<void>
  updateSettings: (patch: Partial<Settings>) => Promise<void>
  setPaused: (paused: boolean) => Promise<void>
  refreshConflicts: () => void
}

const emptyLibrary: Library = { formatVersion: 1, groups: [], snippets: [], usage: [] }
const defaultStatus: EngineStatus = {
  canPostEvents: false,
  canExpand: false,
  paused: false,
  tap: "off",
  permissions: { accessibility: false, inputMonitoring: false },
  secureInput: false,
  summary: "Starting…",
}

let conflictTimer: ReturnType<typeof setTimeout> | undefined

export const useStore = create<Store>((set, get) => ({
  ready: false,
  platform: "web",
  library: emptyLibrary,
  settings: {
    typingSpeedWPM: 40,
    defaultTriggerPrefix: ";",
    paletteHotkey: "Control+Alt+Space",
    paused: false,
    defaultInsertion: "automatic",
    deniedBundleIDs: [],
    appInsertions: {},
    launchAtLogin: false,
    onboardingCompleted: true,
    theme: "dark",
    showInDock: true,
    autoCheckUpdates: true,
    icloudSync: true,
    updateChannel: "stable",
    lastRunVersion: null,
    clipboardEnabled: true,
    clipboardRetentionDays: 7,
    clipboardHotkey: "Control+Alt+V",
    clipboardIgnoredApps: [],
    clipboardPastePlain: false,
  },
  status: defaultStatus,
  usage: new Map(),
  conflicts: [],

  view: { kind: "all" },
  selectedId: null,
  filter: "",
  commandOpen: false,
  shortcutsOpen: false,
  focusNameOf: null,

  init: () => {
    api.getState().then((s) => {
      set({
        ready: true,
        platform: s.platform,
        library: s.library,
        settings: s.settings,
        status: s.status,
        usage: usageById(s.library),
        selectedId: get().selectedId ?? s.library.snippets[0]?.id ?? null,
      })
      // Browser preview only: deep-link a view (?view=insights | settings:apps | favorites …).
      if (s.platform === "web") {
        const v = new URLSearchParams(location.search).get("view")
        if (v) {
          const [kind, section] = v.split(":")
          set({ view: (kind === "settings" ? { kind, section } : { kind }) as View })
        }
        const params = new URLSearchParams(location.search)
        if (params.has("onboarding") || params.has("light")) {
          set({ settings: { ...s.settings,
            ...(params.has("onboarding") ? { onboardingCompleted: false } : {}),
            ...(params.has("light") ? { theme: "light" } : {}),
          } })
        }
      }
      get().refreshConflicts()
    })
    const offs = [
      on<Library>(Events.library, (library) => {
        set({ library, usage: usageById(library) })
        get().refreshConflicts()
      }),
      on<Settings>(Events.settings, (settings) => set({ settings })),
      on<EngineStatus>(Events.status, (status) => set({ status })),
      on<{ snippetId?: string | null; view?: View }>(Events.navigate, (p) => {
        if (p.view) set({ view: p.view })
        if (p.snippetId) {
          const s = get().library.snippets.find((x) => x.id === p.snippetId)
          set({ selectedId: p.snippetId, view: s?.groupID ? { kind: "group", id: s.groupID } : { kind: "all" }, filter: "" })
        }
      }),
      on<{ content?: string; name?: string; trigger?: string }>(Events.newSnippet, (p) => {
        const prefix = get().settings.defaultTriggerPrefix
        void get().newSnippet({
          content: p.content ?? "",
          name: p.name ?? "",
          triggers: p.trigger ? [p.trigger.startsWith(prefix) ? p.trigger : prefix + p.trigger] : [],
        })
      }),
    ]
    return () => offs.forEach((f) => f())
  },

  setView: (view) => {
    set({ view })
  },
  select: (id) => set({ selectedId: id }),
  setFilter: (filter) => set({ filter }),
  setCommandOpen: (commandOpen) => set({ commandOpen }),
  setShortcutsOpen: (shortcutsOpen) => set({ shortcutsOpen }),
  consumeFocusName: () => set({ focusNameOf: null }),

  upsertLocal: (snippet) =>
    set((s) => ({
      library: { ...s.library, snippets: s.library.snippets.map((x) => (x.id === snippet.id ? snippet : x)) },
    })),

  newSnippet: async (draft = {}) => {
    const { view } = get()
    const groupID = draft.groupID ?? (view.kind === "group" ? view.id : undefined)
    const snippet = await api.createSnippet({
      isFavorite: view.kind === "favorites",
      ...draft,
      groupID,
    })
    set((s) => ({
      library: s.library.snippets.some((x) => x.id === snippet.id)
        ? s.library
        : { ...s.library, snippets: [snippet, ...s.library.snippets] },
      selectedId: snippet.id,
      focusNameOf: snippet.id,
      filter: "",
      view: view.kind === "insights" || view.kind === "settings" || view.kind === "clipboard" || view.kind === "disabled" || view.kind === "recent" ? { kind: "all" } : view,
    }))
    return snippet
  },

  saveSnippet: async (snippet) => {
    get().upsertLocal(snippet)
    const saved = await api.saveSnippet(snippet)
    get().upsertLocal(saved)
  },

  deleteSnippets: async (ids) => {
    const { library, selectedId } = get()
    const removed = library.snippets.filter((s) => ids.includes(s.id))
    const remaining = library.snippets.filter((s) => !ids.includes(s.id))
    let nextSelected = selectedId
    if (selectedId && ids.includes(selectedId)) {
      const i = library.snippets.findIndex((s) => s.id === selectedId)
      nextSelected = remaining[Math.min(i, remaining.length - 1)]?.id ?? null
    }
    set({ library: { ...library, snippets: remaining }, selectedId: nextSelected })
    await api.deleteSnippets(ids)
    return removed
  },

  restoreSnippets: async (snippets) => {
    await api.restoreSnippets(snippets)
    set({ selectedId: snippets[0]?.id ?? get().selectedId })
  },

  duplicate: async (id) => {
    const copy = await api.duplicateSnippet(id)
    set((s) => ({
      library: s.library.snippets.some((x) => x.id === copy.id) ? s.library : { ...s.library, snippets: [copy, ...s.library.snippets] },
      selectedId: copy.id,
      focusNameOf: copy.id,
    }))
  },

  toggleFavorite: async (id) => {
    const s = get().library.snippets.find((x) => x.id === id)
    if (s) await get().saveSnippet({ ...s, isFavorite: !s.isFavorite })
  },

  toggleEnabled: async (id) => {
    const s = get().library.snippets.find((x) => x.id === id)
    if (s) await get().saveSnippet({ ...s, isEnabled: !s.isEnabled })
  },

  moveSnippets: async (ids, groupId) => {
    set((s) => ({
      library: {
        ...s.library,
        snippets: s.library.snippets.map((x) => (ids.includes(x.id) ? { ...x, groupID: groupId ?? undefined } : x)),
      },
    }))
    await api.moveSnippets(ids, groupId)
  },

  createGroup: async (name, parentId) => {
    const group = await api.createGroup(name, parentId)
    set((s) => ({
      library: s.library.groups.some((g) => g.id === group.id) ? s.library : { ...s.library, groups: [...s.library.groups, group] },
      view: { kind: "group", id: group.id },
    }))
    return group
  },

  saveGroup: async (group) => {
    set((s) => ({ library: { ...s.library, groups: s.library.groups.map((g) => (g.id === group.id ? group : g)) } }))
    await api.saveGroup(group)
  },

  deleteGroup: async (id, deleteSnippets) => {
    const { view } = get()
    if (view.kind === "group" && view.id === id) set({ view: { kind: "all" } })
    await api.deleteGroup(id, deleteSnippets)
  },

  updateSettings: async (patch) => {
    set((s) => ({ settings: { ...s.settings, ...patch } }))
    const settings = await api.updateSettings(patch)
    set({ settings })
  },

  setPaused: async (paused) => {
    set((s) => ({ settings: { ...s.settings, paused }, status: { ...s.status, paused } }))
    const status = await api.setPaused(paused)
    set({ status })
  },

  refreshConflicts: () => {
    clearTimeout(conflictTimer)
    conflictTimer = setTimeout(() => {
      api.findConflicts().then((conflicts) => set({ conflicts }), () => {})
    }, 250)
  },
}))
