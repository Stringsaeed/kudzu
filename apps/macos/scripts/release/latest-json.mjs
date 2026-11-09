#!/usr/bin/env node
// Writes the updater manifest (latest.json) that installed copies poll.
// One universal archive serves both Apple Silicon and Intel, so both platform keys point at it.
import { readFileSync, writeFileSync } from "node:fs"
import { parseArgs } from "node:util"

const { values: a } = parseArgs({
  options: {
    version: { type: "string" },
    url: { type: "string" },
    signature: { type: "string" },
    notes: { type: "string", default: "" },
    out: { type: "string" },
  },
})
for (const k of ["version", "url", "signature", "out"]) if (!a[k]) throw new Error(`--${k} is required`)

const signature = readFileSync(a.signature, "utf8").trim()
const notes = a.notes ? readFileSync(a.notes, "utf8").trim() : `Kudzu ${a.version}`
const platform = { signature, url: a.url }
const manifest = {
  version: a.version,
  notes,
  pub_date: new Date().toISOString(),
  platforms: { "darwin-aarch64": platform, "darwin-x86_64": platform },
}
writeFileSync(a.out, JSON.stringify(manifest, null, 2) + "\n")
console.log(`wrote ${a.out}`)
