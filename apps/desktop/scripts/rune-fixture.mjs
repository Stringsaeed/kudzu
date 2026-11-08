#!/usr/bin/env node
// Exports the browser preview's mock data (lib/mock.ts) as a Kudzu data folder, so the native
// app can render exactly what `pnpm dev` shows and the two can be compared screen for screen:
//
//   node scripts/rune-fixture.mjs <dir> [--clips=5000] [--emptyclips]
//   KUDZU_DATA_DIR=<dir> src-tauri/target/debug/kudzu --window main --snap main.png --size 1180x760
//
// Writes library.json, settings.json and the clipboard history (clipboard/history.json, with
// thumbnails and item blobs for the mock's images). Ids and times are random per run, as in the
// preview. `--clips` and `--emptyclips` are the preview's `?clips=` and `?emptyclips`.
import { execFileSync } from "node:child_process"
import { existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs"
import { register } from "node:module"
import { dirname, join } from "node:path"
import { fileURLToPath, pathToFileURL } from "node:url"

const root = join(dirname(fileURLToPath(import.meta.url)), "..")
const out = process.argv[2]
if (!out) {
  console.error("usage: node scripts/rune-fixture.mjs <dir> [--clips=N] [--emptyclips]")
  process.exit(2)
}
const flags = process.argv.slice(3)
const search = new URLSearchParams()
for (const f of flags) {
  if (f.startsWith("--clips=")) search.set("clips", f.slice(8))
  else if (f === "--emptyclips") search.set("emptyclips", "")
}

// lib/ imports are extensionless TypeScript; resolve them to .ts for Node's type stripping.
register(
  "data:text/javascript," +
    encodeURIComponent(`
export async function resolve(spec, ctx, next) {
  if ((spec.startsWith("./") || spec.startsWith("../")) && !/\\.[mc]?[jt]sx?$/.test(spec)) {
    try { return await next(spec + ".ts", ctx) } catch {}
  }
  return next(spec, ctx)
}`),
  pathToFileURL(root + "/"),
)

// mock.ts only builds its backend in a browser.
globalThis.window = globalThis
globalThis.location = new URL(`http://localhost/${search.size ? `?${search}` : ""}`)
globalThis.addEventListener ??= () => {}
globalThis.removeEventListener ??= () => {}
globalThis.document ??= { addEventListener() {}, removeEventListener() {}, getSelection: () => null }
globalThis.localStorage ??= { getItem: () => null, setItem() {}, removeItem() {} }
const { mockBackend } = await import(pathToFileURL(join(root, "lib/mock.ts")).href)
const state = await mockBackend.invoke("get_state", {})

mkdirSync(out, { recursive: true })
writeFileSync(join(out, "library.json"), JSON.stringify(state.library, null, 2) + "\n")
writeFileSync(join(out, "settings.json"), JSON.stringify(state.settings, null, 2) + "\n")
console.log(`wrote ${out}: ${state.library.snippets.length} snippets, ${state.library.groups.length} groups`)

// MARK: Clipboard history
//
// `ClipStore` reads clipboard/history.json (kudzu_core::clipboard::History), thumbs/<id>.png and
// items/<id>.kzc (encode_items: "KZC1", then item count, and per item its (type, bytes) pairs).
// The mock draws its images as SVG; headless Chrome rasterizes them, at 480 px on the longer side
// for thumbnails (as the watcher stores them) and at full size for the restorable item.
const { entries } = await mockBackend.invoke("clipboard_list", {})
const clip = join(out, "clipboard")
rmSync(clip, { recursive: true, force: true })
for (const sub of ["items", "thumbs", "trash"]) mkdirSync(join(clip, sub), { recursive: true })

function encodeItems(items) {
  const parts = [Buffer.from("KZC1"), u32(items.length)]
  for (const item of items) {
    parts.push(u32(item.length))
    for (const [type, data] of item) {
      const t = Buffer.from(type)
      const d = Buffer.from(data)
      const len = Buffer.alloc(8)
      len.writeBigUInt64LE(BigInt(d.length))
      parts.push(u32(t.length), t, len, d)
    }
  }
  return Buffer.concat(parts)
}
function u32(n) {
  const b = Buffer.alloc(4)
  b.writeUInt32LE(n)
  return b
}

const chrome = "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"
const scratch = join(out, ".render")
/** Rasterizes an SVG data URL at `w`×`h` px into a PNG. */
function rasterize(url, w, h) {
  mkdirSync(scratch, { recursive: true })
  const html = join(scratch, "image.html")
  const png = join(scratch, "image.png")
  writeFileSync(html, `<!doctype html><html><body style="margin:0;overflow:hidden"><img src="${url}" style="display:block;width:${w}px;height:${h}px"></body></html>`)
  rmSync(png, { force: true })
  execFileSync(chrome, ["--headless=new", "--disable-gpu", "--hide-scrollbars", "--force-device-scale-factor=1", `--window-size=${w},${h}`, `--screenshot=${png}`, pathToFileURL(html).href], { stdio: "ignore" })
  return existsSync(png) ? readFileSync(png) : null
}

let images = 0
for (const e of entries) {
  if (e.text !== undefined) writeFileSync(join(clip, "items", `${e.id}.kzc`), encodeItems([[["public.utf8-plain-text", e.text]]]))
  const url = mockBackend.media("thumb", e.id)
  if (!url || !e.image) continue
  const { width, height } = e.image
  const k = 480 / Math.max(width, height)
  const thumb = rasterize(url, Math.round(width * k), Math.round(height * k))
  if (thumb) writeFileSync(join(clip, "thumbs", `${e.id}.png`), thumb)
  const full = rasterize(url, width, height)
  if (full) writeFileSync(join(clip, "items", `${e.id}.kzc`), encodeItems([[["public.png", full]]]))
  images++
}
rmSync(scratch, { recursive: true, force: true })
writeFileSync(join(clip, "history.json"), JSON.stringify({ version: 1, secretRules: 1, entries }) + "\n")
console.log(`wrote ${clip}: ${entries.length} clipboard items, ${images} images`)
