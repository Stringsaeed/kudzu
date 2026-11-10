"use client"

// Small motion helpers shared by the snippet list and the editor. They only ever animate
// transform/opacity, read their timings from the tokens in globals.css, and respect reduced motion.

import { useCallback, useLayoutEffect, useRef, useState } from "react"

import { cn } from "@/lib/utils"

/** Counts real changes since mount, so a swap animates but mounting stays still. Shared with the shell. */
export { useChanges as useBump } from "@/hooks/use-changes"

const reducedMotion = () => typeof window !== "undefined" && window.matchMedia("(prefers-reduced-motion: reduce)").matches

function token(name: string, fallback: string): string {
  return getComputedStyle(document.documentElement).getPropertyValue(name).trim() || fallback
}

const duration = (name: string, fallback: number) => Number.parseFloat(token(name, `${fallback}ms`)) || fallback

/** A short sideways shake: "that input was rejected". */
export function nudge(el: Element | null | undefined) {
  if (!el || reducedMotion()) return
  el.animate(
    [{ translate: "0 0" }, { translate: "-3px 0" }, { translate: "3px 0" }, { translate: "-2px 0" }, { translate: "1px 0" }, { translate: "0 0" }],
    { duration: duration("--motion-slow", 240), easing: "ease-out" },
  )
}

/**
 * Lands an element: from `scale` back to 1 on the pop spring. With `fade` it also fades in (something
 * just arrived); without, it reads as "here it is" (pointing at something that already exists).
 */
export function land(el: Element | null | undefined, { scale = 0.85, fade = false }: { scale?: number; fade?: boolean } = {}) {
  if (!el || reducedMotion()) return
  el.animate([{ scale: String(scale), ...(fade ? { opacity: 0 } : {}) }, { scale: "1", ...(fade ? { opacity: 1 } : {}) }], {
    duration: duration("--motion-slow", 240),
    easing: token("--spring-pop", "ease-out"),
  })
}

/**
 * FLIP for wrapping rows of chips: call `snapshot()` right before a change that reflows them (say,
 * removing one), and every child marked `data-flip="<stable key>"` slides from its old place to the
 * new one instead of jumping. Translate only, so nothing else in the layout moves.
 */
export function useFlip<T extends HTMLElement>() {
  const ref = useRef<T>(null)
  const before = useRef<Map<string, DOMRect> | null>(null)
  const items = () => [...(ref.current?.querySelectorAll<HTMLElement>("[data-flip]") ?? [])]

  const snapshot = useCallback(() => {
    before.current = new Map(items().map((el) => [el.dataset.flip!, el.getBoundingClientRect()]))
  }, [])

  useLayoutEffect(() => {
    const prev = before.current
    if (!prev) return
    before.current = null
    if (reducedMotion()) return
    const ms = duration("--motion-slow", 240)
    const easing = token("--spring-soft", "ease-out")
    for (const el of items()) {
      const a = prev.get(el.dataset.flip!)
      if (!a) continue
      const b = el.getBoundingClientRect()
      const dx = a.left - b.left
      const dy = a.top - b.top
      if (Math.abs(dx) < 0.5 && Math.abs(dy) < 0.5) continue
      el.animate([{ translate: `${dx}px ${dy}px` }, { translate: "0 0" }], { duration: ms, easing })
    }
  })

  return { ref, snapshot }
}

/**
 * Shows `children` while `show` is on. Turning on after mount plays `enter`; turning off plays `exit`
 * and unmounts when it ends. Mounting plays nothing, so state that was already true stays still.
 */
export function Presence({
  show,
  enter,
  exit = "animate-vanish",
  className,
  children,
}: {
  show: boolean
  enter: string
  exit?: string
  className?: string
  children: React.ReactNode
}) {
  const [state, setState] = useState<{ show: boolean; phase: "idle" | "enter" | "exit" }>({ show, phase: "idle" })
  if (state.show !== show) setState({ show, phase: show ? "enter" : "exit" })
  const phase = state.show !== show ? (show ? "enter" : "exit") : state.phase
  if (!show && phase !== "exit") return null
  return (
    <span
      aria-hidden={phase === "exit" || undefined}
      className={cn("inline-flex shrink-0", className, phase === "enter" && enter, phase === "exit" && exit)}
      onAnimationEnd={(e) => {
        if (e.target === e.currentTarget && phase === "exit") setState({ show, phase: "idle" })
      }}
    >
      {children}
    </span>
  )
}
