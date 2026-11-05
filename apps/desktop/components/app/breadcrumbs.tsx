"use client"

import { MoreHorizontal } from "lucide-react"
import { Fragment } from "react"

import { Breadcrumb, BreadcrumbItem, BreadcrumbLink, BreadcrumbList, BreadcrumbPage, BreadcrumbSeparator } from "@/components/ui/breadcrumb"
import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuTrigger } from "@/components/ui/dropdown-menu"

export type BreadcrumbEntry = { label: string; onSelect?: () => void }

export function Breadcrumbs({ items, label = "Breadcrumb" }: { items: BreadcrumbEntry[]; label?: string }) {
  const collapsed = items.length > 4
  const hidden = collapsed ? items.slice(1, -2) : []
  const visible = collapsed ? [items[0], ...items.slice(-2)] : items

  return (
    <Breadcrumb aria-label={label} className="no-drag min-w-0">
      <BreadcrumbList>
        {visible.map((item, index) => (
          <Fragment key={index}>
            {index > 0 && <BreadcrumbSeparator />}
            {collapsed && index === 1 && (
              <>
                <BreadcrumbItem className="shrink-0">
                  <DropdownMenu>
                    <DropdownMenuTrigger className="flex size-6 items-center justify-center rounded-md hover:bg-accent hover:text-foreground" aria-label="Show parent groups">
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
                <BreadcrumbPage title={item.label}>{item.label}</BreadcrumbPage>
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
