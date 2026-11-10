"use client"

import { MoreHorizontal } from "lucide-react"
import { Fragment, useLayoutEffect, useRef, useState } from "react"

import { Breadcrumb, BreadcrumbItem, BreadcrumbLink, BreadcrumbList, BreadcrumbPage, BreadcrumbSeparator } from "@/components/ui/breadcrumb"
import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuTrigger } from "@/components/ui/dropdown-menu"

/** `id` says what the crumb stands for. Give one when the label can change under it (a snippet being renamed), so retyping doesn't count as arriving somewhere new. */
export type BreadcrumbEntry = { label: string; id?: string; onSelect?: () => void }

/** Changes closer together than this are a burst (a held arrow key), where replaying the entrance would strobe. */
const BURST_MS = 300

/** Counts arrivals at a new `id`, skipping mount and bursts. Key the crumb on it to replay its entrance. */
function useArrivals(id: string) {
  const [arrivals, setArrivals] = useState(0)
  const last = useRef({ id, at: 0 })
  // Layout effect, so the keyed crumb is swapped in before paint and never shows un-animated first.
  useLayoutEffect(() => {
    if (last.current.id === id) return
    const now = performance.now()
    const burst = now - last.current.at < BURST_MS
    last.current = { id, at: now }
    if (!burst) setArrivals((n) => n + 1)
  }, [id])
  return arrivals
}

export function Breadcrumbs({ items, label = "Breadcrumb" }: { items: BreadcrumbEntry[]; label?: string }) {
  const collapsed = items.length > 4
  const hidden = collapsed ? items.slice(1, -2) : []
  const visible = collapsed ? [items[0], ...items.slice(-2)] : items
  const current = items[items.length - 1]
  const arrivals = useArrivals(current ? (current.id ?? current.label) : "")

  return (
    <Breadcrumb aria-label={label} className="min-w-0">
      <BreadcrumbList>
        {visible.map((item, index) => (
          <Fragment key={index}>
            {index > 0 && <BreadcrumbSeparator />}
            {collapsed && index === 1 && (
              <>
                <BreadcrumbItem className="shrink-0">
                  <DropdownMenu>
                    <DropdownMenuTrigger className="flex size-6 items-center justify-center rounded-md press hover:bg-accent hover:text-foreground" aria-label="Show parent groups">
                      <MoreHorizontal className="size-4" />
                    </DropdownMenuTrigger>
                    <DropdownMenuContent align="start">
                      {hidden.map((parent, i) => <DropdownMenuItem key={i} onSelect={parent.onSelect}>{parent.label}</DropdownMenuItem>)}
                    </DropdownMenuContent>
                  </DropdownMenu>
                </BreadcrumbItem>
                <BreadcrumbSeparator />
              </>
            )}
            <BreadcrumbItem className={index === visible.length - 1 ? "flex-1" : "max-w-40"}>
              {index === visible.length - 1 ? (
                <BreadcrumbPage key={arrivals} title={item.label} className={arrivals > 0 ? "animate-rise" : undefined}>{item.label}</BreadcrumbPage>
              ) : (
                <BreadcrumbLink asChild>
                  <button type="button" onClick={item.onSelect} title={item.label}>{item.label}</button>
                </BreadcrumbLink>
              )}
            </BreadcrumbItem>
          </Fragment>
        ))}
      </BreadcrumbList>
    </Breadcrumb>
  )
}
