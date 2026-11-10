"use client"

import { useEffect, useState } from "react"

import { api, isTauri } from "@/lib/backend"
import { cn } from "@/lib/utils"

const cache = new Map<string, Promise<string | null>>()
/** Icons that have finished loading, readable during render so a known icon is there on the first frame. */
const loaded = new Map<string, string | null>()

function load(path: string) {
  if (!cache.has(path)) {
    cache.set(
      path,
      api
        .appIcon(path)
        .catch(() => null)
        .then((src) => {
          loaded.set(path, src)
          return src
        }),
    )
  }
  return cache.get(path)!
}

/** An app's icon (from its bundle), falling back to a monogram tile. */
export function AppIcon({ path, name, size = 18, className }: { path?: string | null; name: string; size?: number; className?: string }) {
  // The path whose icon arrived while this was on screen: that one fades in over the monogram's place.
  const [arrived, setArrived] = useState<string | null>(null)
  useEffect(() => {
    if (!path || !isTauri() || loaded.has(path)) return
    let live = true
    void load(path).then(() => live && setArrived(path))
    return () => {
      live = false
    }
  }, [path])
  const src = path ? loaded.get(path) : null
  // oxlint-disable-next-line nextjs/no-img-element -- static export; data: URL from the backend
  if (src) return <img src={src} alt="" width={size} height={size} className={cn("shrink-0", arrived === path && "animate-in fade-in-0 duration-(--motion-moderate)", className)} draggable={false} />
  return (
    <span
      aria-hidden
      style={{ width: size, height: size, fontSize: size * 0.5 }}
      className={cn("inline-flex shrink-0 items-center justify-center rounded-[5px] bg-foreground/[0.08] font-semibold text-muted-foreground", className)}
    >
      {name.slice(0, 1).toUpperCase()}
    </span>
  )
}
