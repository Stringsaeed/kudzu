#!/usr/bin/env node
// Decides what the beta feed should point at: the newer of what it has now and this release.
// Beta users follow beta.json, so a stable release newer than the latest beta must land there
// too, or they'd be stuck on an old pre-release.
//
//   node feed.mjs --release dist/latest.json [--current beta.json] --out beta.json
// Exits 0 having written --out when it changed, 3 when the current feed is already newer.
import { existsSync, readFileSync, writeFileSync } from "node:fs"
import { parseArgs } from "node:util"

/** SemVer precedence: 1 if a > b, -1 if a < b, 0 if equal. Handles pre-release tags. */
export function compare(a, b) {
  const parse = (v) => {
    const [core, pre] = v.replace(/^v/, "").split("+")[0].split(/-(.*)/s)
    return { nums: core.split(".").map(Number), pre: pre ? pre.split(".") : [] }
  }
  const x = parse(a)
  const y = parse(b)
  for (let i = 0; i < 3; i++) if (x.nums[i] !== y.nums[i]) return x.nums[i] > y.nums[i] ? 1 : -1
  if (!x.pre.length || !y.pre.length) return x.pre.length === y.pre.length ? 0 : x.pre.length ? -1 : 1
  for (let i = 0; i < Math.max(x.pre.length, y.pre.length); i++) {
    const p = x.pre[i]
    const q = y.pre[i]
    if (p === undefined) return -1
    if (q === undefined) return 1
    if (p === q) continue
    const pn = /^\d+$/.test(p)
    const qn = /^\d+$/.test(q)
    if (pn && qn) return Number(p) > Number(q) ? 1 : -1
    if (pn !== qn) return pn ? -1 : 1
    return p > q ? 1 : -1
  }
  return 0
}

if (import.meta.url === `file://${process.argv[1]}`) {
  const { values: a } = parseArgs({ options: { release: { type: "string" }, current: { type: "string" }, out: { type: "string" } } })
  const release = JSON.parse(readFileSync(a.release, "utf8"))
  const current = a.current && existsSync(a.current) ? JSON.parse(readFileSync(a.current, "utf8")) : null
  if (current && compare(current.version, release.version) >= 0) {
    console.log(`feed stays on ${current.version} (≥ ${release.version})`)
    process.exit(3)
  }
  writeFileSync(a.out, JSON.stringify(release, null, 2) + "\n")
  console.log(`feed → ${release.version}`)
}
