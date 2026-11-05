"use client"

import { useEffect, useRef } from "react"

import { isEditable, matches } from "@/lib/keys"

export interface Binding {
  combo: string | string[]
  handler: (e: KeyboardEvent) => void
  /** Fire even while typing in an input/editor (for mod-shortcuts like ⌘N). */
  allowInInput?: boolean
  enabled?: boolean
}

/** Window-level shortcuts. Bindings are read through a ref, so callers needn't memoize. */
export function useHotkeys(bindings: Binding[], deps: { capture?: boolean } = {}) {
  const ref = useRef(bindings)
  useEffect(() => {
    ref.current = bindings
  })
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.defaultPrevented || e.isComposing) return
      const editable = isEditable(e.target)
      for (const b of ref.current) {
        if (b.enabled === false) continue
        if (editable && !b.allowInInput) continue
        const combos = Array.isArray(b.combo) ? b.combo : [b.combo]
        if (combos.some((c) => matches(e, c))) {
          e.preventDefault()
          b.handler(e)
          return
        }
      }
    }
    window.addEventListener("keydown", onKey, { capture: deps.capture })
    return () => window.removeEventListener("keydown", onKey, { capture: deps.capture })
  }, [deps.capture])
}
