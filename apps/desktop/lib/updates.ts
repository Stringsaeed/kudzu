// Update state shared by the About page and the background "update available" toast.

import { useEffect, useState } from "react"

import { api, Events, isTauri, on } from "./backend"
import type { SyncStatus, UpdateState } from "./types"

/** Public releases page (changelog and manual downloads). */
export const RELEASES_URL = "https://github.com/Stringsaeed/kudzu/releases"

/** Live update state, seeded from the backend and kept current by `kudzu://update`. */
export function useUpdateState(): [UpdateState, (s: UpdateState) => void] {
  const [state, setState] = useState<UpdateState>({ phase: "idle" })
  useEffect(() => {
    let live = true
    api.updateState().then((s) => live && s && setState(s), () => {})
    const off = on<UpdateState>(Events.update, setState)
    return () => {
      live = false
      off()
    }
  }, [])
  return [state, setState]
}

/**
 * release-please notes are Markdown with commit links; reduce them to readable lines:
 * headings ("Features"), and bullet items without scopes' bold markers or commit hashes.
 */
export function parseNotes(md: string | null): { heading?: string; items: string[] }[] {
  if (!md) return []
  const sections: { heading?: string; items: string[] }[] = []
  let cur: { heading?: string; items: string[] } = { items: [] }
  for (const raw of md.split("\n")) {
    const line = raw.trim()
    if (!line) continue
    const h = line.match(/^#{1,6}\s+(.*)$/)
    if (h) {
      if (h[1].match(/^\[?\d+\.\d+\.\d+/)) continue // the "## [0.2.0](compare…) (date)" title line
      if (cur.items.length || cur.heading) sections.push(cur)
      cur = { heading: h[1], items: [] }
      continue
    }
    const item = line.replace(/^[*-]\s+/, "")
    const clean = item
      .replace(/\s*\(\[[0-9a-f]{7,}\]\([^)]*\)\)/g, "") // ([abc1234](url))
      .replace(/\[([^\]]+)\]\([^)]*\)/g, "$1") // [text](url) → text
      .replace(/\*\*([^*]+)\*\*/g, "$1") // **scope:** → scope:
      .trim()
    if (clean) cur.items.push(clean)
  }
  if (cur.items.length || cur.heading) sections.push(cur)
  return sections
}

/** Opens a URL in the default browser (falls back to window.open in the web preview). */
export async function openExternal(url: string) {
  if (isTauri()) {
    const { openUrl } = await import("@tauri-apps/plugin-opener")
    await openUrl(url)
  } else {
    window.open(url, "_blank", "noopener")
  }
}

/** Live iCloud sync status, seeded from the backend and kept current by `kudzu://sync`. */
export function useSyncStatus(): SyncStatus {
  const [status, setStatus] = useState<SyncStatus>({ state: "starting" })
  useEffect(() => {
    let live = true
    api.syncStatus().then((s) => live && s && setStatus(s), () => {})
    const off = on<SyncStatus>(Events.sync, setStatus)
    return () => {
      live = false
      off()
    }
  }, [])
  return status
}
