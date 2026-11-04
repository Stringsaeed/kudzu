"use client"

// Clipboard history on the client: a store mirrored from the backend (kept fresh by events), plus
// pure helpers for search, grouping by day, titles and formatting. Every window has its own copy.

import { create } from "zustand"

import { api, Events, on } from "./backend"
import type { ClipEntry, ClipEvent, ClipKind } from "./types"

// MARK: Store

interface ClipboardStore {
  ready: boolean
  entries: ClipEntry[]
  diskUsage: number
  canPaste: boolean
  /** Loads the history and subscribes to changes. Returns an unsubscribe function. */
  init: () => () => void
  refresh: () => Promise<void>
  remove: (ids: string[]) => Promise<ClipEntry[]>
  restore: (entries: ClipEntry[]) => Promise<void>
  setPinned: (id: string, pinned: boolean) => Promise<void>
  clear: (includePinned: boolean) => Promise<number>
}

const byNewest = (a: ClipEntry, b: ClipEntry) => (a.copiedAt < b.copiedAt ? 1 : a.copiedAt > b.copiedAt ? -1 : 0)

/** Inserts or replaces `entry`, keeping newest-first order (O(n), n ≤ 5,000). */
function upsert(list: ClipEntry[], entry: ClipEntry): ClipEntry[] {
  const rest = list.filter((e) => e.id !== entry.id)
  let i = 0
  while (i < rest.length && rest[i].copiedAt > entry.copiedAt) i++
  rest.splice(i, 0, entry)
  return rest
}

export const useClipboard = create<ClipboardStore>((set, get) => ({
  ready: false,
  entries: [],
  diskUsage: 0,
  canPaste: true,

  init: () => {
    void get().refresh()
    return on<ClipEvent>(Events.clipboard, (ev) => {
      if (ev.type === "upsert") set((s) => ({ entries: upsert(s.entries, ev.entry) }))
      else if (ev.type === "remove") {
        const ids = new Set(ev.ids)
        set((s) => ({ entries: s.entries.filter((e) => !ids.has(e.id)) }))
      } else set({ entries: [...ev.entries].sort(byNewest) })
    })
  },

  refresh: async () => {
    const info = await api.clipboardList()
    set({ ready: true, entries: info.entries, diskUsage: info.diskUsage, canPaste: info.canPaste })
  },

  remove: async (ids) => {
    const gone = new Set(ids)
    set((s) => ({ entries: s.entries.filter((e) => !gone.has(e.id)) }))
    return api.clipboardDelete(ids)
  },

  restore: async (entries) => {
    set((s) => ({ entries: entries.reduce(upsert, s.entries) }))
    await api.clipboardRestore(entries)
  },

  setPinned: async (id, pinned) => {
    set((s) => ({ entries: s.entries.map((e) => (e.id === id ? { ...e, pinned } : e)) }))
    await api.clipboardPin(id, pinned)
  },

  clear: async (includePinned) => {
    set((s) => ({ entries: includePinned ? [] : s.entries.filter((e) => e.pinned) }))
    const n = await api.clipboardClear(includePinned)
    void get().refresh()
    return n
  },
}))

// MARK: Kinds and filters

export type ClipFilter = "all" | "pinned" | ClipKind

export const KIND_LABEL: Record<ClipKind, string> = {
  text: "Text",
  code: "Code",
  link: "Link",
  color: "Color",
  image: "Image",
  file: "File",
  other: "Other",
}

export const FILTERS: { value: ClipFilter; label: string; short: string }[] = [
  { value: "all", label: "All", short: "All" },
  { value: "pinned", label: "Pinned", short: "Pinned" },
  { value: "text", label: "Text", short: "Text" },
  { value: "image", label: "Images", short: "Images" },
  { value: "link", label: "Links", short: "Links" },
  { value: "file", label: "Files", short: "Files" },
  { value: "code", label: "Code", short: "Code" },
  { value: "color", label: "Colors", short: "Colors" },
]

/** Whether an entry belongs under a filter ("Text" also covers code and other data). */
export function inFilter(e: ClipEntry, f: ClipFilter): boolean {
  if (f === "all") return true
  if (f === "pinned") return e.pinned
  if (f === "text") return e.kind === "text" || e.kind === "other"
  return e.kind === f
}

// MARK: Search

/** Per-entry search text, cached by object identity (entries are replaced, never mutated). */
const haystacks = new WeakMap<ClipEntry, string>()

function haystack(e: ClipEntry): string {
  let h = haystacks.get(e)
  if (h === undefined) {
    h = [e.text ?? "", e.files?.map(basename).join(" ") ?? "", e.color ?? "", KIND_LABEL[e.kind]].join("\n").toLowerCase()
    haystacks.set(e, h)
  }
  return h
}

export interface ParsedQuery {
  terms: string[]
  app: string | null
  kind: ClipKind | null
}

const KIND_ALIASES: Record<string, ClipKind> = {
  text: "text",
  code: "code",
  link: "link",
  links: "link",
  url: "link",
  color: "color",
  colour: "color",
  image: "image",
  images: "image",
  img: "image",
  file: "file",
  files: "file",
  other: "other",
}

/** "invoice app:mail is:link" → terms + an app filter + a kind filter. */
export function parseQuery(q: string): ParsedQuery {
  let app: string | null = null
  let kind: ClipKind | null = null
  const terms: string[] = []
  for (const raw of q.toLowerCase().split(/\s+/)) {
    if (!raw) continue
    if (raw.startsWith("app:")) app = raw.slice(4)
    else if (raw.startsWith("is:") && KIND_ALIASES[raw.slice(3)]) kind = KIND_ALIASES[raw.slice(3)]
    else terms.push(raw)
  }
  return { terms, app, kind }
}

/** Entries matching a filter and query, in history order. ~1 ms for 5,000 entries. */
export function searchClips(entries: ClipEntry[], filter: ClipFilter, query: string): ClipEntry[] {
  const { terms, app, kind } = parseQuery(query)
  return entries.filter((e) => {
    if (!inFilter(e, filter)) return false
    if (kind && e.kind !== kind) return false
    if (app && !(e.source?.name.toLowerCase().includes(app) || e.source?.bundleId.toLowerCase().includes(app))) return false
    if (!terms.length) return true
    const h = haystack(e)
    return terms.every((t) => h.includes(t))
  })
}

/** [start, end) ranges of `terms` in `text`, merged, for highlighting. */
export function matchRanges(text: string, terms: string[]): [number, number][] {
  if (!terms.length || !text) return []
  const lower = text.toLowerCase()
  const ranges: [number, number][] = []
  for (const t of terms) {
    let i = lower.indexOf(t)
    while (i !== -1 && ranges.length < 64) {
      ranges.push([i, i + t.length])
      i = lower.indexOf(t, i + t.length)
    }
  }
  ranges.sort((a, b) => a[0] - b[0])
  const merged: [number, number][] = []
  for (const r of ranges) {
    const last = merged[merged.length - 1]
    if (last && r[0] <= last[1]) last[1] = Math.max(last[1], r[1])
    else merged.push([...r])
  }
  return merged
}

// MARK: Rows and sections

export type ClipRow =
  | { type: "header"; key: string; label: string; count: number }
  /** `dated`: the row isn't under a day heading, so its time needs the day too. */
  | { type: "entry"; key: string; entry: ClipEntry; index: number; dated: boolean }

function dayKey(d: Date): string {
  return `${d.getFullYear()}-${d.getMonth()}-${d.getDate()}`
}

/** Section label for a copy time: Today, Yesterday, a weekday this week, or a date. */
export function dayLabel(ms: number, now = Date.now()): string {
  const d = new Date(ms)
  const today = new Date(now)
  if (dayKey(d) === dayKey(today)) return "Today"
  const y = new Date(now)
  y.setDate(y.getDate() - 1)
  if (dayKey(d) === dayKey(y)) return "Yesterday"
  const days = (new Date(today.getFullYear(), today.getMonth(), today.getDate()).getTime() - ms) / 86_400_000
  if (days < 6) return d.toLocaleDateString(undefined, { weekday: "long" })
  return d.toLocaleDateString(undefined, { weekday: "short", month: "short", day: "numeric", ...(d.getFullYear() !== today.getFullYear() ? { year: "numeric" } : {}) })
}

/**
 * Flattens entries into virtual-list rows: pinned first (when not searching), then one section
 * per day. `index` is the entry's position among selectable rows.
 */
export function buildRows(entries: ClipEntry[], opts: { pinnedFirst: boolean; sections: boolean }): { rows: ClipRow[]; order: ClipEntry[] } {
  const pinned = opts.pinnedFirst ? entries.filter((e) => e.pinned) : []
  const rest = opts.pinnedFirst ? entries.filter((e) => !e.pinned) : entries
  const rows: ClipRow[] = []
  const order: ClipEntry[] = []
  const push = (e: ClipEntry, dated: boolean) => {
    rows.push({ type: "entry", key: e.id, entry: e, index: order.length, dated })
    order.push(e)
  }
  if (pinned.length) {
    if (opts.sections) rows.push({ type: "header", key: "h-pinned", label: "Pinned", count: pinned.length })
    pinned.forEach((e) => push(e, true))
  }
  let current = ""
  let header: { count: number } | null = null
  const now = Date.now()
  for (const e of rest) {
    if (opts.sections) {
      const ms = Date.parse(e.copiedAt)
      const k = dayKey(new Date(ms))
      if (k !== current) {
        current = k
        const h = { type: "header" as const, key: `h-${k}`, label: dayLabel(ms, now), count: 0 }
        rows.push(h)
        header = h
      }
      if (header) header.count++
    }
    push(e, !opts.sections)
  }
  return { rows, order }
}

// MARK: Formatting

export function basename(path: string): string {
  const p = path.replace(/\/+$/, "")
  return p.slice(p.lastIndexOf("/") + 1) || p
}

export function dirname(path: string): string {
  const p = path.replace(/\/+$/, "")
  const i = p.lastIndexOf("/")
  return i <= 0 ? "/" : p.slice(0, i)
}

/** `/Users/me/Documents` → `~/Documents`. */
export function tildify(path: string): string {
  return path.replace(/^\/Users\/[^/]+/, "~")
}

export function formatBytes(n: number): string {
  if (n < 1024) return `${n} B`
  const units = ["KB", "MB", "GB"]
  let v = n / 1024
  let u = 0
  while (v >= 1024 && u < units.length - 1) {
    v /= 1024
    u++
  }
  return `${v.toFixed(v < 10 ? 1 : 0)} ${units[u]}`
}

const timeFmt = typeof Intl !== "undefined" ? new Intl.DateTimeFormat(undefined, { hour: "numeric", minute: "2-digit" }) : null

/**
 * Time of day ("2:04 PM"), or "Just now" / "5m ago" within the last hour. With `dated`, other
 * days get their day too ("Yesterday", "Mon", "Oct 28").
 */
export function clipTime(iso: string, now = Date.now(), dated = false): string {
  const ms = Date.parse(iso)
  const diff = Math.max(0, now - ms) / 1000
  if (diff < 45) return "Just now"
  if (diff < 3600) return `${Math.round(diff / 60)}m ago`
  const time = timeFmt ? timeFmt.format(ms) : new Date(ms).toLocaleTimeString()
  if (!dated) return time
  const day = dayLabel(ms, now)
  if (day === "Today") return time
  if (day === "Yesterday") return "Yesterday"
  return diff < 6 * 86_400 ? new Date(ms).toLocaleDateString(undefined, { weekday: "short" }) : new Date(ms).toLocaleDateString(undefined, { month: "short", day: "numeric" })
}

/** "Today at 2:04:11 PM", "Yesterday at …", or a full date and time. */
export function exactTime(iso: string): string {
  const ms = Date.parse(iso)
  const day = dayLabel(ms)
  const time = new Date(ms).toLocaleTimeString(undefined, { hour: "numeric", minute: "2-digit", second: "2-digit" })
  if (day === "Today" || day === "Yesterday") return `${day} at ${time}`
  return new Date(ms).toLocaleString(undefined, { dateStyle: "medium", timeStyle: "short" })
}

/** "in 6 days", "in 3 hours", "Never". */
export function expiresIn(e: ClipEntry, retentionDays: number, now = Date.now()): string {
  if (e.pinned) return "Never (pinned)"
  if (!retentionDays) return "When history is full"
  const ms = Date.parse(e.copiedAt) + retentionDays * 86_400_000 - now
  if (ms <= 3_600_000) return "Within the hour"
  const hours = ms / 3_600_000
  if (hours < 36) return `In ${Math.round(hours)} hours`
  return `In ${Math.round(hours / 24)} days`
}

export function retentionLabel(days: number): string {
  if (!days) return "until you clear it"
  if (days === 1) return "for 1 day"
  if (days % 365 === 0) return days === 365 ? "for 1 year" : `for ${days / 365} years`
  if (days % 30 === 0 && days >= 30) return days === 30 ? "for 1 month" : `for ${days / 30} months`
  if (days % 7 === 0) return days === 7 ? "for 1 week" : `for ${days / 7} weeks`
  return `for ${days} days`
}

export const RETENTION_OPTIONS: { days: number; label: string }[] = [
  { days: 1, label: "1 day" },
  { days: 7, label: "1 week" },
  { days: 30, label: "1 month" },
  { days: 90, label: "3 months" },
  { days: 365, label: "1 year" },
  { days: 0, label: "Forever" },
]

/** The first meaningful line, whitespace collapsed. */
function firstLine(text: string): string {
  for (const line of text.split("\n")) {
    const t = line.trim()
    if (t) return t.replace(/\s+/g, " ")
  }
  return text.trim()
}

function linkParts(url: string): { host: string; rest: string } {
  try {
    const u = new URL(/^www\./i.test(url) ? `https://${url}` : url)
    if (u.protocol === "mailto:") return { host: u.pathname, rest: "Email address" }
    return { host: u.host.replace(/^www\./, ""), rest: decodeURI(u.pathname + u.search).replace(/\/$/, "") }
  } catch {
    return { host: url, rest: "" }
  }
}

/** Two short lines describing an entry, for list rows and accessible names. */
export function describe(e: ClipEntry): { title: string; subtitle: string } {
  switch (e.kind) {
    case "link": {
      const { host, rest } = linkParts((e.text ?? "").trim())
      return { title: host, subtitle: rest || (e.text ?? "").trim() }
    }
    case "color":
      return { title: (e.color ?? e.text ?? "").toUpperCase(), subtitle: "Color" }
    case "image":
      return { title: e.image ? `Image  ${e.image.width} × ${e.image.height}` : "Image", subtitle: `${imageFormat(e)} · ${formatBytes(e.size)}` }
    case "file": {
      const files = e.files ?? []
      const first = files[0] ?? ""
      return {
        title: files.length > 1 ? `${basename(first)} and ${files.length - 1} more` : basename(first),
        subtitle: tildify(dirname(first)),
      }
    }
    case "other":
      return { title: e.source ? `Content from ${e.source.name}` : "Data", subtitle: `${e.types.length} format${e.types.length === 1 ? "" : "s"} · ${formatBytes(e.size)}` }
    default: {
      const text = e.text ?? ""
      const title = firstLine(text)
      const lines = text.split("\n").map((l) => l.trim()).filter(Boolean)
      const subtitle = lines.length > 1 ? lines[1].replace(/\s+/g, " ") : `${e.textLen.toLocaleString()} character${e.textLen === 1 ? "" : "s"}`
      return { title, subtitle }
    }
  }
}

export function imageFormat(e: ClipEntry): string {
  const t = e.types.find((x) => /png|jpeg|tiff|gif|heic|webp/.test(x)) ?? ""
  return t.includes("png") ? "PNG" : t.includes("jpeg") ? "JPEG" : t.includes("tiff") ? "TIFF" : t.includes("gif") ? "GIF" : t.includes("heic") ? "HEIC" : t.includes("webp") ? "WebP" : "Image"
}

/** Accessible name for a row: "Link, github.com, copied 2:04 PM from Safari, pinned". */
export function spokenLabel(e: ClipEntry): string {
  const { title } = describe(e)
  const parts = [KIND_LABEL[e.kind], title.slice(0, 120), `copied ${exactTime(e.copiedAt)}`]
  if (e.source) parts.push(`from ${e.source.name}`)
  if (e.copyCount > 1) parts.push(`${e.copyCount} times`)
  if (e.pinned) parts.push("pinned")
  return parts.join(", ")
}

export function wordCount(text: string): number {
  const m = text.match(/\S+/g)
  return m ? m.length : 0
}

/** Whether an entry can be turned into a snippet / copied as plain text. */
export const hasText = (e: ClipEntry) => e.kind === "text" || e.kind === "code" || e.kind === "link" || e.kind === "color"

/** Full text of an entry, fetched once per id when the excerpt was cut. */
const fullText = new Map<string, Promise<string>>()
export function loadFullText(e: ClipEntry): Promise<string> {
  if (e.text !== undefined && e.textLen <= (e.text?.length ?? 0)) return Promise.resolve(e.text ?? "")
  let p = fullText.get(e.id + e.hash)
  if (!p) {
    p = api.clipboardText(e.id).then((t) => t ?? e.text ?? "", () => e.text ?? "")
    fullText.set(e.id + e.hash, p)
  }
  return p
}
