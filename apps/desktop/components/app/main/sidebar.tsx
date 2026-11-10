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

import {
  ContextMenu,
  ContextMenuContent,
  ContextMenuItem,
  ContextMenuSeparator,
  ContextMenuTrigger,
} from "@/components/ui/context-menu"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { useChanges } from "@/hooks/use-changes"
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
        className="-ml-1 flex h-6 min-w-0 flex-1 items-center gap-1 rounded-md px-1 text-left text-[11px] font-semibold tracking-wide text-faint outline-none transition-colors duration-(--motion-fast) hover:text-muted-foreground focus-visible:ring-2 focus-visible:ring-ring"
      >
        <span className="truncate">{label}</span>
        <ChevronRight className={cn("size-3 shrink-0 opacity-0 transition-[rotate,opacity] duration-(--motion-moderate) ease-spring group-hover/sec:opacity-100 group-focus-within/sec:opacity-100", open && "rotate-90", !open && "opacity-100")} />
      </button>
      {trailing}
    </div>
  )
}

/**
 * Folds its content away by animating height (0fr ↔ 1fr), fading as it goes. The content stays
 * mounted, so nothing replays an entrance when it opens again; folded, it is inert and invisible,
 * which takes it out of the tab order and the accessibility tree. Clipped on the block axis only,
 * so a drop target's scale isn't cut off at the sides.
 */
function Collapse({ open, id, className, children }: { open: boolean; id?: string; className?: string; children: React.ReactNode }) {
  return (
    <div
      id={id}
      inert={!open}
      aria-hidden={!open || undefined}
      className={cn(
        "grid transition-[grid-template-rows,opacity,visibility] duration-(--motion-slow) ease-spring",
        open ? "grid-rows-[1fr]" : "invisible grid-rows-[0fr] opacity-0",
      )}
    >
      <div className={cn("min-h-0 overflow-y-clip", className)}>{children}</div>
    </div>
  )
}

/**
 * While a Collapse is moving, rows below it move every frame. The selection pill follows them
 * through its ResizeObserver; `data-folding` tells it to keep up exactly instead of easing after
 * each step (see NAV_PILL).
 */
function trackFold(e: React.TransitionEvent<HTMLElement>, delta: 1 | -1) {
  if (e.propertyName !== "grid-template-rows") return
  // Counted, because reversing a fold mid-way cancels one transition and starts another in the same frame.
  const folding = Math.max(0, Number(e.currentTarget.dataset.folding ?? 0) + delta)
  if (folding) e.currentTarget.dataset.folding = String(folding)
  else delete e.currentTarget.dataset.folding
}
const foldTracking = {
  onTransitionRun: (e: React.TransitionEvent<HTMLElement>) => trackFold(e, 1),
  onTransitionEnd: (e: React.TransitionEvent<HTMLElement>) => trackFold(e, -1),
  onTransitionCancel: (e: React.TransitionEvent<HTMLElement>) => trackFold(e, -1),
}

/** A row's count. It ticks when the number changes, not when the row mounts. */
function Count({ value }: { value: number }) {
  const changes = useChanges(value)
  if (value <= 0) return null
  return <span key={changes} className={cn("text-[11px] text-faint tabular", changes > 0 && "animate-tick")}>{value}</span>
}

/** Arrow-key navigation between sidebar items. */
function onNavKeyDown(e: React.KeyboardEvent<HTMLElement>) {
  if (!["ArrowDown", "ArrowUp", "Home", "End"].includes(e.key)) return
  // Folded groups and sections stay in the DOM, inert; skip their rows.
  const items = [...e.currentTarget.querySelectorAll<HTMLElement>("[data-nav-item]")].filter((el) => !el.closest("[inert]"))
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
  // dragleave also fires when the pointer crosses onto the icon or label, which made the highlight
  // blink off and on. Counting enters against leaves keeps it steady until the row is really left.
  const dragDepth = useRef(0)
  return (
    <button
      data-nav-item
      aria-current={active ? "page" : undefined}
      onClick={onSelect}
      onDragEnter={
        onDrop
          ? () => {
              dragDepth.current += 1
            }
          : undefined
      }
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
      onDragLeave={
        onDrop
          ? () => {
              dragDepth.current = Math.max(0, dragDepth.current - 1)
              if (dragDepth.current === 0) setOver(false)
            }
          : undefined
      }
      onDrop={
        onDrop
          ? (e) => {
              dragDepth.current = 0
              setOver(false)
              onDrop(e)
            }
          : undefined
      }
      className={cn(
        "group/nav relative flex h-7 w-full items-center gap-2 rounded-md pr-2 text-left text-[13px] press [--press-scale:0.985] outline-none",
        "text-sidebar-foreground/80 hover:text-sidebar-foreground",
        !active && "hover:bg-sidebar-accent",
        // Inset, so a fold's clip edge never cuts the ring of its first or last row.
        "ring-inset focus-visible:ring-2 focus-visible:ring-ring",
        active && "text-sidebar-foreground",
        muted && "text-muted-foreground/70",
        (over || dropActive) && "scale-[1.02] bg-iris-soft text-iris-foreground ring-1 ring-iris/40",
      )}
      style={{ paddingLeft: 8 + depth * 14 }}
      {...rest}
    >
      <Icon className={cn("size-4 shrink-0 text-muted-foreground transition-colors duration-(--motion-fast)", active && "text-iris")} />
      <span className="min-w-0 flex-1 truncate">{label}</span>
      {trailing}
      {count !== undefined && <Count value={count} />}
      {shortcut && <Shortcut combo={shortcut} />}
    </button>
  )
}

function GroupRow({
  node,
  counts,
  collapsed,
  shown,
  toggleCollapsed,
  renaming,
  setRenaming,
  onOpenSettings,
}: {
  node: GroupNode
  counts: Map<string, number>
  collapsed: Set<string>
  /** False while an ancestor or the section is folded: the row is still mounted, but must not claim the selection pill. */
  shown: boolean
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
  const active = shown && view.kind === "group" && view.id === g.id
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
                    className="-mr-1 flex size-5 items-center justify-center rounded text-faint transition-colors duration-(--motion-fast) hover:text-foreground"
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
        <ContextMenuItem
          onSelect={() => {
            // The new row has to be reachable to be renamed; a folded parent would keep it inert.
            if (isCollapsed) toggleCollapsed(g.id)
            void createGroup("New group", g.id).then((ng) => setRenaming(ng.id))
          }}
        >
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

  const snippetsOpen = !closed.has("snippets")
  const clipboardOpen = !closed.has("clipboard")
  const [rootOver, setRootOver] = useState(false)

  const renderNodes = (nodes: GroupNode[], shown: boolean): React.ReactNode =>
    nodes.map((n) => {
      const open = !collapsed.has(n.group.id)
      return (
        <div key={n.group.id} role="none">
          <GroupRow node={n} counts={counts} collapsed={collapsed} shown={shown} toggleCollapsed={toggleCollapsed} renaming={renaming} setRenaming={setRenaming} onOpenSettings={onOpenGroupSettings} />
          {n.children.length > 0 && <Collapse open={open}>{renderNodes(n.children, shown && open)}</Collapse>}
        </div>
      )
    })

  // `shown` is false for rows inside a folded section (see GroupRow).
  const item = (v: View, icon: NavItemIcon, label: string, count?: number, shortcut?: string, shown = true) => (
    <NavItem icon={icon} label={label} count={count} active={shown && sameView(view, v)} onSelect={() => setView(v)} shortcut={shortcut} />
  )

  return (
    <aside aria-label="Sidebar" className="flex h-full w-full flex-col">
      <div className="relative flex h-[72px] shrink-0 items-center px-4">
        <button
          onClick={() => setView({ kind: "all" })}
          aria-label="Kudzu — all snippets"
          className="flex min-w-0 flex-1 rounded-lg p-1 text-foreground/90 press [--press-scale:0.985] hover:text-foreground"
        >
          <Wordmark className="w-full shrink" />
        </button>
        {isDev && (
          // A ribbon pinned across the wordmark's top-right corner, so the wordmark keeps the full width.
          <span className="pointer-events-none absolute top-3.5 right-2.5 rotate-12 rounded-[3px] bg-iris px-2 py-px text-[10px] leading-4 font-semibold tracking-[0.08em] text-iris-contrast uppercase shadow-accent sheen-strong">
            Dev
          </span>
        )}
      </div>
      <nav aria-label="Library" className="relative isolate flex min-h-0 flex-1 flex-col gap-3 overflow-y-auto px-2.5 pb-3" onKeyDown={onNavKeyDown} {...foldTracking}>
        <SelectionPill className={NAV_PILL} />
        <section aria-label="Snippets" className="flex flex-col gap-px">
          <SectionHeader id="sec-snippets" label="Snippets" open={snippetsOpen} onToggle={() => toggleSection("snippets")} />
          <Collapse id="sec-snippets" open={snippetsOpen} className="flex flex-col gap-3">
        <div className="flex flex-col gap-px">
          {item({ kind: "all" }, Layers, "All snippets", library.snippets.length, "mod+1", snippetsOpen)}
          {item({ kind: "favorites" }, Star, "Favorites", favorites, "mod+2", snippetsOpen)}
          {item({ kind: "recent" }, Clock3, "Recently used", recent, "mod+3", snippetsOpen)}
          {disabled > 0 && item({ kind: "disabled" }, CircleOff, "Turned off", disabled, undefined, snippetsOpen)}
        </div>
        <div className="flex flex-col gap-px">
          <div className="group/h flex h-6 items-center justify-between pr-1 pl-2">
            <span
              className={cn("text-[11px] font-medium tracking-wide text-faint transition-colors duration-(--motion-fast)", rootOver && "text-iris-foreground")}
              onDragOver={(e) => {
                e.preventDefault()
                setRootOver(true)
              }}
              onDragLeave={() => setRootOver(false)}
              onDrop={(e) => {
                setRootOver(false)
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
            renderNodes(tree, snippetsOpen)
          ) : (
            <button
              data-nav-item
              onClick={() => createGroup("New group", null).then((g) => setRenaming(g.id))}
              className="mx-1 rounded-md border border-dashed border-border px-2 py-2 text-left text-xs text-muted-foreground press [--press-scale:0.985] outline-none ring-inset hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring"
            >
              Organize snippets into groups — like Work, Email or Code.
            </button>
          )}
        </div>
          </Collapse>
        </section>

        <section aria-label="Clipboard" className="flex flex-col gap-px">
          <SectionHeader
            id="sec-clipboard"
            label="Clipboard"
            open={clipboardOpen}
            onToggle={() => toggleSection("clipboard")}
            trailing={!clipOn ? <span className="mr-1 rounded-full bg-warning/15 px-1.5 text-[10px] font-medium text-warning">Paused</span> : undefined}
          />
          <Collapse id="sec-clipboard" open={clipboardOpen} className="flex flex-col gap-px">
              <NavItem icon={ClipboardList} label="History" count={clipCounts.get("all")} active={clipboardOpen && activeClip === "all"} onSelect={() => setView({ kind: "clipboard" })} shortcut="mod+5" />
              {FILTERS.filter((f) => f.value !== "all" && (f.value === "pinned" || (clipCounts.get(f.value) ?? 0) > 0 || activeClip === f.value)).map((f) => (
                <NavItem
                  key={f.value}
                  icon={CLIP_ICONS[f.value as Exclude<ClipFilter, "all">]}
                  label={f.label}
                  count={clipCounts.get(f.value)}
                  active={clipboardOpen && activeClip === f.value}
                  onSelect={() => setView({ kind: "clipboard", filter: f.value })}
                />
              ))}
          </Collapse>
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

// While a group or section folds, the pill is re-placed every frame; easing each step would leave it trailing its row.
const NAV_PILL = "rounded-md bg-sidebar-accent shadow-selected in-data-[folding]:transition-[opacity]!"

type NavItemIcon = React.ComponentType<{ className?: string }>

