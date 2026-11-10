"use client"

import { PanelLeft, Search } from "lucide-react"

import { Button } from "@/components/ui/button"
import { Separator } from "@/components/ui/separator"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { useFullscreen } from "@/hooks/use-fullscreen"
import { FILTERS } from "@/lib/clipboard"
import { useStore, type View } from "@/lib/store"
import { Breadcrumbs, type BreadcrumbEntry } from "../breadcrumbs"
import { Shortcut } from "../shortcut"
import { SETTINGS_SECTIONS } from "./settings"

export function Toolbar({ sidebarOpen, onToggleSidebar, onNavigate }: { sidebarOpen: boolean; onToggleSidebar: () => void; onNavigate: (view: View) => void }) {
  const view = useStore((s) => s.view)
  const groups = useStore((s) => s.library.groups)
  const snippet = useStore((s) => s.library.snippets.find((item) => item.id === s.selectedId))
  const setCommandOpen = useStore((s) => s.setCommandOpen)
  const platform = useStore((s) => s.platform)
  // macOS hides the traffic lights in fullscreen, so their inset would just be empty space.
  const fullscreen = useFullscreen()
  const items: BreadcrumbEntry[] = []
  const add = (label: string, destination: View) => items.push({ label, onSelect: () => onNavigate(destination) })

  switch (view.kind) {
    case "settings":
      add("Settings", { kind: "settings" })
      items.push({ label: SETTINGS_SECTIONS.find((s) => s.id === (view.section ?? "general"))?.label ?? "Settings" })
      break
    case "insights":
      items.push({ label: "Insights" })
      break
    case "clipboard":
      add("Clipboard", { kind: "clipboard" })
      items.push({ label: view.filter && view.filter !== "all" ? FILTERS.find((f) => f.value === view.filter)?.label ?? "History" : "History" })
      break
    default: {
      add("Snippets", { kind: "all" })
      if (view.kind === "favorites") add("Favorites", view)
      if (view.kind === "recent") add("Recently used", view)
      if (view.kind === "disabled") add("Turned off", view)
      const ancestors: BreadcrumbEntry[] = []
      const seen = new Set<string>()
      let group = groups.find((g) => g.id === (snippet?.groupID ?? (view.kind === "group" ? view.id : undefined)))
      while (group && !seen.has(group.id)) {
        const id = group.id
        seen.add(id)
        ancestors.unshift({ label: group.name, id: `group:${id}`, onSelect: () => onNavigate({ kind: "group", id }) })
        group = groups.find((g) => g.id === group?.parentID)
      }
      items.push(...ancestors)
      if (snippet) items.push({ label: snippet.name || "Untitled", id: `snippet:${snippet.id}` })
      else if (view.kind === "all") items.push({ label: "All snippets" })
    }
  }

  return (
    // "deep" so empty space inside the breadcrumbs also drags; Tauri still excludes buttons and links.
    <header data-tauri-drag-region="deep" className={`drag flex h-[42px] shrink-0 items-center gap-3 border-b border-hairline pr-4 ${platform === "macos" && !fullscreen ? "pl-[88px]" : "pl-3"}`}>
      <Tooltip>
        <TooltipTrigger asChild>
          <Button variant="ghost" size="icon" className="no-drag text-muted-foreground" onClick={onToggleSidebar} aria-label={sidebarOpen ? "Hide sidebar" : "Show sidebar"} aria-expanded={sidebarOpen} aria-controls="main-sidebar">
            <PanelLeft />
          </Button>
        </TooltipTrigger>
        <TooltipContent>{sidebarOpen ? "Hide sidebar" : "Show sidebar"}</TooltipContent>
      </Tooltip>
      <Separator orientation="vertical" className="my-3" />
      <div className="min-w-0 flex-1"><Breadcrumbs items={items} /></div>
      <Button variant="outline" className="no-drag h-8 w-48 justify-start gap-2 text-xs text-muted-foreground max-[1000px]:w-36" onClick={() => setCommandOpen(true)} aria-label="Search commands and snippets">
        <Search className="size-3.5" />
        <span className="flex-1 text-left">Search…</span>
        <Shortcut combo="mod+k" />
      </Button>
    </header>
  )
}
