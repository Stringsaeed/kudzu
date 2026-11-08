// Generates `parity.json`: outputs of the web UI's TypeScript (`apps/desktop/lib`) for the
// Rust port in `kudzu_core::client` to reproduce (see `tests/client_parity.rs`).
//
//   cd apps/desktop && TZ=America/New_York LANG=en_US.UTF-8 \
//     node src-tauri/crates/kudzu-core/tests/fixtures/client/generate.mjs
//
// Runs the real modules through the TypeScript loader the formatting tests use. The palette
// and list orderings replicate the component code (palette.tsx, snippet-list.tsx) verbatim.

import { writeFileSync } from "node:fs"
import * as module from "node:module"

const lib = new URL("../../../../../../lib/", import.meta.url)
const loader = new URL("../../../../../../scripts/tests/typescript-loader.mjs", import.meta.url)
Object.defineProperty(globalThis, "navigator", { value: { platform: "MacIntel", userAgent: "Mac" }, configurable: true })
if (module.registerHooks) module.registerHooks(await import(loader))
else module.register(loader)

const NOW = Date.UTC(2026, 10, 8, 17, 30, 0) // Sun Nov 8 2026, 12:30 PM in New York
Date.now = () => NOW

const search = await import(new URL("search.ts", lib))
const groups = await import(new URL("groups.ts", lib))
const stats = await import(new URL("stats.ts", lib))
const template = await import(new URL("template.ts", lib))
const formatting = await import(new URL("formatting.ts", lib))
const keys = await import(new URL("keys.ts", lib))
const clipboard = await import(new URL("clipboard.ts", lib))
const updates = await import(new URL("updates.ts", lib))

// Deterministic data.
let seed = 42
const rand = () => ((seed = (seed * 1103515245 + 12345) % 2147483648) / 2147483648)
const pick = (a) => a[Math.floor(rand() * a.length)]
const uuid = (n) => `00000000-0000-4000-8000-${n.toString(16).padStart(12, "0")}`
const iso = (ms) => new Date(Math.floor(ms / 1000) * 1000).toISOString().replace(".000Z", "Z")

const WORDS = ["email", "Email", "sig", "signature", "address", "Adresse", "café", "résumé", "naïve", "meeting", "notes", "invoice", "thanks", "Thank you", "hello", "hi", "bye", "regards", "zoom", "link", "phone", "date", "ok", "OK", "Ünïcödé", "😀 smile", "x2", "v1.2", "path/to", "snake_case", "camelCase", "a-b", "", "Straße", "ÆON"]
const TRIGGERS = [";sig", ";email", ";addr", ";hi", ";thx", ";ty", "sig", ";date", ";zoom", ";ph", ";inv", ";Sig", "!!", ";a", ";ab", ";abc", ";résumé"]
const BODIES = [
  "Best regards,\nSam",
  "Hello {{field name=who}}, thanks for {{choice name=what options=\"the call|your email\"}}!",
  "{{date format=\"EEEE\"}} {{time}} {{cursor}}",
  "Call me at 555-0100 or email sam@example.com",
  "{{clipboard trim=true}}",
  "Meeting notes\n- agenda\n- actions",
  "😀😀 emoji body with snake_case and camelCase2",
  "Invoice #2026-11 total $1,234.00",
  "{{snippet id=\"" + uuid(1) + "\"}} and {{snippet id=\"" + uuid(999) + "\"}}",
  "Grüße aus München — naïve café",
  "",
  "{{broken tag",
  "path/to/file.txt v1.2.3",
]

const groupList = [
  { id: uuid(9001), name: "Work", sortIndex: 1, isEnabled: true },
  { id: uuid(9002), name: "Email", parentID: uuid(9001), sortIndex: 0, isEnabled: true },
  { id: uuid(9003), name: "work-extra", sortIndex: 0, isEnabled: false },
  { id: uuid(9004), name: "Personal", sortIndex: 1, isEnabled: true },
  { id: uuid(9005), name: "Deep", parentID: uuid(9002), sortIndex: 0, isEnabled: true },
  { id: uuid(9006), name: "Orphan", parentID: uuid(77), sortIndex: 2, isEnabled: true },
  { id: uuid(9007), name: "Self", parentID: uuid(9007), sortIndex: 0, isEnabled: true },
  { id: uuid(9008), name: "Loop A", parentID: uuid(9009), sortIndex: 0, isEnabled: true },
  { id: uuid(9009), name: "Loop B", parentID: uuid(9008), sortIndex: 0, isEnabled: true },
  { id: uuid(9010), name: "émoji", sortIndex: 1, isEnabled: true },
  { id: uuid(9011), name: "apple", sortIndex: 1, isEnabled: true },
]

const snippets = []
for (let i = 1; i <= 100; i++) {
  const nTriggers = Math.floor(rand() * 3)
  const triggers = Array.from({ length: nTriggers }, () => pick(TRIGGERS) + (rand() < 0.3 ? String(i % 7) : ""))
  const name = rand() < 0.7 ? `${pick(WORDS)}${rand() < 0.4 ? " " + pick(WORDS) : ""}`.trim() : ""
  const updated = NOW - Math.floor(rand() * 40) * 86_400_000 - Math.floor(rand() * 3) * 1000
  snippets.push({
    id: uuid(i),
    name,
    triggers,
    content: pick(BODIES),
    ...(rand() < 0.6 ? { groupID: pick(groupList).id } : rand() < 0.1 ? { groupID: uuid(5555) } : {}),
    fireMode: "immediate",
    caseMode: "propagate",
    isFavorite: rand() < 0.2,
    isEnabled: rand() < 0.85,
    createdAt: iso(updated - 86_400_000),
    updatedAt: iso(updated),
  })
}
// One body past the 2,000-unit fold limit: only its first 2,000 units are searchable.
snippets.push({ ...snippets[0], id: uuid(101), name: "long body", triggers: [";long"], content: "a".repeat(1990) + " tail-term", groupID: undefined })
const usage = []
for (const s of snippets) {
  if (rand() < 0.5) continue
  for (const device of ["mac-a", "mac-b"]) {
    if (rand() < 0.4) continue
    const ago = pick([30_000, 3_600_000, 2 * 86_400_000, 10 * 86_400_000, 60 * 86_400_000])
    usage.push({ snippetID: s.id, deviceID: device, count: Math.floor(rand() * 50), charsSaved: Math.floor(rand() * 2000) - 100, ...(rand() < 0.9 ? { lastUsedAt: iso(NOW - ago) } : {}) })
  }
}
const library = { formatVersion: 1, groups: groupList, snippets, usage }

const QUERIES = ["", "sig", ";sig", "SIG", "email", "em", "e", "thx", "thank", "regards sam", "inv", "café", "cafe", "résumé", "naive", "😀", "smile", "x2", "v1", "path", "to", "snake", "case", "camel", "a-b", "ab", ";a", ";ab", "zz", "ŭ", "straße", "strasse", "Æon", "tail-term", "tail", "aaaa", "555", "date", "who", "Missing", "missing snippet", "  hello   world  ", "h", "hi", "ok", "bye zoom", "meeting notes agenda", "ünïcödé", "x", "1", "#", "e m a i l", "s".repeat(70), "tx", ";tx", "sg", ";sg", "ty1", ";ay", "rs", "zm", "dt", "ad", "inv3"]

const usageMap = stats.usageById(library)
const hit = (h) => [h.snippet.id, h.score, h.field, h.nameHits]
const index = search.buildIndex(library.snippets, usageMap)
const searchCases = QUERIES.map((q) => ({ query: q, limit: 200, hits: search.search(index, q, 200).map(hit) }))

// palette.tsx
const SCOPES = ["all", "favorites", "recent"]
function paletteHits(query, scope) {
  const m = /(?:^|\s)#(\S*)/.exec(query)
  const text = m ? query.replace(m[0], " ").trim() : query
  const groupFilter = m ? m[1].toLowerCase() : null
  let s = library.snippets
  if (scope === "favorites") s = s.filter((x) => x.isFavorite)
  if (scope === "recent") s = s.filter((x) => usageMap.get(x.id)?.last)
  if (groupFilter !== null) {
    const ids = new Set(library.groups.filter((g) => g.name.toLowerCase().startsWith(groupFilter)).map((g) => g.id))
    s = s.filter((x) => x.groupID && ids.has(x.groupID))
  }
  const r = search.search(search.buildIndex(s, usageMap), text, 60)
  if (scope === "recent" && !text) r.sort((a, b) => (usageMap.get(b.snippet.id)?.last ?? 0) - (usageMap.get(a.snippet.id)?.last ?? 0))
  else if (!text) r.sort((a, b) => Number(b.snippet.isFavorite) - Number(a.snippet.isFavorite) || b.score - a.score)
  const createRow = !!query.trim() && !r.some((h) => h.snippet.triggers.some((t) => t.toLowerCase() === query.trim().toLowerCase()))
  return { text, groupFilter, ids: r.map((h) => h.snippet.id), createRow }
}
const PALETTE_QUERIES = ["", "#work", "#WORK sig", "sig #e", "#", "# hello", "a#b", "  #em  thanks", "hello #per x", "#zzz", ";sig", "#émoji"]
const paletteCases = PALETTE_QUERIES.flatMap((query) => SCOPES.map((scope) => ({ query, scope, ...paletteHits(query, scope) })))

// snippet-list.tsx
const VIEWS = [{ kind: "all" }, { kind: "favorites" }, { kind: "recent" }, { kind: "disabled" }, { kind: "group", id: uuid(9001) }, { kind: "group", id: uuid(9008) }]
const SORTS = ["relevance", "name", "edited", "used"]
function listHits(view, filter, sort) {
  const s = library.snippets
  let scoped
  switch (view.kind) {
    case "favorites": scoped = s.filter((x) => x.isFavorite); break
    case "recent": scoped = s.filter((x) => usageMap.get(x.id)?.last); break
    case "disabled": scoped = s.filter((x) => !x.isEnabled); break
    case "group": { const ids = groups.descendantIds(library.groups, view.id); scoped = s.filter((x) => x.groupID && ids.has(x.groupID)); break }
    default: scoped = s
  }
  const r = search.search(search.buildIndex(scoped, usageMap), filter, 10_000)
  const q = filter.trim()
  const by = (cmp) => r.sort((a, b) => cmp(a.snippet, b.snippet))
  if (view.kind === "recent" && (!q || sort === "relevance")) by((a, b) => (usageMap.get(b.id)?.last ?? 0) - (usageMap.get(a.id)?.last ?? 0))
  else if (sort === "name" || (!q && sort === "relevance")) by((a, b) => (a.name || "￿").localeCompare(b.name || "￿", undefined, { sensitivity: "base" }))
  else if (sort === "edited") by((a, b) => b.updatedAt.localeCompare(a.updatedAt))
  else if (sort === "used") by((a, b) => (usageMap.get(b.id)?.count ?? 0) - (usageMap.get(a.id)?.count ?? 0))
  return r.map((h) => h.snippet.id)
}
const listCases = VIEWS.flatMap((view) => ["", "e", " sig"].flatMap((filter) => SORTS.map((sort) => ({ view, filter, sort, ids: listHits(view, filter, sort) }))))

const tree = groups.flatten(groups.buildTree(library.groups)).map((n) => [n.group.id, n.depth])
const groupCases = {
  tree,
  collapsed: groups.flatten(groups.buildTree(library.groups), new Set([uuid(9001)])).map((n) => n.group.id),
  counts: Object.fromEntries(groups.recursiveCounts(library.groups, library.snippets)),
  paths: library.groups.map((g) => [g.id, groups.groupPath(library.groups, g.id)]),
  enabled: library.groups.map((g) => [g.id, groups.isGroupEnabled(library.groups, g.id)]),
  descendants: library.groups.map((g) => [g.id, [...groups.descendantIds(library.groups, g.id)].sort()]),
}

const totals = stats.totals(library, usageMap, 45)
const statsCases = {
  usage: Object.fromEntries([...usageMap].map(([k, v]) => [k, v])),
  totals,
  durations: [0, 0.004, 0.0083, 0.5, 0.99, 1, 1.49, 1.5, 2.5, 59.4, 59.6, 60, 89, 150, 599, 600, 601, 1439, 1440, 5000, 123456.7].map((m) => [m, stats.formatDuration(m)]),
  counts: [0, 1, 999, 1000, 1049, 1050, 1250, 1350, 9949, 9950, 9999, 10_000, 10_499, 10_500, 999_499, 999_500, 1_000_000, 1_049_999, 1_050_000, 123_456_789].map((n) => [n, stats.formatCount(n)]),
  relative: [0, -1, 30, 59, 60, 89, 90, 91, 150, 3570, 3600, 5399, 5400, 86_399, 86_400, 129_599, 129_600, 2_591_999, 2_592_000, 40_000_000].flatMap((s) => [s, -s]).map((s) => [s, stats.relativeTime(s === 0 ? 0 : NOW - s * 1000)]),
}

const SOURCES = [
  "", "plain", "😀 {{date}} x", "{{date format=\"EEEE\" offset=+1d locale=fr_FR}}", "{{time format=\"h:mm a\"}}", "{{date format=yyyy}}",
  "{{clipboard trim=true}}{{clipboard trim=false}}{{clipboard}}", "{{cursor}}", "{{field name=who default=\"a \\\"q\\\" b\" multiline=true}}",
  "{{choice name=size options=\"S|M\\\\|L|XL\" default=M}}", "{{snippet id=" + uuid(1) + "}}", "{{snippet id=\"" + uuid(2).toUpperCase() + "\"}}",
  "{{snippet id=nope}}", "{{unknown}}", "{{date format=1}}", "{{field name=1.5}}", "{{field x=1 x=2 2=b 1=a}}", "\\{{literal}} \\\\ {{{cursor}}",
  "{{ cursor }}", "{{cursor}", "{{date format=\"}}\"}} tail", "{{date format=\"unterminated}}", "a {{ b", "{{field name=\"ü\" default=}}",
  "{{date offset=-3m}}", "{{clipboard trim=1}}", "{{field name=a multiline=yes}}", "x\n\n  y\t{{cursor}}  ", "{{date __proto__=1}}",
  "{{time format=\"HH:mm\" offset=+30min}}{{date format=\"MMMM d, yyyy\"}}", "a".repeat(150) + "{{cursor}}", "😀".repeat(80),
]
const names = new Map([[uuid(1), "First snippet"]])
const token = (t) => (t.kind === "text" ? ["text", t.from, t.to, t.text] : ["tag", t.from, t.to, t.source, t.name, Object.entries(t.attrs), t.verbatim])
const templateCases = SOURCES.map((source) => {
  const tokens = template.tokenize(source)
  return {
    source,
    tokens: tokens.map(token),
    labels: tokens.filter((t) => t.kind === "tag").map((t) => [template.chipLabel(t, (id) => names.get(id)), template.chipTone(t)]),
    analyze: template.analyze(source),
    summary: template.plainSummary(source, (id) => names.get(id)),
    summary10: template.plainSummary(source, undefined, 10),
  }
})
const buildTagCases = [
  ["field", { name: "who", default: "", multiline: true }],
  ["choice", { name: "a \"b\" \\c", options: "x|y", default: undefined }],
  ["date", { format: "EEEE", offset: null, n: 1.5, z: 0, big: 1e21, small: 1e-7, neg: -0 }],
  ["x", { b: 1, 2: "two", a: false, 1: "one" }],
].map(([name, attrs]) => [name, Object.entries(attrs), template.buildTag(name, attrs)])

const PNG = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAABgAAAAYCAYAAADgdz34AAAAJUlEQVR4nGMICLj0n5aYYdSCUQtGLRi1YNSCUQtGLRi1YGhYAACpwH6M7RqhEQAAAABJRU5ErkJggg=="
const formattingCases = [
  ["Hello world", [{ start: 0, end: 5, style: { kind: "bold" } }, { start: 3, end: 8, style: { kind: "italic" } }, { start: 6, end: 11, style: { kind: "link", url: "https://x.y/?a=1&b=\"2\"" } }], 4],
  ["a😀b<>&'\"\nc", [{ start: 0, end: 2, style: { kind: "bold" } }, { start: 1, end: 3, style: { kind: "underline" } }, { start: 3, end: 9, style: { kind: "link", url: "javascript:x" } }, { start: 3, end: 10, style: { kind: "italic" } }], 2],
  ["Hello\n[Logo] world", [{ start: 0, end: 5, style: { kind: "bold" } }, { start: 6, end: 12, style: { kind: "image", dataUrl: PNG, width: 120 } }, { start: 8, end: 15, style: { kind: "italic" } }], null],
  ["", [], 0],
  ["xyz", [{ start: 0, end: 3, style: { kind: "image", dataUrl: PNG, width: 0 } }, { start: 2, end: 1, style: { kind: "bold" } }, { start: 0, end: 9, style: { kind: "bold" } }], 9],
].map(([text, spans, cursor]) => ({
  text, spans, cursor,
  valid: formatting.validSpans(text, spans),
  parts: formatting.formattedParts(text, spans, cursor).map((p) => [p.start, p.text, p.image ?? null, p.styles]),
  html: formatting.formattedHtml(text, spans),
}))
const links = ["https://a.b", "HTTP://A", "mailto:x", "ftp://x", "javascript:x", "https://a b", "https://a b", "https://a﻿b", "https://a\u0085b", "https://a\u0000b", "https://a\u007fb", "https://a'b", "https://a<b"].map((u) => [u, formatting.safeLink(u)])

const keyCases = {
  labels: ["mod+shift+k", "alt+up", "ctrl+enter", "escape", "delete", "backspace", "tab", "space", "mod+ß", "F5", "shift+?", "Cmd+Option+x"].map((c) => [c, keys.keyLabels(c)]),
  accelerators: ["Control+Alt+Space", "CommandOrControl+Shift+V", "Super+Semicolon", "Command+F12", "cmd+a", "Meta+Option+Tab", "Control+ß"].map((a) => [a, keys.acceleratorLabels(a)]),
}

// Clipboard entries.
const entry = (n, kind, extra) => ({
  id: uuid(n), kind, textLen: extra.text ? extra.text.length : 0, lineCount: 1, rich: false, types: [], size: 0, hash: `h${n}`,
  firstCopiedAt: extra.copiedAt, copyCount: 1, timings: [], useCount: 0, pinned: false, ...extra,
})
const H = 3_600_000
const clips = [
  entry(1, "text", { text: "Quarterly Invoice\n\n  second   line\nthird", copiedAt: iso(NOW - 20_000), source: { bundleId: "com.tinyspeck.slackmacgap", name: "Slack" }, pinned: true, copyCount: 3 }),
  entry(2, "link", { text: "https://www.github.com/kudzu/app/", copiedAt: iso(NOW - 10 * 60_000), source: { bundleId: "com.apple.Safari", name: "Safari" } }),
  entry(3, "link", { text: "www.example.com/a%20b?q=%2F&x=%E2%82%AC#frag", copiedAt: iso(NOW - 2 * H) }),
  entry(4, "link", { text: "mailto:Me@Example.com", copiedAt: iso(NOW - 13 * H) }),
  entry(5, "link", { text: "https://bücher.de:443/ü/./x/../y?q=a b", copiedAt: iso(NOW - 20 * H) }),
  entry(6, "link", { text: "https://x.com/%E0%A4%A", copiedAt: iso(NOW - 30 * H) }),
  entry(7, "color", { text: "#ff00AA", color: "#ff00aa", copiedAt: iso(NOW - 2 * 24 * H) }),
  entry(8, "image", { image: { width: 640, height: 480 }, types: ["public.tiff", "public.png"], size: 123456, copiedAt: iso(NOW - 3 * 24 * H), source: { bundleId: "com.apple.Preview", name: "Preview" } }),
  entry(9, "file", { files: ["/Users/sam/Documents/Report.PDF", "/Users/sam/b.txt", "/tmp/c/"], copiedAt: iso(NOW - 5 * 24 * H) }),
  entry(10, "other", { types: ["com.figma.data"], size: 5 * 1024 * 1024 + 1, copiedAt: iso(NOW - 6 * 24 * H), source: { bundleId: "com.figma.Desktop", name: "Figma" } }),
  entry(11, "code", { text: "const x = 1;\nreturn x", copiedAt: iso(NOW - 7 * 24 * H) }),
  entry(12, "text", { text: "😀 emoji text", textLen: 4096, copiedAt: iso(NOW - 40 * 24 * H), secret: "apiKey" }),
  entry(13, "text", { text: "hunter2", copiedAt: iso(NOW - 400 * 24 * H), secret: "password", secretOverride: false }),
  entry(14, "other", { types: ["a", "b"], size: 900, copiedAt: iso(NOW - 401 * 24 * H) }),
  entry(15, "file", { files: [], copiedAt: iso(NOW - 402 * 24 * H) }),
  entry(16, "link", { text: "http://user:pw@0x7f.1:8080/a/%7e?x", copiedAt: iso(NOW - 403 * 24 * H) }),
]
const CLIP_QUERIES = ["", "invoice", "app:slack", "APP:Slack invoice", "app:safari github", "is:link", "is:links", "is:url example", "is:image", "is:img", "is:bogus", "app:", "app: is:file", "report", "documents", "#ff00", "color", "image", "file", "code", "other", "secon", "😀", "is:text", "is:colour", "is:other data", "x"]
const FILTER_VALUES = clipboard.FILTERS.map((f) => f.value)
const clipCases = {
  entries: clips,
  parse: CLIP_QUERIES.map((q) => [q, clipboard.parseQuery(q)]),
  search: CLIP_QUERIES.flatMap((q) => FILTER_VALUES.map((f) => [q, f, clipboard.searchClips(clips, f, q).map((e) => e.id)])),
  describe: clips.map((e) => [e.id, clipboard.describe(e), clipboard.spokenLabel(e), clipboard.spokenLabel(e, true), clipboard.clipTime(e.copiedAt, NOW), clipboard.clipTime(e.copiedAt, NOW, true), clipboard.exactTime(e.copiedAt), clipboard.expiresIn(e, 7, NOW), clipboard.expiresIn(e, 1, NOW), clipboard.secretReason(e), clipboard.isSecret(e), clipboard.hasText(e), clipboard.imageFormat(e)]),
  rows: [[true, true], [false, true], [true, false], [false, false]].map(([pinnedFirst, sections]) => [pinnedFirst, sections, clipboard.buildRows(clips, { pinnedFirst, sections }).rows.map((r) => (r.type === "header" ? ["header", r.key, r.label, r.count] : ["entry", r.key, r.index, r.dated]))]),
  ranges: [["Hello hello HELLO", ["ell"]], ["abcdef", ["bc", "cde", "f"]], ["😀ab😀AB", ["ab"]], ["aaaa", ["aa"]], ["xyz", []], ["İx", ["x"]]].map(([text, terms]) => [text, terms, clipboard.matchRanges(text, terms)]),
  bytes: [0, 1, 1023, 1024, 1536, 10239, 10240, 10752, 1048575, 1048576, 1073741824, 5e12].map((n) => [n, clipboard.formatBytes(n)]),
  retention: [0, 1, 2, 7, 14, 21, 28, 30, 31, 60, 90, 180, 210, 365, 730, 1095, 420].map((d) => [d, clipboard.retentionLabel(d)]),
  words: ["", "  one two\n three four ", "﻿a\u0085b"].map((t) => [t, clipboard.wordCount(t)]),
  days: [0, 1, 11, 12, 13, 24, 35, 36, 37, 4 * 24, 5 * 24, 5 * 24 + 12, 5 * 24 + 13, 6 * 24, 7 * 24, 300 * 24, 320 * 24, -24, -200].map((h) => [h, clipboard.dayLabel(NOW - h * H, NOW)]),
}

const notes = [
  "## [0.2.0](https://github.com/x/y/compare/v0.1.0...v0.2.0) (2026-01-01)\n\n\n### Features\n\n* **desktop:** import from Raycast ([b8b1f36](https://github.com/x/y/commit/b8b1f36d)) ([#12](https://github.com/x/y/issues/12))\n* plain item\n\n### Bug Fixes\n\n- fix **a** and **b** ([abcdef0](u))\n",
  "intro\n# H\n####### seven\n#nospace\n* a ([abc12](u)) [](v) **** [x]](y) **a*b**\n-   spaced\n*tight\n### [1.2.3 notes\n### 1.2",
  "",
]
const notesCases = notes.map((md) => [md, updates.parseNotes(md)])

const fixed = [0, 0.05, 0.15, 0.25, 0.35, 0.45, 1.005, 2.5, -2.5, 9.95, 9.96, 99.95, 999.5, 1234.5678, 1e20, 1e21, -0.0001].map((x) => [x, x.toFixed(0), x.toFixed(1), x.toFixed(2)])

// A cut that halves a surrogate pair leaves a lone surrogate in JS; Rust strings hold U+FFFD there.
const wellFormed = (_, v) => (typeof v === "string" ? v.toWellFormed() : v)
writeFileSync(new URL("parity.json", import.meta.url), JSON.stringify({ now: NOW, tz: "America/New_York", library, searchCases, paletteCases, listCases, groupCases, statsCases, templateCases, buildTagCases, formattingCases, links, keyCases, clipCases, notesCases, fixed }, wellFormed) + "\n")
console.log("wrote parity.json")
