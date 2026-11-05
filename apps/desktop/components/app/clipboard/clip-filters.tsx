"use client"

import { useRef } from "react"

import { FILTERS, type ClipFilter } from "@/lib/clipboard"
import { cn } from "@/lib/utils"

/**
 * Kind filters as a single-select chip row (a radio group: arrow keys move, one tab stop).
 * Scrolls sideways when narrow, with a soft fade at the edge.
 */
export function ClipFilters({
  value,
  onChange,
  counts,
  options = FILTERS.map((f) => f.value),
  className,
  focusable = true,
}: {
  value: ClipFilter
  onChange: (f: ClipFilter) => void
  counts?: Partial<Record<ClipFilter, number>>
  options?: ClipFilter[]
  className?: string
  /** False in the picker, where Tab cycles filters and focus stays in the search field. */
  focusable?: boolean
}) {
  const refs = useRef<(HTMLButtonElement | null)[]>([])
  const list = FILTERS.filter((f) => options.includes(f.value))
  const index = list.findIndex((f) => f.value === value)
  return (
    <div
      role="radiogroup"
      aria-label="Show"
      className={cn("flex items-center gap-1 overflow-x-auto [scrollbar-width:none] [&::-webkit-scrollbar]:hidden", className)}
    >
      {list.map((f, i) => {
        const on = f.value === value
        const n = counts?.[f.value]
        const hidden = n === 0 && f.value !== "all" && !on
        return (
          <button
            key={f.value}
            ref={(el) => {
              refs.current[i] = el
            }}
            type="button"
            role="radio"
            aria-checked={on}
            tabIndex={focusable ? (i === Math.max(0, index) ? 0 : -1) : -1}
            onMouseDown={(e) => !focusable && e.preventDefault()}
            onClick={() => onChange(f.value)}
            onKeyDown={(e) => {
              const d = e.key === "ArrowRight" ? 1 : e.key === "ArrowLeft" ? -1 : 0
              if (!d) return
              e.preventDefault()
              const n2 = (i + d + list.length) % list.length
              onChange(list[n2].value)
              refs.current[n2]?.focus()
              refs.current[n2]?.scrollIntoView({ block: "nearest", inline: "nearest" })
            }}
            className={cn(
              "flex h-6 shrink-0 items-center gap-1 rounded-full px-2.5 text-[11.5px] font-medium whitespace-nowrap text-muted-foreground transition-[color,background,box-shadow] duration-150",
              "hover:bg-foreground/[0.05] hover:text-foreground",
              on && "bg-surface-raised text-foreground shadow-soft sheen hover:bg-surface-raised",
              hidden && "opacity-45",
            )}
          >
            {f.label}
            {n !== undefined && f.value !== "all" && n > 0 && <span className={cn("text-[10.5px] tabular", on ? "text-muted-foreground" : "text-faint")}>{n}</span>}
          </button>
        )
      })}
    </div>
  )
}
