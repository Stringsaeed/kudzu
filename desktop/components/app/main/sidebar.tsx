"use client"

import {
  BarChart3,
  ChevronRight,
  CircleOff,
  Clock3,
  Folder,
  FolderOpen,
  FolderPlus,
  Layers,
  Pencil,
  Play,
  Plus,
  Search,
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
import { buildTree, descendantIds, recursiveCounts, type GroupNode } from "@/lib/groups"
import { useStore, type View } from "@/lib/store"
import type { Group } from "@/lib/types"
import { cn } from "@/lib/utils"

import { Shortcut } from "../shortcut"
import { StatusIndicator } from "./status"

export const SNIPPET_DRAG_TYPE = "application/x-kudzu-snippets"
const GROUP_DRAG_TYPE = "application/x-kudzu-group"

function sameView(a: View, b: View) {
  return a.kind === b.kind && (a.kind !== "group" || (b.kind === "group" && a.id === b.id))
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
        "group/nav relative flex h-7 w-full items-center gap-2 rounded-md pr-2 text-left text-[13px] transition-colors duration-100 outline-none",
        "text-sidebar-foreground/80 hover:bg-sidebar-accent hover:text-sidebar-foreground",
        "focus-visible:ring-2 focus-visible:ring-ring",
        active && "bg-sidebar-accent text-sidebar-foreground shadow-selected",
        muted && "text-muted-foreground/70",
        (over || dropActive) && "bg-iris-soft text-iris-foreground ring-1 ring-iris/40",
      )}
      style={{ paddingLeft: 8 + depth * 14 }}
      {...rest}
    >
      <Icon className={cn("size-4 shrink-0 text-muted-foreground transition-colors", active && "text-iris")} />
      <span className="min-w-0 flex-1 truncate">{label}</span>
      {trailing}
      {count !== undefined && count > 0 && <span className="text-[11px] text-faint tabular">{count}</span>}
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
                    <ChevronRight className={cn("size-3 transition-transform", !isCollapsed && "rotate-90")} />
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
  const setCommandOpen = useStore((s) => s.setCommandOpen)
  const saveGroup = useStore((s) => s.saveGroup)
  const moveSnippets = useStore((s) => s.moveSnippets)
  const [collapsed, setCollapsed] = useState<Set<string>>(new Set())
  const [renaming, setRenaming] = useState<string | null>(null)

  const tree = useMemo(() => buildTree(library.groups), [library.groups])
  const counts = useMemo(() => recursiveCounts(library.groups, library.snippets), [library.groups, library.snippets])
  const favorites = library.snippets.filter((s) => s.isFavorite).length
  const disabled = library.snippets.filter((s) => !s.isEnabled).length
  const recent = useMemo(() => [...usage.values()].filter((u) => u.last).length, [usage])

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
    <aside aria-label="Sidebar" className="flex h-full w-full flex-col bg-sidebar">
      {/* Space for the traffic lights + drag */}
      <div className="drag flex h-12 shrink-0 items-center justify-end px-3" data-tauri-drag-region>
        {isDev && <Badge>Dev</Badge>}
      </div>
      <div className="px-2.5 pb-2">
        <button
          onClick={() => setCommandOpen(true)}
          className="flex h-8 w-full items-center gap-2 rounded-lg bg-foreground/[0.03] px-2.5 text-[13px] text-muted-foreground shadow-well transition-colors hover:bg-foreground/[0.06] hover:text-foreground"
        >
          <Search className="size-3.5" />
          <span className="flex-1 truncate text-left whitespace-nowrap">Search…</span>
          <Shortcut combo="mod+k" />
        </button>
      </div>
      <nav aria-label="Library" className="flex min-h-0 flex-1 flex-col gap-4 overflow-y-auto px-2.5 pb-3" onKeyDown={onNavKeyDown}>
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
                  className="flex size-5 items-center justify-center rounded text-faint opacity-60 transition hover:bg-sidebar-accent hover:text-foreground hover:opacity-100 focus-visible:opacity-100"
                  aria-label="New group"
                >
                  <Plus className="size-3.5" />
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
      </nav>
      <div className="flex flex-col gap-px border-t border-hairline px-2.5 py-2" onKeyDown={onNavKeyDown}>
        {item({ kind: "insights" }, BarChart3, "Insights", undefined, "mod+4")}
        <NavItem icon={Settings} label="Settings" active={view.kind === "settings"} onSelect={() => setView({ kind: "settings" })} shortcut="mod+," />
      </div>
      <StatusIndicator />
    </aside>
  )
}

type NavItemIcon = React.ComponentType<{ className?: string }>

