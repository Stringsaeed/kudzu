"use client"

import { useRef } from "react"

import { cn } from "@/lib/utils"

export interface SegmentOption<T extends string> {
  value: T
  label: string
  description?: string
}

/** A radio group drawn as a segmented control. Arrow keys move between options. */
export function Segmented<T extends string>({
  value,
  options,
  onChange,
  label,
  className,
  size = "md",
}: {
  value: T
  options: SegmentOption<T>[]
  onChange: (v: T) => void
  label: string
  className?: string
  size?: "sm" | "md"
}) {
  const refs = useRef<(HTMLButtonElement | null)[]>([])
  const index = options.findIndex((o) => o.value === value)
  return (
    <div role="radiogroup" aria-label={label} className={cn("inline-flex rounded-lg bg-foreground/[0.04] p-0.5 shadow-well", className)}>
      {options.map((o, i) => (
        <button
          key={o.value}
          ref={(el) => {
            refs.current[i] = el
          }}
          role="radio"
          type="button"
          aria-checked={value === o.value}
          tabIndex={i === Math.max(0, index) ? 0 : -1}
          title={o.description}
          onClick={() => onChange(o.value)}
          onKeyDown={(e) => {
            const d = e.key === "ArrowRight" || e.key === "ArrowDown" ? 1 : e.key === "ArrowLeft" || e.key === "ArrowUp" ? -1 : 0
            if (!d) return
            e.preventDefault()
            const n = (i + d + options.length) % options.length
            onChange(options[n].value)
            refs.current[n]?.focus()
          }}
          className={cn(
            "rounded-md px-2.5 font-medium whitespace-nowrap text-muted-foreground transition-[color,background,box-shadow] duration-150 hover:text-foreground",
            size === "md" ? "h-7 text-xs" : "h-6 text-[11px]",
            value === o.value && "bg-surface-raised text-foreground shadow-soft sheen",
          )}
        >
          {o.label}
        </button>
      ))}
    </div>
  )
}
