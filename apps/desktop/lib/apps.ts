"use client"

import { useEffect, useState } from "react"

import { api } from "./backend"
import type { AppInfo } from "./types"

let cache: Promise<AppInfo[]> | null = null

/** Installed apps, loaded once per window. */
export function useInstalledApps(): AppInfo[] {
  const [apps, setApps] = useState<AppInfo[]>([])
  useEffect(() => {
    cache ??= api.listApps().catch(() => [])
    let live = true
    cache.then((a) => live && setApps(a))
    return () => {
      live = false
    }
  }, [])
  return apps
}

export function appName(apps: AppInfo[], bundleId: string): string {
  return apps.find((a) => a.bundleId === bundleId)?.name ?? bundleId
}
