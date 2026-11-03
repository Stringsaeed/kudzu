// A small JavaScript renderer used only when the Rust backend isn't available (browser
// preview / design work). The app itself renders through kudzu-core for exact results.

import { splitOptions, tokenize } from "./template"
import type { RenderResult } from "./types"

const pad = (n: number, w = 2) => String(n).padStart(w, "0")

function applyOffset(date: Date, offset: string | undefined): Date {
  if (!offset) return date
  const m = /^([+-])((?:\d+(?:min|d|w|m|y|h))+)$/.exec(offset)
  if (!m) return date
  const sign = m[1] === "-" ? -1 : 1
  const d = new Date(date)
  for (const [, n, unit] of m[2].matchAll(/(\d+)(min|d|w|m|y|h)/g)) {
    const v = sign * Number(n)
    if (unit === "d") d.setDate(d.getDate() + v)
    else if (unit === "w") d.setDate(d.getDate() + 7 * v)
    else if (unit === "m") {
      const day = d.getDate()
      d.setDate(1)
      d.setMonth(d.getMonth() + v)
      d.setDate(Math.min(day, new Date(d.getFullYear(), d.getMonth() + 1, 0).getDate()))
    } else if (unit === "y") d.setFullYear(d.getFullYear() + v)
    else if (unit === "h") d.setHours(d.getHours() + v)
    else d.setMinutes(d.getMinutes() + v)
  }
  return d
}

/** Formats an ICU date pattern with Intl for names. Covers the common fields. */
export function formatIcu(date: Date, pattern: string, locale = navigator.language): string {
  const name = (opts: Intl.DateTimeFormatOptions) => new Intl.DateTimeFormat(locale, opts).format(date)
  let out = ""
  let i = 0
  while (i < pattern.length) {
    const c = pattern[i]
    if (c === "'") {
      if (pattern[i + 1] === "'") {
        out += "'"
        i += 2
        continue
      }
      const end = pattern.indexOf("'", i + 1)
      out += pattern.slice(i + 1, end === -1 ? undefined : end)
      i = end === -1 ? pattern.length : end + 1
      continue
    }
    if (!/[A-Za-z]/.test(c)) {
      out += c
      i++
      continue
    }
    let n = 1
    while (pattern[i + n] === c) n++
    i += n
    const h = date.getHours()
    switch (c) {
      case "y":
        out += n === 2 ? pad(date.getFullYear() % 100) : pad(date.getFullYear(), n)
        break
      case "M":
      case "L":
        out +=
          n >= 4 ? name({ month: "long" }) : n === 3 ? name({ month: "short" }) : n === 2 ? pad(date.getMonth() + 1) : String(date.getMonth() + 1)
        break
      case "d":
        out += n === 2 ? pad(date.getDate()) : String(date.getDate())
        break
      case "E":
      case "e":
      case "c":
        out += n >= 4 ? name({ weekday: "long" }) : name({ weekday: "short" })
        break
      case "a":
        out += h < 12 ? "AM" : "PM"
        break
      case "h":
        out += n === 2 ? pad(h % 12 || 12) : String(h % 12 || 12)
        break
      case "H":
        out += n === 2 ? pad(h) : String(h)
        break
      case "m":
        out += n === 2 ? pad(date.getMinutes()) : String(date.getMinutes())
        break
      case "s":
        out += n === 2 ? pad(date.getSeconds()) : String(date.getSeconds())
        break
      case "X":
      case "x":
      case "Z": {
        const o = -date.getTimezoneOffset()
        out += `${o >= 0 ? "+" : "-"}${pad(Math.floor(Math.abs(o) / 60))}:${pad(Math.abs(o) % 60)}`
        break
      }
      default:
        out += c.repeat(n)
    }
  }
  return out
}

export function renderLocal(
  source: string,
  opts: {
    formValues?: Record<string, string>
    clipboard?: string
    resolve?: (id: string) => string | undefined
    depth?: number
    path?: string[]
  } = {},
): RenderResult {
  const now = new Date()
  let text = ""
  let cursorOffset: number | null = null
  for (const t of tokenize(source)) {
    if (t.kind === "text") {
      text += t.text
      continue
    }
    if (t.verbatim) {
      text += t.source
      continue
    }
    const a = t.attrs
    switch (t.name) {
      case "date":
      case "time":
        text += formatIcu(
          applyOffset(now, a.offset as string | undefined),
          (a.format as string) ?? (t.name === "date" ? "yyyy-MM-dd" : "HH:mm"),
          (a.locale as string | undefined)?.replace("_", "-"),
        )
        break
      case "clipboard": {
        const clip = opts.clipboard ?? ""
        text += a.trim ? clip.trim() : clip
        break
      }
      case "cursor":
        if (cursorOffset === null && !opts.depth) cursorOffset = text.length
        break
      case "snippet": {
        const id = a.id as string
        const depth = opts.depth ?? 0
        const path = opts.path ?? []
        const nested = opts.resolve?.(id)
        if (nested !== undefined && depth < 8 && !path.includes(id)) {
          text += renderLocal(nested, { ...opts, depth: depth + 1, path: [...path, id] }).text
        }
        break
      }
      case "field":
        text += opts.formValues?.[a.name as string] ?? (a.default as string) ?? ""
        break
      case "choice":
        text +=
          opts.formValues?.[a.name as string] ?? (a.default as string) ?? splitOptions((a.options as string) ?? "")[0] ?? ""
        break
    }
  }
  return { text, cursorOffset, diagnostics: [] }
}
