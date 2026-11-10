// A small JavaScript renderer used only when the Rust backend isn't available (browser
// preview / design work). The app itself renders through kudzu-core for exact results.

import { literalPositions, sourceSpans } from "./formatting"
import { clampInt, keyInfo, MAX_CLIPBOARD_HISTORY, MAX_DELAY_MS, MAX_KEY_COUNT, MAX_TOTAL_DELAY_MS, splitOptions, tokenize } from "./template"
import type { FormatSpan, Formatting, RenderAction, RenderResult } from "./types"

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
    formatting?: Formatting
    resolveFormatting?: (id: string) => Formatting | undefined
    rich?: boolean
    formValues?: Record<string, string>
    clipboard?: string
    /** Older copies, most recent first (`[0]` is `{{clipboard history=1}}`). */
    clipboardHistory?: string[]
    selection?: string
    resolve?: (id: string) => string | undefined
    depth?: number
    path?: string[]
    /** Pause budget left for nested snippets (ms); the whole expansion shares one. */
    pauseLeft?: { ms: number }
  } = {},
): RenderResult {
  const now = new Date()
  let text = ""
  let cursorOffset: number | null = null
  let cursorSeen = false
  const actions: RenderAction[] = []
  const pauseLeft = opts.pauseLeft ?? { ms: MAX_TOTAL_DELAY_MS }
  const spans = sourceSpans(source, opts.formatting)
  const rich = opts.rich ?? spans !== null
  const nestedSpans: FormatSpan[] = []
  const positions = new Map<number, number>()
  for (const t of tokenize(source)) {
    positions.set(t.from, text.length)
    if (t.kind === "text") {
      if (spans) literalPositions(source.slice(t.from, t.to), t.from, text.length, positions)
      text += t.text
      positions.set(t.to, text.length)
      continue
    }
    if (t.verbatim) {
      text += t.source
      positions.set(t.to, text.length)
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
        const back = clampInt(a.history, 0, MAX_CLIPBOARD_HISTORY, 0)
        const clip = (back === 0 ? opts.clipboard : opts.clipboardHistory?.[back - 1]) ?? ""
        text += a.trim ? clip.trim() : clip
        break
      }
      case "selection": {
        const selection = opts.selection ?? ""
        text += a.trim ? selection.trim() : selection
        break
      }
      case "uuid": {
        const id = typeof crypto !== "undefined" && "randomUUID" in crypto ? crypto.randomUUID() : "00000000-0000-4000-8000-000000000000"
        text += a.upper ? id.toUpperCase() : id
        break
      }
      case "random": {
        const options = splitOptions((a.options as string) ?? "")
        if (options.length) text += options[Math.floor(Math.random() * options.length)]
        break
      }
      case "key":
        // A key press after the cursor cancels it: the caret can only be placed in the last text.
        cursorOffset = null
        actions.push({ offset: text.length, kind: "key", key: keyInfo(a.name)!.name, count: clampInt(a.count, 1, MAX_KEY_COUNT, 1) })
        break
      case "delay": {
        const ms = Math.min(clampInt(a.ms, 1, MAX_DELAY_MS, 1), pauseLeft.ms)
        if (ms > 0) {
          pauseLeft.ms -= ms
          actions.push({ offset: text.length, kind: "pause", ms })
        }
        break
      }
      case "cursor":
        if (!cursorSeen && !opts.depth) {
          cursorSeen = true
          cursorOffset = text.length
        }
        break
      case "snippet": {
        const id = a.id as string
        const depth = opts.depth ?? 0
        const path = opts.path ?? []
        const nested = opts.resolve?.(id)
        if (nested !== undefined && depth < 8 && !path.includes(id)) {
          const result = renderLocal(nested, { ...opts, rich, formatting: rich ? opts.resolveFormatting?.(id) : undefined, depth: depth + 1, path: [...path, id], pauseLeft })
          if (result.actions?.some((x) => x.kind === "key")) cursorOffset = null
          actions.push(...(result.actions ?? []).map((x) => ({ ...x, offset: x.offset + text.length })))
          nestedSpans.push(...(result.formatting ?? []).map((s) => ({ ...s, start: s.start + text.length, end: s.end + text.length })))
          text += result.text
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
    positions.set(t.to, text.length)
  }
  const ownSpans = spans?.flatMap((span) => {
    const start = positions.get(span.start)
    const end = positions.get(span.end)
    return start !== undefined && end !== undefined && start < end ? [{ ...span, start, end }] : []
  })
  const formatting = rich ? [...nestedSpans, ...(ownSpans ?? [])] : undefined
  return { text, cursorOffset, formatting, diagnostics: [], ...(actions.length ? { actions } : {}) }
}
