// Fuzzy, usage-boosted snippet search shared by the palette and the library list.
//
// Each whitespace-separated term must match the name, a trigger, or the content as a
// subsequence. Matches score per character with bonuses for word starts and consecutive
// runs (the fzf model, greedy variant), weighted name > trigger > content. A query equal
// to a trigger always wins. Usage nudges ties without burying a clearly better match.
// The index pre-folds every field once, so a keystroke is a tight loop over strings.

import type { Snippet } from "./types"
import { plainSummary } from "./template"

export interface SearchEntry {
  snippet: Snippet
  name: string
  nameLower: string
  triggersLower: string[]
  contentLower: string
  usage: number
  lastUsed: number
}

export interface SearchHit {
  snippet: Snippet
  score: number
  field: "name" | "trigger" | "content" | null
  /** Indices into `snippet.name` that matched. */
  nameHits: number[]
}

const BOUNDARY = /[\s\-_./:;,()[\]{}'"!?]/

function isBoundary(text: string, i: number): boolean {
  if (i === 0) return true
  const prev = text[i - 1]
  if (BOUNDARY.test(prev)) return true
  const c = text[i]
  // camelHump: lower → upper in the original casing is approximated by digit/letter switch here.
  return /[a-z]/.test(prev) && /[0-9]/.test(c)
}

/** Greedy subsequence match with a backwards tightening pass. Returns score and positions. */
function fuzzy(term: string, text: string, wantPositions: boolean): { score: number; positions: number[] } | null {
  const n = text.length
  const m = term.length
  if (m === 0) return { score: 0, positions: [] }
  if (m > n) return null
  // Fast path: contiguous substring.
  const idx = text.indexOf(term)
  if (idx !== -1) {
    let score = m * 16 + 8 * (m - 1)
    if (isBoundary(text, idx)) score += 20
    if (idx === 0) score += 20
    if (m === n) score += 20
    score -= Math.min(idx, 20) * 0.5
    return { score, positions: wantPositions ? Array.from({ length: m }, (_, k) => idx + k) : [] }
  }
  // Forward scan to find the end of the earliest match.
  let ti = 0
  let end = -1
  for (let i = 0; i < n; i++) {
    if (text[i] === term[ti]) {
      ti++
      if (ti === m) {
        end = i
        break
      }
    }
  }
  if (end === -1) return null
  // Backward scan from `end` to find the tightest start.
  ti = m - 1
  let start = end
  for (let i = end; i >= 0; i--) {
    if (text[i] === term[ti]) {
      ti--
      if (ti < 0) {
        start = i
        break
      }
    }
  }
  // Score the window forward.
  ti = 0
  let score = 0
  let prevMatch = -2
  const positions: number[] = []
  for (let i = start; i <= end && ti < m; i++) {
    if (text[i] === term[ti]) {
      let s = 16
      if (isBoundary(text, i)) s += ti === 0 ? 20 : 10
      if (prevMatch === i - 1) s += 4
      score += s
      prevMatch = i
      if (wantPositions) positions.push(i)
      ti++
    } else {
      score -= prevMatch === i - 1 ? 3 : 1
    }
  }
  return { score, positions }
}

// Folded fields keyed by id + edit stamp, so usage-only library updates (every expansion)
// don't re-tokenize every snippet.
const folded = new Map<string, { key: string; name: string; nameLower: string; triggersLower: string[]; contentLower: string }>()

export function buildIndex(snippets: Snippet[], usage: Map<string, { count: number; last: number }>): SearchEntry[] {
  return snippets.map((s) => {
    const u = usage.get(s.id)
    const key = `${s.updatedAt}|${s.name}|${s.triggers.join("\u0000")}|${s.content.length}`
    let f = folded.get(s.id)
    if (!f || f.key !== key) {
      const name = s.name || s.triggers[0] || "Untitled"
      f = {
        key,
        name,
        nameLower: name.toLowerCase(),
        triggersLower: s.triggers.map((t) => t.toLowerCase()),
        contentLower: plainSummary(s.content, undefined, 2000).toLowerCase(),
      }
      folded.set(s.id, f)
    }
    return { snippet: s, name: f.name, nameLower: f.nameLower, triggersLower: f.triggersLower, contentLower: f.contentLower, usage: u?.count ?? 0, lastUsed: u?.last ?? 0 }
  })
}

function usageBoost(e: SearchEntry, now: number): number {
  const freq = Math.log2(1 + e.usage) * 4
  const ageDays = e.lastUsed ? (now - e.lastUsed) / 86_400_000 : Infinity
  const recency = ageDays < 1 ? 6 : ageDays < 7 ? 3 : ageDays < 30 ? 1 : 0
  return freq + recency
}

/** Ranks `entries` for `query`. An empty query sorts by usage then recency then name. */
export function search(entries: SearchEntry[], query: string, limit = 200): SearchHit[] {
  const now = Date.now()
  const q = query.trim().toLowerCase()
  if (!q) {
    return entries
      .map((e) => ({ snippet: e.snippet, score: usageBoost(e, now), field: null, nameHits: [] }) as SearchHit)
      .sort((a, b) => b.score - a.score || a.snippet.name.localeCompare(b.snippet.name))
      .slice(0, limit)
  }
  const terms = q.split(/\s+/).map((t) => t.slice(0, 64))
  const hits: SearchHit[] = []
  for (const e of entries) {
    let total = 0
    let field: SearchHit["field"] = null
    let best = -Infinity
    const nameHits: number[] = []
    let ok = true
    for (const term of terms) {
      let termBest = -Infinity
      let termField: SearchHit["field"] = null
      const n = fuzzy(term, e.nameLower, true)
      if (n) {
        termBest = n.score * 1.0
        termField = "name"
        nameHits.push(...n.positions)
      }
      for (const t of e.triggersLower) {
        if (t === term) {
          termBest = Math.max(termBest, 10_000)
          termField = "trigger"
          continue
        }
        if (t.startsWith(term)) {
          if (5_000 > termBest) {
            termBest = 5_000 - t.length
            termField = "trigger"
          }
          continue
        }
        const r = fuzzy(term, t, false)
        if (r && r.score * 0.85 > termBest) {
          termBest = r.score * 0.85
          termField = "trigger"
        }
      }
      // Content only matches contiguous text: fuzzy subsequences over long bodies are noise
      // (and the slowest part of a keystroke).
      if (termBest < 40 && term.length > 1) {
        const at = e.contentLower.indexOf(term)
        if (at !== -1) {
          const c = term.length * 16 + (isBoundary(e.contentLower, at) ? 10 : 0)
          if (c * 0.6 > termBest) {
            termBest = c * 0.6
            termField = "content"
          }
        }
      }
      if (termBest === -Infinity) {
        ok = false
        break
      }
      total += termBest
      if (termBest > best) {
        best = termBest
        field = termField
      }
    }
    if (!ok) continue
    total += usageBoost(e, now)
    if (!e.snippet.isEnabled) total -= 15
    hits.push({ snippet: e.snippet, score: total, field, nameHits: [...new Set(nameHits)].sort((a, b) => a - b) })
  }
  hits.sort((a, b) => b.score - a.score || a.snippet.name.localeCompare(b.snippet.name))
  return hits.slice(0, limit)
}
