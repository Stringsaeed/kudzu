"use client"

import { Command as CommandPrimitive } from "cmdk"
import {
  BarChart3,
  CircleOff,
  ClipboardList,
  Clock3,
  Copy,
  Download,
  FileText,
  Folder,
  FolderPlus,
  Keyboard,
  Layers,
  Moon,
  Pause,
  Play,
  Plus,
  Search,
  Settings,
  SlidersHorizontal,
  Star,
  Sun,
  Trash2,
  Upload,
} from "lucide-react"
import { useMemo, useState } from "react"

import { Dialog, DialogContent, DialogDescription, DialogTitle } from "@/components/ui/dialog"
import { buildTree, flatten, groupPath } from "@/lib/groups"
import { buildIndex, search } from "@/lib/search"
import { useStore } from "@/lib/store"
import type { Group } from "@/lib/types"
import { cn } from "@/lib/utils"

import { Shortcut } from "../shortcut"
import { TriggerChip } from "../trigger-chip"
import { runExport, runImport } from "./data-actions"

interface Action {
  id: string
  label: string
  keywords?: string
  icon: React.ComponentType<{ className?: string }>
  shortcut?: string
  run: () => void
  section: "Selected snippet" | "Actions" | "Go to"
}

function Item({ onSelect, children, value }: { onSelect: () => void; children: React.ReactNode; value: string }) {
  return (
    <CommandPrimitive.Item
      value={value}
      onSelect={onSelect}
      className="flex h-9 cursor-default items-center gap-2.5 rounded-lg px-2.5 text-[13px] text-foreground/90 outline-none data-[selected=true]:bg-foreground/[0.07] data-[selected=true]:text-foreground [&_svg]:size-4 [&_svg]:shrink-0 [&_svg]:text-muted-foreground"
    >
      {children}
    </CommandPrimitive.Item>
  )
}

function Heading({ children }: { children: React.ReactNode }) {
  return <div className="px-2.5 pt-3 pb-1 text-[11px] font-medium text-faint">{children}</div>
}

export function CommandMenu({ onOpenGroupSettings, onDelete }: { onOpenGroupSettings: (g: Group) => void; onDelete: (ids: string[]) => void }) {
  const open = useStore((s) => s.commandOpen)
  const setOpen = useStore((s) => s.setCommandOpen)
  const library = useStore((s) => s.library)
  const usage = useStore((s) => s.usage)
  const status = useStore((s) => s.status)
  const settings = useStore((s) => s.settings)
  const selectedId = useStore((s) => s.selectedId)
  const st = useStore.getState
  const [query, setQuery] = useState("")

  const index = useMemo(() => buildIndex(library.snippets, usage), [library.snippets, usage])
  const snippetHits = useMemo(() => search(index, query, query ? 8 : 5), [index, query])
  const selected = library.snippets.find((s) => s.id === selectedId)

  const close = () => {
    setOpen(false)
    setQuery("")
  }
  const run = (fn: () => void) => () => {
    close()
    requestAnimationFrame(fn)
  }

  const actions: Action[] = useMemo(() => {
    const a: Action[] = []
    if (selected) {
      const name = selected.name || "Untitled"
      a.push(
        { id: "fav", section: "Selected snippet", icon: Star, label: selected.isFavorite ? `Unfavorite “${name}”` : `Favorite “${name}”`, shortcut: "mod+shift+f", run: () => st().toggleFavorite(selected.id) },
        { id: "en", section: "Selected snippet", icon: CircleOff, label: selected.isEnabled ? `Turn off “${name}”` : `Turn on “${name}”`, shortcut: "mod+shift+e", run: () => st().toggleEnabled(selected.id) },
        { id: "dup", section: "Selected snippet", icon: Copy, label: `Duplicate “${name}”`, shortcut: "mod+d", run: () => st().duplicate(selected.id) },
        { id: "del", section: "Selected snippet", icon: Trash2, label: `Delete “${name}”`, shortcut: "mod+backspace", run: () => onDelete([selected.id]) },
      )
    }
    a.push(
      { id: "new", section: "Actions", icon: Plus, label: "New snippet", keywords: "create add", shortcut: "mod+n", run: () => st().newSnippet() },
      { id: "newg", section: "Actions", icon: FolderPlus, label: "New group", keywords: "folder create", shortcut: "mod+shift+n", run: () => st().createGroup("New group", null) },
      { id: "pause", section: "Actions", icon: status.paused ? Play : Pause, label: status.paused ? "Resume expanding" : "Pause expanding", keywords: "stop disable", shortcut: "mod+shift+p", run: () => st().setPaused(!status.paused) },
      { id: "imp", section: "Actions", icon: Download, label: "Import snippets…", keywords: "raycast textexpander espanso json yaml csv", run: () => runImport() },
      { id: "exp", section: "Actions", icon: Upload, label: "Export library…", keywords: "backup json espanso", run: () => runExport("json") },
      {
        id: "theme",
        section: "Actions",
        icon: settings.theme === "dark" ? Sun : Moon,
        label: settings.theme === "dark" ? "Switch to light appearance" : "Switch to dark appearance",
        keywords: "theme mode color",
        run: () => st().updateSettings({ theme: settings.theme === "dark" ? "light" : "dark" }),
      },
      { id: "keys", section: "Actions", icon: Keyboard, label: "Keyboard shortcuts", keywords: "help keys", shortcut: "mod+/", run: () => st().setShortcutsOpen(true) },
      { id: "all", section: "Go to", icon: Layers, label: "All snippets", shortcut: "mod+1", run: () => st().setView({ kind: "all" }) },
      { id: "favs", section: "Go to", icon: Star, label: "Favorites", shortcut: "mod+2", run: () => st().setView({ kind: "favorites" }) },
      { id: "recent", section: "Go to", icon: Clock3, label: "Recently used", shortcut: "mod+3", run: () => st().setView({ kind: "recent" }) },
      { id: "ins", section: "Go to", icon: BarChart3, label: "Insights", keywords: "stats statistics usage", shortcut: "mod+4", run: () => st().setView({ kind: "insights" }) },
      { id: "clip", section: "Go to", icon: ClipboardList, label: "Clipboard history", keywords: "copy paste history copied", shortcut: "mod+5", run: () => st().setView({ kind: "clipboard" }) },
      { id: "clipset", section: "Go to", icon: Settings, label: "Settings › Clipboard", keywords: "history retention keep days privacy", run: () => st().setView({ kind: "settings", section: "clipboard" }) },
      {
        id: "clippause",
        section: "Actions",
        icon: settings.clipboardEnabled ? Pause : Play,
        label: settings.clipboardEnabled ? "Pause clipboard history" : "Resume clipboard history",
        keywords: "copy record privacy stop",
        run: () => st().updateSettings({ clipboardEnabled: !settings.clipboardEnabled }),
      },
      { id: "set", section: "Go to", icon: Settings, label: "Settings", keywords: "preferences", shortcut: "mod+,", run: () => st().setView({ kind: "settings" }) },
      { id: "apps", section: "Go to", icon: Settings, label: "Settings › Apps", keywords: "exclude block terminal", run: () => st().setView({ kind: "settings", section: "apps" }) },
      { id: "data", section: "Go to", icon: Settings, label: "Settings › Backups & data", keywords: "restore import export", run: () => st().setView({ kind: "settings", section: "data" }) },
    )
    for (const n of flatten(buildTree(library.groups))) {
      const path = groupPath(library.groups, n.group.id).join(" › ")
      a.push({ id: `g-${n.group.id}`, section: "Go to", icon: Folder, label: path, keywords: "group folder", run: () => st().setView({ kind: "group", id: n.group.id }) })
      a.push({ id: `gs-${n.group.id}`, section: "Actions", icon: SlidersHorizontal, label: `Group settings: ${path}`, keywords: "rules apps", run: () => onOpenGroupSettings(n.group) })
    }
    return a
  }, [selected, status.paused, settings.theme, settings.clipboardEnabled, library.groups, st, onDelete, onOpenGroupSettings])

  const q = query.trim().toLowerCase()
  const filtered = q
    ? actions.filter((a) => q.split(/\s+/).every((t) => `${a.label} ${a.keywords ?? ""}`.toLowerCase().includes(t)))
    : actions.filter((a) => a.section !== "Go to" || !a.id.startsWith("g-")).filter((a) => !a.id.startsWith("gs-"))
  const sections = ["Selected snippet", "Actions", "Go to"] as const

  return (
    <Dialog open={open} onOpenChange={(o) => (o ? setOpen(true) : close())}>
      <DialogContent showCloseButton={false} className="top-[18%] max-w-[600px] translate-y-0 gap-0 overflow-hidden rounded-2xl bg-popover p-0 shadow-float ring-0 backdrop-blur-2xl sm:max-w-[600px]">
        <DialogTitle className="sr-only">Search and commands</DialogTitle>
        <DialogDescription className="sr-only">Find snippets, jump anywhere, or run an action.</DialogDescription>
        <CommandPrimitive shouldFilter={false} loop className="flex flex-col">
          <div className="flex items-center gap-3 border-b border-hairline px-4">
            <Search className="size-4 text-muted-foreground" />
            <CommandPrimitive.Input
              value={query}
              onValueChange={setQuery}
              placeholder="Search snippets, groups and actions…"
              className="h-12 flex-1 bg-transparent text-[15px] outline-none placeholder:text-faint"
            />
            <Shortcut combo="escape" />
          </div>
          <CommandPrimitive.List className="max-h-[min(440px,60vh)] overflow-y-auto px-2 pb-2">
            <CommandPrimitive.Empty className="py-10 text-center text-sm text-muted-foreground">No results for “{query}”.</CommandPrimitive.Empty>
            {snippetHits.length > 0 && (
              <CommandPrimitive.Group heading={<Heading>{q ? "Snippets" : "Frequently used"}</Heading>}>
                {snippetHits.map((h) => (
                  <Item key={h.snippet.id} value={`snippet-${h.snippet.id}`} onSelect={run(() => {
                    st().setView({ kind: "all" })
                    st().select(h.snippet.id)
                  })}>
                    <FileText />
                    <span className="truncate">{h.snippet.name || "Untitled"}</span>
                    <span className="truncate text-xs text-faint">{groupPath(library.groups, h.snippet.groupID).join(" › ")}</span>
                    {h.snippet.triggers[0] && <TriggerChip trigger={h.snippet.triggers[0]} className="ml-auto" />}
                  </Item>
                ))}
              </CommandPrimitive.Group>
            )}
            {sections.map((sec) => {
              const items = filtered.filter((a) => a.section === sec)
              if (!items.length) return null
              return (
                <CommandPrimitive.Group key={sec} heading={<Heading>{sec}</Heading>}>
                  {items.map((a) => (
                    <Item key={a.id} value={a.id} onSelect={run(a.run)}>
                      <a.icon />
                      <span className={cn("truncate", a.id === "del" && "text-destructive")}>{a.label}</span>
                      {a.shortcut && <Shortcut combo={a.shortcut} className="ml-auto" />}
                    </Item>
                  ))}
                </CommandPrimitive.Group>
              )
            })}
          </CommandPrimitive.List>
        </CommandPrimitive>
      </DialogContent>
    </Dialog>
  )
}
