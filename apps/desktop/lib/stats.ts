// Usage aggregation for the Insights view, list badges and search ranking.
// Usage records are per device; everything here sums across devices. Typed text is never stored.

import type { Library } from "./types"

export interface SnippetUsage {
  count: number
  chars: number
  last: number
}

export function usageById(library: Library): Map<string, SnippetUsage> {
  const map = new Map<string, SnippetUsage>()
  for (const u of library.usage) {
    const cur = map.get(u.snippetID) ?? { count: 0, chars: 0, last: 0 }
    cur.count += u.count
    cur.chars += Math.max(0, u.charsSaved)
    const t = u.lastUsedAt ? Date.parse(u.lastUsedAt) : 0
    if (t > cur.last) cur.last = t
    map.set(u.snippetID, cur)
  }
  return map
}

export interface Totals {
  expansions: number
  chars: number
  minutesSaved: number
  activeSnippets: number
  usedSnippets: number
}

/** Average word length used by typing-speed conventions (5 characters per word). */
export const CHARS_PER_WORD = 5

export function totals(library: Library, usage: Map<string, SnippetUsage>, wpm: number): Totals {
  let expansions = 0
  let chars = 0
  for (const u of usage.values()) {
    expansions += u.count
    chars += u.chars
  }
  return {
    expansions,
    chars,
    minutesSaved: chars / CHARS_PER_WORD / Math.max(1, wpm),
    activeSnippets: library.snippets.filter((s) => s.isEnabled).length,
    usedSnippets: [...usage.values()].filter((u) => u.count > 0).length,
  }
}

export function formatDuration(minutes: number): string {
  if (minutes < 1) return `${Math.round(minutes * 60)}s`
  if (minutes < 60) return `${Math.round(minutes)} min`
  const h = minutes / 60
  if (h < 24) return `${h.toFixed(h < 10 ? 1 : 0)} h`
  return `${(h / 24).toFixed(1)} days`
}

export function formatCount(n: number): string {
  if (n < 1000) return String(n)
  if (n < 1_000_000) return `${(n / 1000).toFixed(n < 10_000 ? 1 : 0)}k`
  return `${(n / 1_000_000).toFixed(1)}M`
}

const rtf = typeof Intl !== "undefined" ? new Intl.RelativeTimeFormat(undefined, { numeric: "auto" }) : null

export function relativeTime(ms: number): string {
  if (!ms) return "Never"
  const diff = (ms - Date.now()) / 1000
  const abs = Math.abs(diff)
  if (!rtf) return new Date(ms).toLocaleDateString()
  if (abs < 60) return "Just now"
  if (abs < 3600) return rtf.format(Math.round(diff / 60), "minute")
  if (abs < 86_400) return rtf.format(Math.round(diff / 3600), "hour")
  if (abs < 86_400 * 30) return rtf.format(Math.round(diff / 86_400), "day")
  return new Date(ms).toLocaleDateString(undefined, { month: "short", day: "numeric", year: "numeric" })
}
