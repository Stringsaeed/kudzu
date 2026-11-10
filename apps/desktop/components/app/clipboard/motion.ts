"use client"

// Small state helpers so animations play on real changes only: never on first load, and never
// again when a virtual-list row scrolls back into view and remounts.

import { useEffect, useState } from "react"

import type { ClipEntry } from "@/lib/types"

/**
 * True once `value` has changed while `key` stayed the same (e.g. this entry got pinned while it
 * was on screen). Resets when `key` changes, so moving to another entry doesn't count.
 */
export function useChanged(key: string, value: unknown): boolean {
  const [seen, setSeen] = useState({ key, value, changed: false })
  if (seen.key !== key) {
    setSeen({ key, value, changed: false })
    return false
  }
  if (!Object.is(seen.value, value)) {
    setSeen({ key, value, changed: true })
    return true
  }
  return seen.changed
}

const EMPTY = new Set<string>()
/** More changes than this at once is a load or a bulk action: no motion, just the new state. */
const BULK = 10
const SETTLE_MS = 400

/**
 * What just changed in the history: `fresh` entries (new, copied again or restored) and whether the
 * list is `settling`, i.e. rows may glide to new places after an insert, delete or pin. Worked out
 * during render, so the class lands in the same commit as the new positions.
 */
export function useListChanges(entries: ClipEntry[], ready: boolean): { fresh: Set<string>; settling: boolean } {
  const [s, setS] = useState({ entries, ready, fresh: EMPTY, settling: false, stamp: 0 })
  if (s.entries !== entries || s.ready !== ready) {
    const visible = typeof document === "undefined" || document.visibilityState === "visible"
    let next = { ...s, entries, ready }
    if (s.ready && ready && visible) {
      const before = new Map(s.entries.map((e) => [e.id, e]))
      const fresh = new Set<string>()
      let moved = 0
      for (const e of entries) {
        const b = before.get(e.id)
        if (!b || b.copiedAt !== e.copiedAt) fresh.add(e.id)
        else if (b.pinned !== e.pinned) moved++
        before.delete(e.id)
      }
      moved += before.size // removed
      const total = fresh.size + moved
      if (total > 0 && total <= BULK) next = { entries, ready, fresh, settling: true, stamp: s.stamp + 1 }
    }
    setS(next)
  }

  useEffect(() => {
    if (!s.settling) return
    const stamp = s.stamp
    const t = setTimeout(() => setS((c) => (c.stamp === stamp ? { ...c, fresh: EMPTY, settling: false } : c)), SETTLE_MS)
    return () => clearTimeout(t)
  }, [s.stamp, s.settling])

  return { fresh: s.fresh, settling: s.settling }
}

/**
 * Keeps the last non-null `value` around for `ms` after it goes null, so it can play an exit.
 * `value` must be stable between renders (memoize it).
 */
export function useExit<T>(value: T | null, ms: number): { shown: T | null; leaving: boolean } {
  const [last, setLast] = useState<T | null>(value)
  if (value !== null && value !== last) setLast(value)
  const leaving = value === null && last !== null
  useEffect(() => {
    if (!leaving) return
    const t = setTimeout(() => setLast(null), ms)
    return () => clearTimeout(t)
  }, [leaving, ms])
  return { shown: value ?? last, leaving }
}
