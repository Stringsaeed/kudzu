"use client"

import { useEffect, useLayoutEffect, useRef } from "react"

import { cn } from "@/lib/utils"

const SELECTED = '[aria-checked="true"], [aria-current="page"]'

/** Walks offsetParents rather than comparing rects, so a transform mid-animation can't skew it. */
function offsetWithin(el: HTMLElement, root: HTMLElement) {
  let x = 0
  let y = 0
  let node: HTMLElement | null = el
  while (node && node !== root) {
    x += node.offsetLeft
    y += node.offsetTop
    node = node.offsetParent as HTMLElement | null
  }
  return { x, y }
}

function place(pill: HTMLElement) {
  const group = pill.parentElement
  const on = group?.querySelector<HTMLElement>(SELECTED)
  if (!group || !on?.offsetWidth) {
    pill.style.opacity = "0"
    delete pill.dataset.slide
    return
  }
  const { x, y } = offsetWithin(on, group)
  pill.style.translate = `${x}px ${y}px`
  pill.style.width = `${on.offsetWidth}px`
  pill.style.height = `${on.offsetHeight}px`
  pill.style.opacity = "1"
  if (pill.dataset.slide === undefined) {
    // Appearing: commit the position first, so it fades in where it belongs instead of sliding there.
    void pill.offsetWidth
    pill.dataset.slide = ""
  }
}

/**
 * One background for a group's selected option, sliding from option to option. Render it as the
 * first child of a `relative isolate` group and leave the background off the options themselves.
 * The selected option is found by aria-checked or aria-current="page"; with none, the pill fades out.
 */
export function SelectionPill({ className }: { className?: string }) {
  const ref = useRef<HTMLSpanElement>(null)

  // Every render: selection, labels and collapsed rows all change through the parent.
  useLayoutEffect(() => {
    if (ref.current) place(ref.current)
  })

  useEffect(() => {
    const pill = ref.current
    const group = pill?.parentElement
    if (!pill || !group) return
    const observer = new ResizeObserver(() => place(pill))
    observer.observe(group)
    for (const child of group.children) observer.observe(child)
    return () => observer.disconnect()
  }, [])

  return (
    <span
      ref={ref}
      aria-hidden
      className={cn(
        "pointer-events-none absolute top-0 left-0 -z-10 opacity-0 transition-opacity duration-(--motion-fast)",
        "data-slide:transition-[translate,width,height,opacity] data-slide:duration-(--motion-slow) data-slide:ease-spring",
        className,
      )}
    />
  )
}
