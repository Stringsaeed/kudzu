"use client"

import { useVirtualizer } from "@tanstack/react-virtual"
import { ArrowDownUp, CircleOff, FolderInput, Plus, Search, Star, Trash2, X } from "lucide-react"
import { forwardRef, memo, useCallback, useEffect, useImperativeHandle, useMemo, useRef, useState } from "react"

import { Button } from "@/components/ui/button"
import { Item, ItemContent, ItemDescription, ItemTitle } from "@/components/ui/item"
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { buildTree, descendantIds, flatten, isGroupEnabled } from "@/lib/groups"
import { buildIndex, search, type SearchHit } from "@/lib/search"
import { formatCount } from "@/lib/stats"
import { useStore, type View } from "@/lib/store"
import { plainSummary } from "@/lib/template"
import type { Snippet } from "@/lib/types"
import { cn } from "@/lib/utils"

import { Shortcut } from "../shortcut"
import { SNIPPET_DRAG_TYPE } from "./sidebar"

type Sort = "relevance" | "name" | "edited" | "used"

const ROW_HEIGHT = 64

export interface SnippetListHandle {
  focusFilter: () => void
  focusList: () => void
  /** Ids currently selected (multi-select aware). */
  selection: () => string[]
}

function viewTitle(view: View, groupName?: string): string {
  switch (view.kind) {
    case "all":
      return "All snippets"
    case "favorites":
      return "Favorites"
    case "recent":
      return "Recently used"
    case "disabled":
      return "Turned off"
    case "group":
      return groupName ?? "Group"
    default:
      return ""
  }
}

function Highlighted({ text, hits }: { text: string; hits: number[] }) {
  if (!hits.length) return <>{text}</>
  const set = new Set(hits)
  const out: React.ReactNode[] = []
  let run = ""
  let runHit = false
  const push = (i: number) => {
    if (run) out.push(runHit ? <mark key={i} className="rounded-[2px] bg-transparent text-iris-foreground underline decoration-iris/50 underline-offset-2">{run}</mark> : run)
  }
  for (let i = 0; i < text.length; i++) {
    const h = set.has(i)
    if (h !== runHit) {
      push(i)
      run = ""
      runHit = h
    }
    run += text[i]
  }
  push(text.length)
  return <>{out}</>
}

const Row = memo(function Row({
  hit,
  selected,
  inSelection,
  active,
  usage,
  summary,
  groupOff,
  onClick,
  onDragStart,
}: {
  hit: SearchHit
  selected: boolean
  inSelection: boolean
  active: boolean
  usage: number
  summary: string
  groupOff: boolean
  onClick: (e: React.MouseEvent, id: string) => void
  onDragStart: (e: React.DragEvent, id: string) => void
}) {
  const s = hit.snippet
  const off = !s.isEnabled || groupOff
  return (
    <Item
      id={`snippet-${s.id}`}
      role="option"
      aria-selected={inSelection}
      draggable
      onDragStart={(e) => onDragStart(e, s.id)}
      onMouseDown={(e) => onClick(e, s.id)}
      className={cn(
        "group/row relative mx-2 h-[60px] w-auto cursor-default flex-nowrap px-3 py-0 duration-75",
        "hover:bg-foreground/[0.035]",
        inSelection && "bg-foreground/[0.06] shadow-selected hover:bg-foreground/[0.07]",
        selected && active && "bg-iris-soft hover:bg-iris-soft",
        selected && active && "after:absolute after:inset-0 after:rounded-lg after:ring-1 after:ring-iris/35 after:ring-inset",
      )}
    >
      <ItemContent className="min-w-0 gap-1">
        <div className="flex min-w-0 items-center gap-2">
          <ItemTitle className={cn("block min-w-0 flex-1 truncate text-[13px]", off && "text-muted-foreground", !s.name && "text-muted-foreground italic")}>
            <Highlighted text={s.name || "Untitled"} hits={s.name ? hit.nameHits : []} />
          </ItemTitle>
          {s.isFavorite && <Star className="size-3 shrink-0 fill-favorite text-favorite" aria-label="Favorite" />}
          {off && <CircleOff className="size-3 shrink-0 text-faint" aria-label="Turned off" />}
          {s.triggers[0] && (
            <span className={cn("max-w-[45%] shrink-0 truncate font-mono text-[11.5px]", off ? "text-faint" : "text-iris-foreground/90")}>
              {s.triggers[0]}
              {s.triggers.length > 1 && <span className="text-faint"> +{s.triggers.length - 1}</span>}
            </span>
          )}
        </div>
        <div className="flex min-w-0 items-center gap-2">
          <ItemDescription className="min-w-0 flex-1 truncate text-xs text-muted-foreground/80">{summary || <span className="italic">Empty</span>}</ItemDescription>
          {usage > 0 && <span className="shrink-0 text-[10.5px] text-faint tabular">{formatCount(usage)}×</span>}
        </div>
      </ItemContent>
    </Item>
  )
})

export const SnippetList = forwardRef<SnippetListHandle, { onRequestDelete: (ids: string[]) => void; onOpen: () => void }>(function SnippetList(
  { onRequestDelete, onOpen },
  ref,
) {
  const library = useStore((s) => s.library)
  const usage = useStore((s) => s.usage)
  const view = useStore((s) => s.view)
  const selectedId = useStore((s) => s.selectedId)
  const select = useStore((s) => s.select)
  const filter = useStore((s) => s.filter)
  const setFilter = useStore((s) => s.setFilter)
  const newSnippet = useStore((s) => s.newSnippet)
  const moveSnippets = useStore((s) => s.moveSnippets)
  const saveSnippet = useStore((s) => s.saveSnippet)
  const [sort, setSort] = useState<Sort>("relevance")
  const [multi, setMulti] = useState<Set<string>>(new Set())
  const [focused, setFocused] = useState(false)
  const anchor = useRef<string | null>(null)
  const filterRef = useRef<HTMLInputElement>(null)
  const listRef = useRef<HTMLDivElement>(null)

  // Scope snippets to the view.
  const scoped = useMemo(() => {
    const s = library.snippets
    switch (view.kind) {
      case "favorites":
        return s.filter((x) => x.isFavorite)
      case "recent":
        return s.filter((x) => usage.get(x.id)?.last)
      case "disabled":
        return s.filter((x) => !x.isEnabled)
      case "group": {
        const ids = descendantIds(library.groups, view.id)
        return s.filter((x) => x.groupID && ids.has(x.groupID))
      }
      default:
        return s
    }
  }, [library, view, usage])

  const index = useMemo(() => buildIndex(scoped, usage), [scoped, usage])
  const hits = useMemo(() => {
    const r = search(index, filter, 10_000)
    const q = filter.trim()
    const by = (cmp: (a: Snippet, b: Snippet) => number) => r.sort((a, b) => cmp(a.snippet, b.snippet))
    if (view.kind === "recent" && (!q || sort === "relevance")) return by((a, b) => (usage.get(b.id)?.last ?? 0) - (usage.get(a.id)?.last ?? 0))
    if (sort === "name" || (!q && sort === "relevance")) return by((a, b) => (a.name || "￿").localeCompare(b.name || "￿", undefined, { sensitivity: "base" }))
    if (sort === "edited") return by((a, b) => b.updatedAt.localeCompare(a.updatedAt))
    if (sort === "used") return by((a, b) => (usage.get(b.id)?.count ?? 0) - (usage.get(a.id)?.count ?? 0))
    return r
  }, [index, filter, sort, view.kind, usage])

  // Row summaries are computed lazily per visible row and cached by content.
  const names = useMemo(() => new Map(library.snippets.map((s) => [s.id, s.name])), [library.snippets])
  const summaryCache = useRef(new Map<string, string>())
  useEffect(() => summaryCache.current.clear(), [names])
  const summaryOf = (s: Snippet) => {
    let v = summaryCache.current.get(s.content)
    if (v === undefined) {
      v = plainSummary(s.content, (id) => names.get(id))
      summaryCache.current.set(s.content, v)
    }
    return v
  }

  const groupOff = useMemo(() => {
    const m = new Map<string, boolean>()
    for (const g of library.groups) m.set(g.id, !isGroupEnabled(library.groups, g.id))
    return m
  }, [library.groups])

  const ids = useMemo(() => hits.map((h) => h.snippet.id), [hits])
  const selectedIndex = selectedId ? ids.indexOf(selectedId) : -1

  // Keep a valid selection inside the visible list.
  useEffect(() => {
    if (selectedIndex === -1 && ids.length && (!selectedId || !filter)) {
      const keep = selectedId && library.snippets.some((s) => s.id === selectedId) && !filter && view.kind === "all"
      if (!keep) select(ids[0])
    }
  }, [ids, selectedIndex, selectedId, select, filter, view.kind, library.snippets])

  useEffect(() => setMulti(new Set()), [view])

  const virtualizer = useVirtualizer({
    count: hits.length,
    getScrollElement: () => listRef.current,
    estimateSize: () => ROW_HEIGHT,
    overscan: 8,
    paddingStart: 4,
    paddingEnd: 8,
  })

  useEffect(() => {
    if (selectedIndex >= 0) virtualizer.scrollToIndex(selectedIndex, { align: "auto" })
  }, [selectedIndex, virtualizer])

  const selection = useCallback(() => (multi.size > 1 ? [...multi] : selectedId ? [selectedId] : []), [multi, selectedId])

  useImperativeHandle(ref, () => ({
    focusFilter: () => filterRef.current?.focus(),
    focusList: () => listRef.current?.focus(),
    selection,
  }))

  const move = (delta: number, extend: boolean) => {
    if (!ids.length) return
    const n = Math.max(0, Math.min(ids.length - 1, (selectedIndex === -1 ? (delta > 0 ? -1 : ids.length) : selectedIndex) + delta))
    const id = ids[n]
    select(id)
    if (extend) {
      const a = anchor.current ?? selectedId ?? id
      anchor.current = a
      const ai = ids.indexOf(a)
      const [lo, hi] = ai < n ? [ai, n] : [n, ai]
      setMulti(new Set(ids.slice(lo, hi + 1)))
    } else {
      anchor.current = id
      setMulti(new Set())
    }
  }

  const onRowClick = useCallback(
    (e: React.MouseEvent, id: string) => {
      if (e.button !== 0) return
      if (e.metaKey || e.ctrlKey) {
        setMulti((m) => {
          const n = new Set(m.size ? m : selectedId ? [selectedId] : [])
          if (n.has(id)) n.delete(id)
          else n.add(id)
          return n
        })
        select(id)
      } else if (e.shiftKey && anchor.current) {
        const a = ids.indexOf(anchor.current)
        const b = ids.indexOf(id)
        const [lo, hi] = a < b ? [a, b] : [b, a]
        setMulti(new Set(ids.slice(lo, hi + 1)))
        select(id)
      } else {
        anchor.current = id
        setMulti(new Set())
        select(id)
      }
    },
    [ids, select, selectedId],
  )

  const onDragStart = useCallback(
    (e: React.DragEvent, id: string) => {
      const dragged = multi.has(id) ? [...multi] : [id]
      e.dataTransfer.setData(SNIPPET_DRAG_TYPE, JSON.stringify(dragged))
      e.dataTransfer.effectAllowed = "move"
      const ghost = document.createElement("div")
      ghost.textContent = dragged.length > 1 ? `${dragged.length} snippets` : (library.snippets.find((s) => s.id === id)?.name ?? "Snippet")
      ghost.className = "fixed -top-20 rounded-md bg-popover px-2 py-1 text-xs text-popover-foreground shadow"
      document.body.appendChild(ghost)
      e.dataTransfer.setDragImage(ghost, 10, 10)
      setTimeout(() => ghost.remove())
    },
    [multi, library.snippets],
  )

  const groupName = view.kind === "group" ? library.groups.find((g) => g.id === view.id)?.name : undefined
  const flatGroups = useMemo(() => flatten(buildTree(library.groups)), [library.groups])
  const bulk = multi.size > 1 ? [...multi] : []

  return (
    <section aria-label="Snippets" className="flex h-full min-h-0 flex-col">
      <header className="drag flex h-12 shrink-0 items-center gap-2 pr-2 pl-5" data-tauri-drag-region>
        <h2 className="min-w-0 flex-1 truncate text-[13px] font-semibold">
          {viewTitle(view, groupName)} <span className="ml-1 font-normal text-faint tabular">{hits.length}</span>
        </h2>
        <DropdownMenu>
          <Tooltip>
            <TooltipTrigger asChild>
              <DropdownMenuTrigger asChild>
                <Button variant="ghost" size="icon-sm" className="no-drag text-muted-foreground" aria-label="Sort">
                  <ArrowDownUp />
                </Button>
              </DropdownMenuTrigger>
            </TooltipTrigger>
            <TooltipContent>Sort</TooltipContent>
          </Tooltip>
          <DropdownMenuContent align="end" className="w-44">
            <DropdownMenuLabel>Sort by</DropdownMenuLabel>
            <DropdownMenuRadioGroup value={sort} onValueChange={(v) => setSort(v as Sort)}>
              <DropdownMenuRadioItem value="relevance">Best match</DropdownMenuRadioItem>
              <DropdownMenuRadioItem value="name">Name</DropdownMenuRadioItem>
              <DropdownMenuRadioItem value="edited">Recently edited</DropdownMenuRadioItem>
              <DropdownMenuRadioItem value="used">Most used</DropdownMenuRadioItem>
            </DropdownMenuRadioGroup>
          </DropdownMenuContent>
        </DropdownMenu>
        <Tooltip>
          <TooltipTrigger asChild>
            <Button variant="ghost" size="icon-sm" className="no-drag text-muted-foreground" onClick={() => newSnippet()} aria-label="New snippet">
              <Plus className="transition-[rotate] duration-(--motion-slow) ease-pop group-hover/button:rotate-90" />
            </Button>
          </TooltipTrigger>
          <TooltipContent>
            New snippet <Shortcut combo="mod+n" />
          </TooltipContent>
        </Tooltip>
      </header>

      <div className="px-3 pb-2">
        <div className="group relative flex items-center">
          <Search className="pointer-events-none absolute left-2.5 size-3.5 text-faint" />
          <input
            ref={filterRef}
            value={filter}
            onChange={(e) => setFilter(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "ArrowDown" || e.key === "ArrowUp") {
                e.preventDefault()
                move(e.key === "ArrowDown" ? 1 : -1, e.shiftKey)
              } else if (e.key === "Enter") {
                e.preventDefault()
                onOpen()
              } else if (e.key === "Escape") {
                if (filter) {
                  e.preventDefault()
                  e.stopPropagation()
                  setFilter("")
                } else listRef.current?.focus()
              }
            }}
            placeholder="Filter by name, trigger or text"
            aria-label="Filter snippets"
            aria-controls="snippet-listbox"
            spellCheck={false}
            className="h-8 w-full rounded-lg bg-foreground/[0.03] pr-14 pl-8 text-[13px] shadow-well outline-none placeholder:text-faint focus:bg-transparent focus:shadow-none focus:ring-2 focus:ring-ring"
          />
          {filter ? (
            <button className="absolute right-2 flex size-5 items-center justify-center rounded text-faint hover:text-foreground" onClick={() => setFilter("")} aria-label="Clear filter">
              <X className="size-3.5" />
            </button>
          ) : (
            <Shortcut combo="mod+f" className="pointer-events-none absolute right-2" />
          )}
        </div>
      </div>

      <div
        ref={listRef}
        id="snippet-listbox"
        role="listbox"
        tabIndex={0}
        aria-label={`${viewTitle(view, groupName)} snippets`}
        aria-multiselectable
        aria-activedescendant={selectedId && selectedIndex >= 0 ? `snippet-${selectedId}` : undefined}
        onFocus={() => setFocused(true)}
        onBlur={() => setFocused(false)}
        onKeyDown={(e) => {
          const k = e.key
          if (k === "ArrowDown" || k === "ArrowUp" || (!e.metaKey && !e.ctrlKey && (k === "j" || k === "k"))) {
            e.preventDefault()
            move(k === "ArrowDown" || k === "j" ? 1 : -1, e.shiftKey)
          } else if (k === "Home" || k === "End") {
            e.preventDefault()
            move(k === "Home" ? -ids.length : ids.length, e.shiftKey)
          } else if (k === "PageDown" || k === "PageUp") {
            e.preventDefault()
            move(k === "PageDown" ? 8 : -8, e.shiftKey)
          } else if (k === "Enter" || k === "ArrowRight") {
            e.preventDefault()
            onOpen()
          } else if ((e.metaKey || e.ctrlKey) && k === "a") {
            e.preventDefault()
            setMulti(new Set(ids))
          } else if (k === "Escape" && multi.size) {
            e.preventDefault()
            setMulti(new Set())
          } else if (k === "/" ) {
            e.preventDefault()
            filterRef.current?.focus()
          } else if (k.length === 1 && !e.metaKey && !e.ctrlKey && !e.altKey && /\S/.test(k)) {
            // Type-to-filter.
            filterRef.current?.focus()
          }
        }}
        className="relative min-h-0 flex-1 overflow-y-auto outline-none"
      >
        {hits.length === 0 ? (
          <EmptyList view={view} filter={filter} onClear={() => setFilter("")} onNew={() => newSnippet(filter.startsWith(";") ? { triggers: [filter.trim()] } : { name: filter.trim() })} />
        ) : (
          <div style={{ height: virtualizer.getTotalSize(), position: "relative" }}>
            {virtualizer.getVirtualItems().map((v) => {
              const hit = hits[v.index]
              const id = hit.snippet.id
              return (
                <div key={id} style={{ position: "absolute", top: 0, left: 0, right: 0, height: v.size, transform: `translateY(${v.start}px)` }} className="flex items-center">
                  <div className="w-full">
                    <Row
                      hit={hit}
                      selected={id === selectedId}
                      inSelection={multi.size > 1 ? multi.has(id) : id === selectedId}
                      active={focused || multi.size <= 1}
                      usage={usage.get(id)?.count ?? 0}
                      summary={summaryOf(hit.snippet)}
                      groupOff={!!(hit.snippet.groupID && groupOff.get(hit.snippet.groupID))}
                      onClick={onRowClick}
                      onDragStart={onDragStart}
                    />
                  </div>
                </div>
              )
            })}
          </div>
        )}
      </div>

      {bulk.length > 0 && (
        <div role="toolbar" aria-label="Selection actions" className="m-2 flex items-center gap-1 rounded-xl bg-surface-raised p-1.5 pl-3 shadow-float animate-rise">
          <span className="flex-1 text-xs font-medium tabular">{bulk.length} selected</span>
          <Tooltip>
            <TooltipTrigger asChild>
              <Button variant="ghost" size="icon-sm" aria-label="Favorite all" onClick={() => bulk.forEach((id) => { const s = library.snippets.find((x) => x.id === id); if (s && !s.isFavorite) void saveSnippet({ ...s, isFavorite: true }) })}>
                <Star />
              </Button>
            </TooltipTrigger>
            <TooltipContent>Add to favorites</TooltipContent>
          </Tooltip>
          <Tooltip>
            <TooltipTrigger asChild>
              <Button variant="ghost" size="icon-sm" aria-label="Turn off all" onClick={() => bulk.forEach((id) => { const s = library.snippets.find((x) => x.id === id); if (s) void saveSnippet({ ...s, isEnabled: !s.isEnabled }) })}>
                <CircleOff />
              </Button>
            </TooltipTrigger>
            <TooltipContent>Toggle on/off</TooltipContent>
          </Tooltip>
          <DropdownMenu>
            <DropdownMenuTrigger asChild>
              <Button variant="ghost" size="icon-sm" aria-label="Move to group">
                <FolderInput />
              </Button>
            </DropdownMenuTrigger>
            <DropdownMenuContent align="end" side="top" className="w-52">
              <DropdownMenuLabel>Move {bulk.length} to</DropdownMenuLabel>
              <DropdownMenuItem onSelect={() => moveSnippets(bulk, null)}>No group</DropdownMenuItem>
              {flatGroups.map((n) => (
                <DropdownMenuItem key={n.group.id} onSelect={() => moveSnippets(bulk, n.group.id)} style={{ paddingLeft: 8 + n.depth * 14 }}>
                  {n.group.name}
                </DropdownMenuItem>
              ))}
            </DropdownMenuContent>
          </DropdownMenu>
          <Button variant="ghost" size="icon-sm" className="hover:text-destructive" aria-label="Delete selected" onClick={() => (onRequestDelete(bulk), setMulti(new Set()))}>
            <Trash2 />
          </Button>
        </div>
      )}
    </section>
  )
})

function EmptyList({ view, filter, onClear, onNew }: { view: View; filter: string; onClear: () => void; onNew: () => void }) {
  if (filter)
    return (
      <div className="flex flex-col items-center gap-3 px-6 pt-16 text-center animate-rise">
        <p className="text-sm text-muted-foreground">
          No snippets match <span className="text-foreground">“{filter}”</span>
        </p>
        <div className="flex gap-2">
          <Button size="sm" variant="secondary" onClick={onClear}>
            Clear filter
          </Button>
          <Button size="sm" onClick={onNew}>
            <Plus /> Create it
          </Button>
        </div>
      </div>
    )
  const copy: Partial<Record<View["kind"], { title: string; body: string }>> = {
    all: { title: "Your library is empty", body: "Snippets turn a short trigger like ;sig into text you type often." },
    favorites: { title: "No favorites yet", body: "Star the snippets you use most to find them faster — they also rank first in search." },
    recent: { title: "Nothing used yet", body: "Snippets you expand will show up here." },
    disabled: { title: "Everything is on", body: "Snippets you turn off appear here." },
    group: { title: "This group is empty", body: "Create a snippet here or drag snippets onto the group." },
  }
  const c = copy[view.kind] ?? copy.all!
  return (
    <div className="flex flex-col items-center gap-3 px-8 pt-20 text-center animate-rise">
      <div className="flex flex-col gap-1">
        <p className="text-sm font-medium">{c.title}</p>
        <p className="text-xs leading-relaxed text-muted-foreground">{c.body}</p>
      </div>
      {(view.kind === "all" || view.kind === "group" || view.kind === "favorites") && (
        <Button size="sm" onClick={onNew}>
          <Plus /> New snippet
        </Button>
      )}
    </div>
  )
}
