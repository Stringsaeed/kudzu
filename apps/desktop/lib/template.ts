// Client-side template tokenizer (docs/TEMPLATE_SYNTAX.md) for the chip editor, chip labels
// and quick previews. Rendering for real insertion happens in Rust (kudzu-core); this module
// only needs to find tags, their ranges (UTF-16, which JS strings already are) and attributes.

export type AttrValue = string | number | boolean

export interface TagToken {
  kind: "tag"
  from: number
  to: number
  source: string
  /** Empty for verbatim (unknown or malformed) tags. */
  name: string
  attrs: Record<string, AttrValue>
  /** Unknown or malformed `{{…}}` span, kept as written. */
  verbatim: boolean
}

export interface TextToken {
  kind: "text"
  from: number
  to: number
  text: string
}

export type Token = TagToken | TextToken

export const KNOWN_TAGS = ["date", "time", "clipboard", "cursor", "snippet", "field", "choice", "selection", "uuid", "random", "key", "delay"] as const
export type KnownTag = (typeof KNOWN_TAGS)[number]

/** Limits shared with kudzu-core (`template/mod.rs`). */
export const MAX_KEY_COUNT = 50
export const MAX_DELAY_MS = 5000
export const MAX_TOTAL_DELAY_MS = 10000
export const MAX_CLIPBOARD_HISTORY = 99

/** Keys `{{key}}` can press, in menu order. `cap` is what the keycap shows. */
export const KEYS = [
  { name: "tab", label: "Tab", cap: "⇥" },
  { name: "return", label: "Return", cap: "↵" },
  { name: "escape", label: "Escape", cap: "esc" },
  { name: "space", label: "Space", cap: "Space" },
  { name: "backspace", label: "Backspace", cap: "⌫" },
  { name: "delete", label: "Forward delete", cap: "⌦" },
  { name: "left", label: "Left", cap: "←" },
  { name: "right", label: "Right", cap: "→" },
  { name: "up", label: "Up", cap: "↑" },
  { name: "down", label: "Down", cap: "↓" },
] as const

const KEY_ALIASES: Record<string, string> = { enter: "return", esc: "escape" }

/** The key a `name` attribute means (`enter` and `esc` are accepted spellings), if any. */
export function keyInfo(name: unknown): (typeof KEYS)[number] | undefined {
  if (typeof name !== "string") return undefined
  return KEYS.find((k) => k.name === (KEY_ALIASES[name] ?? name))
}

/** A whole number in `min..=max`, the way kudzu-core reads `count`, `ms` and `history`. */
export function clampInt(value: unknown, min: number, max: number, fallback: number): number {
  if (typeof value !== "number" || !Number.isFinite(value)) return fallback
  return Math.min(max, Math.max(min, Math.floor(value)))
}

const NAME_START = /[a-z]/
const NAME_CHAR = /[a-z0-9_]/
const KEY_CHAR = /[A-Za-z0-9_]/
const BARE_CHAR = /[A-Za-z0-9_.+-]/
const NUMBER = /^[+-]?\d+(\.\d+)?$/
const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i
const STRING_ATTRS = new Set(["default", "name", "format", "options", "offset", "locale", "id"])

/** Finds the `}}` that closes a tag starting at `start` (index of `{{`), honouring quotes. */
function findClose(s: string, start: number): number {
  let i = start + 2
  let quoted = false
  while (i < s.length) {
    const c = s[i]
    if (quoted) {
      if (c === "\\" && i + 1 < s.length) i += 2
      else {
        if (c === '"') quoted = false
        i++
      }
      continue
    }
    if (c === '"') quoted = true
    else if (c === "}" && s[i + 1] === "}") return i + 2
    i++
  }
  return -1
}

/** Parses the inside of a closed tag; returns null when malformed. */
function parseInner(inner: string): { name: string; attrs: Record<string, AttrValue> } | null {
  let i = 0
  const ws = () => {
    while (i < inner.length && /\s/.test(inner[i])) i++
  }
  ws()
  if (!NAME_START.test(inner[i] ?? "")) return null
  let name = ""
  while (i < inner.length && NAME_CHAR.test(inner[i])) name += inner[i++]
  const attrs: Record<string, AttrValue> = {}
  while (true) {
    const before = i
    ws()
    if (i >= inner.length) break
    if (i === before) return null // attributes must be separated by whitespace
    let key = ""
    while (i < inner.length && KEY_CHAR.test(inner[i])) key += inner[i++]
    if (!key) return null
    ws()
    if (inner[i] !== "=") return null
    i++
    ws()
    if (inner[i] === '"') {
      i++
      let value = ""
      let closed = false
      while (i < inner.length) {
        const c = inner[i]
        if (c === "\\" && (inner[i + 1] === '"' || inner[i + 1] === "\\")) {
          value += inner[i + 1]
          i += 2
        } else if (c === '"') {
          closed = true
          i++
          break
        } else {
          value += c
          i++
        }
      }
      if (!closed) return null
      attrs[key] = value
    } else {
      let bare = ""
      while (i < inner.length && BARE_CHAR.test(inner[i])) bare += inner[i++]
      if (!bare) return null
      if (STRING_ATTRS.has(key)) attrs[key] = bare
      else if (bare === "true" || bare === "false") attrs[key] = bare === "true"
      else if (NUMBER.test(bare)) attrs[key] = Number(bare)
      else attrs[key] = bare
    }
  }
  return { name, attrs }
}

function isValidKnown(name: string, attrs: Record<string, AttrValue>): boolean {
  const str = (k: string) => attrs[k] === undefined || typeof attrs[k] === "string"
  const bool = (k: string) => attrs[k] === undefined || typeof attrs[k] === "boolean"
  const num = (k: string) => attrs[k] === undefined || typeof attrs[k] === "number"
  switch (name) {
    case "selection":
      return bool("trim")
    case "uuid":
      return bool("upper")
    case "random":
      return str("options")
    case "key":
      return keyInfo(attrs.name) !== undefined && num("count")
    case "delay":
      return typeof attrs.ms === "number"
    case "date":
    case "time":
      return str("format") && str("locale")
    case "clipboard":
      return bool("trim") && num("history")
    case "cursor":
      return true
    case "snippet":
      return typeof attrs.id === "string" && UUID.test(attrs.id)
    case "field":
      return typeof attrs.name === "string" && str("default") && bool("multiline")
    case "choice":
      return typeof attrs.name === "string" && str("options") && str("default")
    default:
      return false
  }
}

/** Splits template source into text and tag tokens. Never throws. */
export function tokenize(source: string): Token[] {
  const tokens: Token[] = []
  let text = ""
  let textFrom = 0
  const flush = (at: number) => {
    if (text) tokens.push({ kind: "text", from: textFrom, to: at, text })
    text = ""
    textFrom = at
  }
  let i = 0
  while (i < source.length) {
    const c = source[i]
    if (c === "\\" && source[i + 1] === "\\") {
      text += "\\"
      i += 2
      continue
    }
    if (c === "\\" && source[i + 1] === "{" && source[i + 2] === "{") {
      text += "{{"
      i += 3
      continue
    }
    if (c === "{" && source[i + 1] === "{" && source[i + 2] !== "{") {
      const close = findClose(source, i)
      if (close === -1) {
        text += "{{"
        i += 2
        continue
      }
      flush(i)
      const raw = source.slice(i, close)
      const parsed = parseInner(raw.slice(2, -2))
      const ok = parsed !== null && isValidKnown(parsed.name, parsed.attrs)
      tokens.push({
        kind: "tag",
        from: i,
        to: close,
        source: raw,
        name: ok ? parsed!.name : "",
        attrs: ok ? parsed!.attrs : {},
        verbatim: !ok,
      })
      i = close
      textFrom = i
      continue
    }
    text += c
    i++
  }
  flush(source.length)
  return tokens
}

function quote(value: string): string {
  return `"${value.replace(/\\/g, "\\\\").replace(/"/g, '\\"')}"`
}

/** Serializes a tag canonically: strings quoted, booleans/numbers bare, attribute order kept. */
export function buildTag(name: string, attrs: Record<string, AttrValue | undefined | null> = {}): string {
  const parts = Object.entries(attrs)
    .filter(([, v]) => v !== undefined && v !== null && v !== "")
    .map(([k, v]) => `${k}=${typeof v === "string" ? quote(v) : String(v)}`)
  return `{{${[name, ...parts].join(" ")}}}`
}

/** Escapes literal text so it survives a round trip through the parser. */
export function escapeText(text: string): string {
  return text.replace(/\\/g, "\\\\").replace(/\{\{/g, "\\{{")
}

/** Splits `options` on unescaped pipes (`\|` is a literal pipe). */
export function splitOptions(source: string): string[] {
  if (!source) return []
  const out: string[] = []
  let item = ""
  for (let i = 0; i < source.length; i++) {
    if (source[i] === "\\" && source[i + 1] === "|") {
      item += "|"
      i++
    } else if (source[i] === "|") {
      out.push(item)
      item = ""
    } else item += source[i]
  }
  out.push(item)
  return out
}

export function joinOptions(options: string[]): string {
  return options.map((o) => o.replace(/\|/g, "\\|")).join("|")
}

const DATE_PRESETS: Record<string, string> = {
  "yyyy-MM-dd": "ISO date",
  "MMMM d, yyyy": "Long date",
  "MMM d, yyyy": "Medium date",
  "M/d/yy": "Short date",
  "d MMMM yyyy": "Day month year",
  "EEEE": "Weekday",
  "EEEE, MMMM d": "Weekday + date",
  "HH:mm": "24-hour time",
  "h:mm a": "12-hour time",
  "HH:mm:ss": "Time with seconds",
  "yyyy-MM-dd'T'HH:mm:ssXXX": "ISO timestamp",
}

export const DATE_FORMAT_PRESETS = Object.entries(DATE_PRESETS).map(([format, label]) => ({ format, label }))

/** Human label for a chip. `snippetName` resolves nested snippet ids. */
export function chipLabel(tag: TagToken, snippetName?: (id: string) => string | undefined): string {
  if (tag.verbatim) return tag.source
  const a = tag.attrs
  switch (tag.name) {
    case "date":
    case "time": {
      const format = (a.format as string) ?? (tag.name === "date" ? "yyyy-MM-dd" : "HH:mm")
      const base = DATE_PRESETS[format] ?? format
      const off = a.offset ? ` ${a.offset}` : ""
      return `${tag.name === "date" ? "Date" : "Time"} · ${base}${off}`
    }
    case "clipboard": {
      const back = clampInt(a.history, 0, MAX_CLIPBOARD_HISTORY, 0)
      const base = back === 0 ? "Clipboard" : back === 1 ? "Previous copy" : `Clipboard · ${back} back`
      return a.trim ? `${base} (trimmed)` : base
    }
    case "selection":
      return a.trim ? "Selection (trimmed)" : "Selection"
    case "uuid":
      return a.upper ? "UUID (capitals)" : "UUID"
    case "random": {
      const n = splitOptions((a.options as string) ?? "").length
      return `Random · ${n} ${n === 1 ? "option" : "options"}`
    }
    case "key": {
      const key = keyInfo(a.name)!
      const count = clampInt(a.count, 1, MAX_KEY_COUNT, 1)
      // The cap alone for symbols; word caps (esc, Space) already say what they are.
      const cap = key.cap === key.label || key.cap.length > 1 ? key.label : `${key.cap} ${key.label}`
      return count > 1 ? `${cap} ×${count}` : cap
    }
    case "delay":
      return `Pause ${formatDelay(clampInt(a.ms, 1, MAX_DELAY_MS, 1))}`
    case "cursor":
      return "Cursor"
    case "snippet":
      return `↳ ${snippetName?.(a.id as string) ?? "Missing snippet"}`
    case "field":
      return `${a.name as string}${a.multiline ? " ¶" : ""}`
    case "choice":
      return `${a.name as string} ▾`
    default:
      return tag.source
  }
}

/** "300 ms", "1.5 s". */
export function formatDelay(ms: number): string {
  return ms < 1000 ? `${ms} ms` : `${Number((ms / 1000).toFixed(2))} s`
}

export type ChipTone = "time" | "clipboard" | "cursor" | "snippet" | "form" | "error" | "key"

export function chipTone(tag: TagToken): ChipTone {
  if (tag.verbatim) return "error"
  switch (tag.name) {
    case "date":
    case "time":
      return "time"
    case "clipboard":
    case "selection":
      return "clipboard"
    // Generated values sit with dates: text Kudzu makes up at expansion time.
    case "uuid":
    case "random":
      return "time"
    case "key":
    case "delay":
      return "key"
    case "cursor":
      return "cursor"
    case "snippet":
      return "snippet"
    default:
      return "form"
  }
}

export interface TemplateFacts {
  isForm: boolean
  fieldNames: string[]
  usesClipboard: boolean
  hasCursor: boolean
  snippetIds: string[]
  hasErrors: boolean
  /** Has `{{selection}}`, which only has a value when inserted from Quick Search. */
  usesSelection: boolean
  /** Has `{{key}}`: the expansion presses keys, so it can't be undone with Backspace. */
  pressesKeys: boolean
  /** A `{{key}}` comes after `{{cursor}}`, which cancels the cursor position. */
  cursorCancelled: boolean
}

export function analyze(source: string): TemplateFacts {
  const facts: TemplateFacts = {
    isForm: false,
    fieldNames: [],
    usesClipboard: false,
    hasCursor: false,
    snippetIds: [],
    hasErrors: false,
    usesSelection: false,
    pressesKeys: false,
    cursorCancelled: false,
  }
  for (const t of tokenize(source)) {
    if (t.kind !== "tag") continue
    if (t.verbatim) facts.hasErrors = true
    else if (t.name === "field" || t.name === "choice") {
      facts.isForm = true
      const n = t.attrs.name as string
      if (!facts.fieldNames.includes(n)) facts.fieldNames.push(n)
    } else if (t.name === "clipboard") facts.usesClipboard = true
    else if (t.name === "selection") facts.usesSelection = true
    else if (t.name === "key") {
      facts.pressesKeys = true
      if (facts.hasCursor) facts.cursorCancelled = true
    } else if (t.name === "cursor") facts.hasCursor = true
    else if (t.name === "snippet") facts.snippetIds.push(t.attrs.id as string)
  }
  return facts
}

/** Plain-text one-line summary of content for list rows (tags shown as their labels). */
export function plainSummary(source: string, snippetName?: (id: string) => string | undefined, max = 140): string {
  let out = ""
  for (const t of tokenize(source)) {
    out += t.kind === "text" ? t.text : `‹${chipLabel(t, snippetName)}›`
    if (out.length > max) break
  }
  return out.replace(/\s+/g, " ").trim().slice(0, max)
}
