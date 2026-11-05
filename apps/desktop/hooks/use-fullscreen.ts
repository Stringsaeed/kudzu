import { getCurrentWindow } from "@tauri-apps/api/window"
import { useEffect, useState } from "react"

import { isTauri } from "@/lib/backend"

/** Whether the current window is in native fullscreen. Always false in the browser preview. */
export function useFullscreen() {
  const [fullscreen, setFullscreen] = useState(false)

  useEffect(() => {
    if (!isTauri()) return
    const win = getCurrentWindow()
    let live = true
    const sync = () => {
      win.isFullscreen().then((value) => {
        if (live) setFullscreen(value)
      }, () => {})
    }
    sync()
    // There is no dedicated fullscreen event; macOS emits a resize once each transition settles.
    const unlisten = win.onResized(sync)
    return () => {
      live = false
      void unlisten.then((stop) => stop(), () => {})
    }
  }, [])

  return fullscreen
}
