"use client"

// Recent expansion attempts for Settings ▸ Activity. The backend keeps them in memory only
// (about 100, cleared on quit); nothing here is persisted either.

import { useCallback, useEffect, useState } from "react"

import { api, Events, on } from "./backend"
import type { ActivityEntry } from "./types"

/** Matches kudzu-core `activity::CAPACITY`. */
export const ACTIVITY_CAPACITY = 100

/** The log, newest first, kept live while the caller is mounted. */
export function useActivity(): { entries: ActivityEntry[]; ready: boolean; clear: () => void } {
  const [entries, setEntries] = useState<ActivityEntry[]>([])
  const [ready, setReady] = useState(false)

  useEffect(() => {
    let live = true
    // Subscribe first so nothing that happens during the initial load is missed.
    const off = on<ActivityEntry>(Events.activity, (entry) => {
      setEntries((current) => (current.some((e) => e.id === entry.id) ? current : [entry, ...current].slice(0, ACTIVITY_CAPACITY)))
    })
    api.activityList().then(
      (list) => {
        if (!live) return
        setEntries((current) => {
          const seen = new Set(list.map((e) => e.id))
          return [...current.filter((e) => !seen.has(e.id)), ...list].slice(0, ACTIVITY_CAPACITY)
        })
        setReady(true)
      },
      () => live && setReady(true),
    )
    return () => {
      live = false
      off()
    }
  }, [])

  const clear = useCallback(() => {
    setEntries([])
    void api.activityClear()
  }, [])

  return { entries, ready, clear }
}

/** Clock time for today's entries, with the date for older ones. */
export function activityTime(at: string): string {
  const date = new Date(at)
  if (Number.isNaN(date.getTime())) return ""
  const time = date.toLocaleTimeString(undefined, { hour: "numeric", minute: "2-digit" })
  return date.toDateString() === new Date().toDateString() ? time : `${date.toLocaleDateString(undefined, { month: "short", day: "numeric" })}, ${time}`
}
