"use client"

import {
  BarChart3,
  AlignLeft,
  Braces,
  ChevronRight,
  CircleOff,
  ClipboardList,
  File,
  Globe,
  Image as ImageIcon,
  Palette,
  Pin,
  Clock3,
  Folder,
  FolderOpen,
  FolderPlus,
  Layers,
  Pencil,
  Play,
  Plus,
  Settings,
  SlidersHorizontal,
  Star,
  Trash2,
} from "lucide-react"
import { useEffect, useMemo, useRef, useState } from "react"

import { Badge } from "@/components/ui/badge"
import {
  ContextMenu,
  ContextMenuContent,
  ContextMenuItem,
  ContextMenuSeparator,
  ContextMenuTrigger,
} from "@/components/ui/context-menu"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { FILTERS, inFilter, useClipboard, type ClipFilter } from "@/lib/clipboard"
import { buildTree, descendantIds, recursiveCounts, type GroupNode } from "@/lib/groups"
import { useStore, type View } from "@/lib/store"
import type { Group } from "@/lib/types"
import { cn } from "@/lib/utils"

import { SelectionPill } from "../selection-pill"
import { Shortcut } from "../shortcut"
import { StatusIndicator } from "./status"
import { Wordmark } from "../wordmark"

export const SNIPPET_DRAG_TYPE = "application/x-kudzu-snippets"
const GROUP_DRAG_TYPE = "application/x-kudzu-group"

function sameView(a: View, b: View) {
  if (a.kind === "clipboard" && b.kind === "clipboard") return (a.filter ?? "all") === (b.filter ?? "all")
  return a.kind === b.kind && (a.kind !== "group" || (b.kind === "group" && a.id === b.id))
}

const CLIP_ICONS: Record<Exclude<ClipFilter, "all">, React.ComponentType<{ className?: string }>> = {
  pinned: Pin,
  text: AlignLeft,
  image: ImageIcon,
  link: Globe,
  file: File,
  code: Braces,
  color: Palette,
  other: AlignLeft,
}

const COLLAPSE_KEY = "kudzu-sidebar-sections"

/** Which feature sections are collapsed, remembered on this Mac. */
function useCollapsedSections() {
  const [closed, setClosed] = useState<Set<string>>(() => {
    try {
      return new Set(JSON.parse(localStorage.getItem(COLLAPSE_KEY) ?? "[]") as string[])
    } catch {
      return new Set()
    }
  })
  const toggle = (id: string) =>
    setClosed((c) => {
      const n = new Set(c)
      if (n.has(id)) n.delete(id)
      else n.add(id)
      try {
        localStorage.setItem(COLLAPSE_KEY, JSON.stringify([...n]))
      } catch {}
      return n
    })
  return [closed, toggle] as const
}

/**
 * A feature's heading (Snippets, Clipboard). Click, Return or ←/→ fold it; the chevron shows on
 * hover and focus, like Finder's sidebar.
 */
function SectionHeader({ id, label, open, onToggle, trailing }: { id: string; label: string; open: boolean; onToggle: () => void; trailing?: React.ReactNode }) {
  return (
    <div className="group/sec flex h-7 items-center gap-1 pr-1 pl-2">
      <button
        data-nav-item
        aria-expanded={open}
        aria-controls={id}
        onClick={onToggle}
        onKeyDown={(e) => {
          if ((e.key === "ArrowLeft" && open) || (e.key === "ArrowRight" && !open)) {
            e.preventDefault()
            onToggle()
          }
        }}
        className="-ml-1 flex h-6 min-w-0 flex-1 items-center gap-1 rounded-md px-1 text-left text-[11px] font-semibold tracking-wide text-faint outline-none transition-colors hover:text-muted-foreground focus-visible:ring-2 focus-visible:ring-ring"
      >
        <span className="truncate">{label}</span>
        <ChevronRight className={cn("size-3 shrink-0 opacity-0 transition-[rotate,opacity] duration-(--motion-moderate) ease-spring group-hover/sec:opacity-100 group-focus-within/sec:opacity-100", open && "rotate-90", !open && "opacity-100")} />
      </button>
      {trailing}
    </div>
  )
}

/** Arrow-key navigation between sidebar items. */
function onNavKeyDown(e: React.KeyboardEvent<HTMLElement>) {
  if (!["ArrowDown", "ArrowUp", "Home", "End"].includes(e.key)) return
  const items = [...e.currentTarget.querySelectorAll<HTMLElement>("[data-nav-item]")]
  const i = items.indexOf(document.activeElement as HTMLElement)
  if (i === -1) return
  e.preventDefault()
  const n = e.key === "Home" ? 0 : e.key === "End" ? items.length - 1 : Math.max(0, Math.min(items.length - 1, i + (e.key === "ArrowDown" ? 1 : -1)))
  items[n].focus()
}

function NavItem({
  icon: Icon,
  label,
  count,
  active,
  onSelect,
  shortcut,
  depth = 0,
  muted,
  trailing,
  onDrop,
  dropActive,
  ...rest
}: {
  icon: React.ComponentType<{ className?: string }>
  label: string
  count?: number
  active: boolean
  onSelect: () => void
  shortcut?: string
  depth?: number
  muted?: boolean
  trailing?: React.ReactNode
  onDrop?: (e: React.DragEvent) => void
  dropActive?: boolean
} & Omit<React.HTMLAttributes<HTMLButtonElement>, "onDrop">) {
  const [over, setOver] = useState(false)
  return (
    <button
      data-nav-item
      aria-current={active ? "page" : undefined}
      onClick={onSelect}
      onDragOver={
        onDrop
          ? (e) => {
              if (![...e.dataTransfer.types].some((t) => t === SNIPPET_DRAG_TYPE || t === GROUP_DRAG_TYPE)) return
              e.preventDefault()
              e.dataTransfer.dropEffect = "move"
              setOver(true)
            }
          : undefined
      }
      onDragLeave={() => setOver(false)}
      onDrop={
        onDrop
          ? (e) => {
              setOver(false)
              onDrop(e)
            }
          : undefined
      }
      className={cn(
        "group/nav relative flex h-7 w-full items-center gap-2 rounded-md pr-2 text-left text-[13px] press [--press-scale:0.985] outline-none",
        "text-sidebar-foreground/80 hover:text-sidebar-foreground",
        !active && "hover:bg-sidebar-accent",
        "focus-visible:ring-2 focus-visible:ring-ring",
        active && "text-sidebar-foreground",
        muted && "text-muted-foreground/70",
        (over || dropActive) && "scale-[1.02] bg-iris-soft text-iris-foreground ring-1 ring-iris/40",
      )}
      style={{ paddingLeft: 8 + depth * 14 }}
      {...rest}
    >
      <Icon className={cn("size-4 shrink-0 text-muted-foreground transition-colors", active && "text-iris")} />
      <span className="min-w-0 flex-1 truncate">{label}</span>
      {trailing}
      {count !== undefined && count > 0 && <span key={count} className="text-[11px] text-faint tabular animate-tick">{count}</span>}
      {shortcut && <Shortcut combo={shortcut} />}
    </button>
  )
}

function GroupRow({
  node,
  counts,
  collapsed,
  toggleCollapsed,
  renaming,
  setRenaming,
  onOpenSettings,
}: {
  node: GroupNode
  counts: Map<string, number>
  collapsed: Set<string>
  toggleCollapsed: (id: string) => void
  renaming: string | null
  setRenaming: (id: string | null) => void
  onOpenSettings: (group: Group) => void
}) {
  const view = useStore((s) => s.view)
  const setView = useStore((s) => s.setView)
  const saveGroup = useStore((s) => s.saveGroup)
  const createGroup = useStore((s) => s.createGroup)
  const deleteGroup = useStore((s) => s.deleteGroup)
  const moveSnippets = useStore((s) => s.moveSnippets)
  const groups = useStore((s) => s.library.groups)
  const g = node.group
  const hasChildren = node.children.length > 0
  const isCollapsed = collapsed.has(g.id)
  const active = view.kind === "group" && view.id === g.id
  if (renaming === g.id) {
    return (
      <RenameRow
        group={g}
        depth={node.depth}
        onDone={(n) => {
          if (n && n !== g.name) void saveGroup({ ...g, name: n })
          setRenaming(null)
        }}
      />
    )
  }

  return (
    <ContextMenu>
      <ContextMenuTrigger asChild>
        <div
          draggable
          onDragStart={(e) => {
            e.dataTransfer.setData(GROUP_DRAG_TYPE, g.id)
            e.dataTransfer.effectAllowed = "move"
          }}
        >
          <NavItem
            icon={isCollapsed || !hasChildren ? Folder : FolderOpen}
            label={g.name}
            count={counts.get(g.id)}
            active={active}
            depth={node.depth}
            muted={!g.isEnabled}
            onSelect={() => setView({ kind: "group", id: g.id })}
            onKeyDown={(e) => {
              if (e.key === "F2" || (e.key === "Enter" && e.altKey)) {
                e.preventDefault()
                setRenaming(g.id)
              } else if (e.key === "ArrowRight" && hasChildren && isCollapsed) {
                e.preventDefault()
                toggleCollapsed(g.id)
              } else if (e.key === "ArrowLeft" && hasChildren && !isCollapsed) {
                e.preventDefault()
                toggleCollapsed(g.id)
              }
            }}
            onDoubleClick={() => setRenaming(g.id)}
            aria-expanded={hasChildren ? !isCollapsed : undefined}
            trailing={
              <>
                {!g.isEnabled && <CircleOff className="size-3 text-faint" aria-label="Off" />}
                {hasChildren && (
                  <span
                    role="presentation"
                    onClick={(e) => {
                      e.stopPropagation()
                      toggleCollapsed(g.id)
                    }}
                    className="-mr-1 flex size-5 items-center justify-center rounded text-faint hover:text-foreground"
                  >
                    <ChevronRight className={cn("size-3 transition-[rotate] duration-(--motion-moderate) ease-spring", !isCollapsed && "rotate-90")} />
                  </span>
                )}
              </>
            }
            onDrop={(e) => {
              const snippets = e.dataTransfer.getData(SNIPPET_DRAG_TYPE)
              const group = e.dataTransfer.getData(GROUP_DRAG_TYPE)
              if (snippets) void moveSnippets(JSON.parse(snippets), g.id)
              else if (group && !descendantIds(groups, group).has(g.id)) {
                const moved = groups.find((x) => x.id === group)
                if (moved) void saveGroup({ ...moved, parentID: g.id })
              }
            }}
          />
        </div>
      </ContextMenuTrigger>
      <ContextMenuContent className="w-52">
        <ContextMenuItem onSelect={() => setRenaming(g.id)}>
          <Pencil /> Rename
        </ContextMenuItem>
        <ContextMenuItem onSelect={() => createGroup("New group", g.id).then((ng) => setRenaming(ng.id))}>
          <FolderPlus /> New subgroup
        </ContextMenuItem>
        <ContextMenuItem onSelect={() => onOpenSettings(g)}>
          <SlidersHorizontal /> Group settings…
        </ContextMenuItem>
        <ContextMenuItem onSelect={() => saveGroup({ ...g, isEnabled: !g.isEnabled })}>
          {g.isEnabled ? <CircleOff /> : <Play />} {g.isEnabled ? "Turn off group" : "Turn on group"}
        </ContextMenuItem>
        {g.parentID && (
          <ContextMenuItem onSelect={() => saveGroup({ ...g, parentID: undefined })}>
            <Layers /> Move to top level
          </ContextMenuItem>
        )}
        <ContextMenuSeparator />
        <ContextMenuItem variant="destructive" onSelect={() => deleteGroup(g.id, false)}>
          <Trash2 /> Delete group, keep snippets
        </ContextMenuItem>
        <ContextMenuItem variant="destructive" onSelect={() => deleteGroup(g.id, true)}>
          <Trash2 /> Delete group and snippets
        </ContextMenuItem>
      </ContextMenuContent>
    </ContextMenu>
  )
}

function RenameRow({ group, depth, onDone }: { group: Group; depth: number; onDone: (name: string | null) => void }) {
  const [name, setName] = useState(group.name)
  const input = useRef<HTMLInputElement>(null)
  useEffect(() => input.current?.select(), [])
  return (
    <div className="flex h-7 items-center gap-2 rounded-md bg-sidebar-accent pr-1" style={{ paddingLeft: 8 + depth * 14 }}>
      <Folder className="size-4 shrink-0 text-iris" />
      <input
        ref={input}
        value={name}
        aria-label="Group name"
        onChange={(e) => setName(e.target.value)}
        onBlur={() => onDone(name.trim())}
        onKeyDown={(e) => {
          if (e.key === "Enter") onDone(name.trim())
          if (e.key === "Escape") onDone(null)
          e.stopPropagation()
        }}
        className="h-6 min-w-0 flex-1 rounded bg-transparent text-[13px] outline-none"
      />
    </div>
  )
}

export function Sidebar({ onOpenGroupSettings, isDev }: { onOpenGroupSettings: (g: Group) => void; isDev: boolean }) {
  const library = useStore((s) => s.library)
  const usage = useStore((s) => s.usage)
  const view = useStore((s) => s.view)
  const setView = useStore((s) => s.setView)
  const createGroup = useStore((s) => s.createGroup)
  const saveGroup = useStore((s) => s.saveGroup)
  const moveSnippets = useStore((s) => s.moveSnippets)
  const [collapsed, setCollapsed] = useState<Set<string>>(new Set())
  const [renaming, setRenaming] = useState<string | null>(null)

  const tree = useMemo(() => buildTree(library.groups), [library.groups])
  const counts = useMemo(() => recursiveCounts(library.groups, library.snippets), [library.groups, library.snippets])
  const favorites = library.snippets.filter((s) => s.isFavorite).length
  const disabled = library.snippets.filter((s) => !s.isEnabled).length
  const recent = useMemo(() => [...usage.values()].filter((u) => u.last).length, [usage])
  const clipEntries = useClipboard((s) => s.entries)
  const clipOn = useStore((s) => s.settings.clipboardEnabled)
  const clipCounts = useMemo(() => {
    const m = new Map<ClipFilter, number>()
    for (const f of FILTERS) m.set(f.value, f.value === "all" ? clipEntries.length : clipEntries.filter((e) => inFilter(e, f.value)).length)
    return m
  }, [clipEntries])
  const [closed, toggleSection] = useCollapsedSections()
  const activeClip = view.kind === "clipboard" ? (view.filter ?? "all") : null

  const toggleCollapsed = (id: string) =>
    setCollapsed((c) => {
      const n = new Set(c)
      if (n.has(id)) n.delete(id)
      else n.add(id)
      return n
    })

  const renderNodes = (nodes: GroupNode[]): React.ReactNode =>
    nodes.map((n) => (
      <div key={n.group.id} role="none">
        <GroupRow node={n} counts={counts} collapsed={collapsed} toggleCollapsed={toggleCollapsed} renaming={renaming} setRenaming={setRenaming} onOpenSettings={onOpenGroupSettings} />
        {!collapsed.has(n.group.id) && renderNodes(n.children)}
      </div>
    ))

  const item = (v: View, icon: NavItemIcon, label: string, count?: number, shortcut?: string) => (
    <NavItem icon={icon} label={label} count={count} active={sameView(view, v)} onSelect={() => setView(v)} shortcut={shortcut} />
  )

  return (
    <aside aria-label="Sidebar" className="flex h-full w-full flex-col">
      <div className="flex h-[72px] shrink-0 items-center gap-2 px-4">
        <button
          onClick={() => setView({ kind: "all" })}
          aria-label="Kudzu — all snippets"
          className="flex min-w-0 items-center gap-2.5 rounded-lg p-1 text-foreground/90 transition-colors hover:text-foreground"
        >
          {/* eslint-disable-next-line @next/next/no-img-element -- bundled app artwork */}
          <img src="/icon.png" alt="" width={36} height={36} draggable={false} />
          <Wordmark variant="small" className="w-[76px]" />
        </button>
        {isDev && <Badge className="ml-auto">Dev</Badge>}
      </div>
      <nav aria-label="Library" className="relative isolate flex min-h-0 flex-1 flex-col gap-3 overflow-y-auto px-2.5 pb-3" onKeyDown={onNavKeyDown}>
        <SelectionPill className={NAV_PILL} />
        <section aria-label="Snippets" className="flex flex-col gap-px">
          <SectionHeader id="sec-snippets" label="Snippets" open={!closed.has("snippets")} onToggle={() => toggleSection("snippets")} />
          {!closed.has("snippets") && (
            <div id="sec-snippets" className="flex flex-col gap-3 animate-rise">
        <div className="flex flex-col gap-px">
          {item({ kind: "all" }, Layers, "All snippets", library.snippets.length, "mod+1")}
          {item({ kind: "favorites" }, Star, "Favorites", favorites, "mod+2")}
          {item({ kind: "recent" }, Clock3, "Recently used", recent, "mod+3")}
          {disabled > 0 && item({ kind: "disabled" }, CircleOff, "Turned off", disabled)}
        </div>
        <div className="flex flex-col gap-px">
          <div className="group/h flex h-6 items-center justify-between pr-1 pl-2">
            <span
              className="text-[11px] font-medium tracking-wide text-faint"
              onDragOver={(e) => e.preventDefault()}
              onDrop={(e) => {
                const group = e.dataTransfer.getData(GROUP_DRAG_TYPE)
                const moved = library.groups.find((x) => x.id === group)
                if (moved) void saveGroup({ ...moved, parentID: undefined })
                const snippets = e.dataTransfer.getData(SNIPPET_DRAG_TYPE)
                if (snippets) void moveSnippets(JSON.parse(snippets), null)
              }}
            >
              Groups
            </span>
            <Tooltip>
              <TooltipTrigger asChild>
                <button
                  data-nav-item
                  onClick={() => createGroup("New group", null).then((g) => setRenaming(g.id))}
                  className="flex size-5 items-center justify-center rounded text-faint opacity-60 press hover:bg-sidebar-accent hover:text-foreground hover:opacity-100 focus-visible:opacity-100 [&:hover>svg]:rotate-90"
                  aria-label="New group"
                >
                  <Plus className="size-3.5 transition-[rotate] duration-(--motion-slow) ease-pop" />
                </button>
              </TooltipTrigger>
              <TooltipContent side="right">
                New group <Shortcut combo="mod+shift+n" />
              </TooltipContent>
            </Tooltip>
          </div>
          {tree.length ? (
            renderNodes(tree)
          ) : (
            <button
              data-nav-item
              onClick={() => createGroup("New group", null).then((g) => setRenaming(g.id))}
              className="mx-1 rounded-md border border-dashed border-border px-2 py-2 text-left text-xs text-muted-foreground hover:text-foreground"
            >
              Organize snippets into groups — like Work, Email or Code.
            </button>
          )}
        </div>
            </div>
          )}
        </section>

        <section aria-label="Clipboard" className="flex flex-col gap-px">
          <SectionHeader
            id="sec-clipboard"
            label="Clipboard"
            open={!closed.has("clipboard")}
            onToggle={() => toggleSection("clipboard")}
            trailing={!clipOn ? <span className="mr-1 rounded-full bg-warning/15 px-1.5 text-[10px] font-medium text-warning">Paused</span> : undefined}
          />
          {!closed.has("clipboard") && (
            <div id="sec-clipboard" className="flex flex-col gap-px animate-rise">
              <NavItem icon={ClipboardList} label="History" count={clipCounts.get("all")} active={activeClip === "all"} onSelect={() => setView({ kind: "clipboard" })} shortcut="mod+5" />
              {FILTERS.filter((f) => f.value !== "all" && (f.value === "pinned" || (clipCounts.get(f.value) ?? 0) > 0 || activeClip === f.value)).map((f) => (
                <NavItem
                  key={f.value}
                  icon={CLIP_ICONS[f.value as Exclude<ClipFilter, "all">]}
                  label={f.label}
                  count={clipCounts.get(f.value)}
                  active={activeClip === f.value}
                  onSelect={() => setView({ kind: "clipboard", filter: f.value })}
                />
              ))}
            </div>
          )}
        </section>
      </nav>
      <div className="relative isolate flex flex-col gap-px border-t border-hairline px-2.5 py-2" onKeyDown={onNavKeyDown}>
        <SelectionPill className={NAV_PILL} />
        {item({ kind: "insights" }, BarChart3, "Insights", undefined, "mod+4")}
        <NavItem icon={Settings} label="Settings" active={view.kind === "settings"} onSelect={() => setView({ kind: "settings" })} shortcut="mod+," />
      </div>
      <StatusIndicator />
    </aside>
  )
}

const NAV_PILL = "rounded-md bg-sidebar-accent shadow-selected"

type NavItemIcon = React.ComponentType<{ className?: string }>

