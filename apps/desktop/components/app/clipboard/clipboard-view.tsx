"use client"

import {
  ClipboardList,
  Copy,
  ExternalLink,
  FolderOpen,
  MoreHorizontal,
  Pause,
  Pin,
  PinOff,
  Play,
  Search,
  Settings2,
  TextCursorInput,
  Trash2,
  Type,
  X,
} from "lucide-react"
import { forwardRef, useCallback, useDeferredValue, useEffect, useImperativeHandle, useMemo, useRef, useState } from "react"
import { toast } from "sonner"

import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "@/components/ui/alert-dialog"
import { Button } from "@/components/ui/button"
import { Checkbox } from "@/components/ui/checkbox"
import { Dialog, DialogContent, DialogDescription, DialogTitle } from "@/components/ui/dialog"
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
  DropdownMenuSeparator,
  DropdownMenuSub,
  DropdownMenuSubContent,
  DropdownMenuSubTrigger,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { useHotkeys } from "@/hooks/use-hotkeys"
import { api } from "@/lib/backend"
import {
  buildRows,
  describe,
  FILTERS,
  formatBytes,
  hasText,
  inFilter,
  isSecret,
  loadFullText,
  parseQuery,
  RETENTION_OPTIONS,
  retentionLabel,
  searchClips,
  useClipboard,
  type ClipFilter,
} from "@/lib/clipboard"
import { acceleratorLabels } from "@/lib/keys"
import { useStore } from "@/lib/store"
import type { ClipEntry } from "@/lib/types"
import { cn } from "@/lib/utils"

import { Shortcut } from "../shortcut"
import { Breadcrumbs } from "../breadcrumbs"
import { ClipList, type ClipListHandle } from "./clip-list"
import { ClipContent, ClipDetails, ClipHeading } from "./clip-preview"
import { SecretToggleIcon, setSecretWithFeedback } from "./secret"

export interface ClipboardViewHandle {
  focusSearch: () => void
  focusList: () => void
}

/** A toolbar action: icon button with a tooltip naming it and its shortcut. */
function Action({ label, combo, onClick, children, destructive, active }: { label: string; combo?: string; onClick: () => void; children: React.ReactNode; destructive?: boolean; active?: boolean }) {
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <Button
          variant="ghost"
          size="icon-sm"
          aria-label={label}
          aria-pressed={active}
          aria-keyshortcuts={combo?.replace("mod", "Meta").replace("backspace", "Backspace")}
          onClick={onClick}
          className={cn("text-muted-foreground", destructive && "hover:text-destructive", active && "text-iris-foreground")}
        >
          {children}
        </Button>
      </TooltipTrigger>
      <TooltipContent>
        {label} {combo && <Shortcut combo={combo} />}
      </TooltipContent>
    </Tooltip>
  )
}

/** The history list and preview. The kind filter comes from the sidebar's Clipboard section. */
export const ClipboardView = forwardRef<ClipboardViewHandle, { filter?: ClipFilter }>(function ClipboardView({ filter = "all" }, ref) {
  const entries = useClipboard((s) => s.entries)
  const ready = useClipboard((s) => s.ready)
  const diskUsage = useClipboard((s) => s.diskUsage)
  const removeEntries = useClipboard((s) => s.remove)
  const restoreEntries = useClipboard((s) => s.restore)
  const setPinned = useClipboard((s) => s.setPinned)
  const clearAll = useClipboard((s) => s.clear)
  const settings = useStore((s) => s.settings)
  const updateSettings = useStore((s) => s.updateSettings)
  const setView = useStore((s) => s.setView)
  const newSnippet = useStore((s) => s.newSnippet)

  const [query, setQuery] = useState("")
  const [activeId, setActiveId] = useState<string | null>(null)
  const [multi, setMulti] = useState<Set<string>>(new Set())
  const [listFocused, setListFocused] = useState(false)
  const [quickLook, setQuickLook] = useState(false)
  const [clearOpen, setClearOpen] = useState(false)
  const [clearPinned, setClearPinned] = useState(false)
  const [announce, setAnnounce] = useState("")
  const anchor = useRef<string | null>(null)
  const searchRef = useRef<HTMLInputElement>(null)
  const listRef = useRef<ClipListHandle>(null)

  useEffect(() => void useClipboard.getState().refresh(), [])
  // A new filter starts a fresh multi-selection (adjusted during render, not in an effect).
  const [shownFilter, setShownFilter] = useState(filter)
  if (shownFilter !== filter) {
    setShownFilter(filter)
    setMulti(new Set())
  }

  const deferredQuery = useDeferredValue(query)
  const terms = useMemo(() => parseQuery(deferredQuery).terms, [deferredQuery])
  const results = useMemo(() => searchClips(entries, filter, deferredQuery), [entries, filter, deferredQuery])
  const { rows, order } = useMemo(() => buildRows(results, { pinnedFirst: !deferredQuery.trim() && filter !== "pinned", sections: true }), [results, deferredQuery, filter])
  const counts = useMemo(() => Object.fromEntries(FILTERS.map((f) => [f.value, entries.filter((e) => inFilter(e, f.value)).length])) as Record<ClipFilter, number>, [entries])

  const ids = useMemo(() => order.map((e) => e.id), [order])
  const activeIndex = activeId ? ids.indexOf(activeId) : -1
  const active = activeIndex >= 0 ? order[activeIndex] : null

  // Keep a valid active row: the first one, or the neighbour of one that disappeared.
  const lastIndex = useRef(0)
  useEffect(() => {
    if (activeIndex >= 0) lastIndex.current = activeIndex
    else if (ids.length) setActiveId(ids[Math.min(lastIndex.current, ids.length - 1)])
    else if (activeId) setActiveId(null)
  }, [activeIndex, ids, activeId])

  useImperativeHandle(ref, () => ({
    focusSearch: () => searchRef.current?.focus(),
    focusList: () => listRef.current?.focus(),
  }))

  const say = (text: string) => {
    setAnnounce("")
    requestAnimationFrame(() => setAnnounce(text))
  }

  const selection = useCallback((): ClipEntry[] => {
    if (multi.size > 1) return order.filter((e) => multi.has(e.id))
    return active ? [active] : []
  }, [multi, order, active])

  const move = (delta: number, extend = false) => {
    if (!ids.length) return
    const n = Math.max(0, Math.min(ids.length - 1, (activeIndex === -1 ? (delta > 0 ? -1 : ids.length) : activeIndex) + delta))
    const id = ids[n]
    setActiveId(id)
    if (extend) {
      const a = anchor.current ?? activeId ?? id
      anchor.current = a
      const ai = ids.indexOf(a)
      const [lo, hi] = ai < n ? [ai, n] : [n, ai]
      setMulti(new Set(ids.slice(lo, hi + 1)))
    } else {
      anchor.current = id
      setMulti(new Set())
    }
  }

  const copy = useCallback(
    async (e: ClipEntry | null, flipPlain = false) => {
      if (!e) return
      const plain = e.rich && settings.clipboardPastePlain !== flipPlain
      try {
        await api.clipboardCopy(e.id, plain)
        toast.success(plain ? "Copied as plain text" : "Copied", { duration: 1600 })
        say("Copied to clipboard")
      } catch (err) {
        toast.error("Couldn't copy", { description: String(err) })
      }
    },
    [settings.clipboardPastePlain],
  )

  const remove = useCallback(
    async (list: ClipEntry[]) => {
      if (!list.length) return
      setMulti(new Set())
      const removed = await removeEntries(list.map((e) => e.id))
      const hidden = (e: ClipEntry) => isSecret(e) && !useStore.getState().settings.clipboardShowSecrets
      const label = removed.length === 1 ? (hidden(removed[0]) ? "Deleted a secret" : `Deleted “${describe(removed[0]).title.slice(0, 40)}”`) : `Deleted ${removed.length} items`
      say(label)
      toast(label, { action: { label: "Undo", onClick: () => void restoreEntries(removed) }, duration: 6000 })
      requestAnimationFrame(() => listRef.current?.focus())
    },
    [removeEntries, restoreEntries],
  )

  const togglePin = useCallback(
    (list: ClipEntry[]) => {
      if (!list.length) return
      const pin = !list.every((e) => e.pinned)
      list.forEach((e) => void setPinned(e.id, pin))
      say(pin ? (list.length > 1 ? `Pinned ${list.length} items` : "Pinned. It won't expire.") : "Unpinned")
    },
    [setPinned],
  )

  const toSnippet = useCallback(
    async (e: ClipEntry | null) => {
      if (!e || !hasText(e)) return
      const content = await loadFullText(e)
      await newSnippet({ content })
      setView({ kind: "all" })
    },
    [newSnippet, setView],
  )

  const open = (e: ClipEntry | null) => {
    if (e && (e.kind === "link" || e.kind === "file")) void api.clipboardOpen(e.id).catch((err) => toast.error("Couldn't open it", { description: String(err) }))
  }

  useHotkeys([
    { combo: "mod+f", allowInInput: true, handler: () => searchRef.current?.focus() },
    { combo: "mod+p", allowInInput: true, handler: () => togglePin(selection()) },
    { combo: "mod+s", allowInInput: true, enabled: !!active && hasText(active), handler: () => void toSnippet(active) },
    { combo: "mod+o", allowInInput: true, enabled: !!active && (active.kind === "link" || active.kind === "file"), handler: () => open(active) },
    { combo: "mod+r", allowInInput: true, enabled: active?.kind === "file", handler: () => active && void api.clipboardReveal(active.id) },
    { combo: "mod+backspace", allowInInput: false, handler: () => void remove(selection()) },
  ])

  // Stable handlers (read the latest list through a ref) so moving the selection only re-renders
  // the two rows whose state changed, not every visible row.
  const live = useRef({ ids, activeId, order, copy })
  useEffect(() => {
    live.current = { ids, activeId, order, copy }
  })
  const onPick = useCallback((id: string) => void live.current.copy(live.current.order.find((e) => e.id === id) ?? null), [])
  const onPointer = useCallback(
    (e: React.MouseEvent, id: string) => {
      if (e.button !== 0) return
      const { ids, activeId } = live.current
      if (e.metaKey || e.ctrlKey) {
        setMulti((m) => {
          const n = new Set(m.size ? m : activeId ? [activeId] : [])
          if (n.has(id)) n.delete(id)
          else n.add(id)
          return n
        })
      } else if (e.shiftKey && anchor.current) {
        const a = ids.indexOf(anchor.current)
        const b = ids.indexOf(id)
        const [lo, hi] = a < b ? [a, b] : [b, a]
        setMulti(new Set(ids.slice(lo, hi + 1)))
      } else {
        anchor.current = id
        setMulti(new Set())
      }
      setActiveId(id)
    },
    [],
  )

  const onListKey = (e: React.KeyboardEvent<HTMLDivElement>) => {
    const k = e.key
    const mod = e.metaKey || e.ctrlKey
    if (k === "ArrowDown" || k === "ArrowUp" || (!mod && (k === "j" || k === "k"))) {
      e.preventDefault()
      move(k === "ArrowDown" || k === "j" ? 1 : -1, e.shiftKey)
    } else if (k === "Home" || k === "End") {
      e.preventDefault()
      move(k === "Home" ? -ids.length : ids.length, e.shiftKey)
    } else if (k === "PageDown" || k === "PageUp") {
      e.preventDefault()
      move(k === "PageDown" ? 8 : -8, e.shiftKey)
    } else if (k === "Enter") {
      e.preventDefault()
      void copy(active, e.shiftKey)
    } else if (k === " ") {
      e.preventDefault()
      if (active) setQuickLook(true)
    } else if ((k === "Backspace" || k === "Delete") && !mod) {
      e.preventDefault()
      void remove(selection())
    } else if (mod && k === "c") {
      e.preventDefault()
      void copy(active)
    } else if (mod && k === "a") {
      e.preventDefault()
      setMulti(new Set(ids))
    } else if (k === "Escape" && multi.size) {
      e.preventDefault()
      setMulti(new Set())
    } else if (k === "/") {
      e.preventDefault()
      searchRef.current?.focus()
    } else if (k.length === 1 && !mod && !e.altKey && /\S/.test(k)) {
      searchRef.current?.focus() // type to search
    }
  }

  const bulk = multi.size > 1 ? order.filter((e) => multi.has(e.id)) : []
  const enabled = settings.clipboardEnabled
  const retention = settings.clipboardRetentionDays

  const empty = !ready ? null : entries.length === 0 ? (
    <Welcome enabled={enabled} hotkey={settings.clipboardHotkey} retention={retention} onEnable={() => void updateSettings({ clipboardEnabled: true })} />
  ) : (
    <div className="flex flex-col items-center gap-3 px-6 pt-16 text-center animate-rise">
      <p className="text-sm text-muted-foreground">
        {query ? (
          <>
            Nothing matches <span className="text-foreground">“{query}”</span>
          </>
        ) : (
          `No ${FILTERS.find((f) => f.value === filter)?.label.toLowerCase()} yet`
        )}
      </p>
      <Button
        size="sm"
        variant="secondary"
        onClick={() => {
          setQuery("")
          setView({ kind: "clipboard" })
        }}
      >
        Show everything
      </Button>
    </div>
  )

  return (
    <div className="flex h-full min-h-0 min-w-0 flex-1">
      <section aria-label="Clipboard history" className="flex w-[clamp(300px,30vw,372px)] shrink-0 flex-col border-r border-hairline">
        <header className="drag flex h-12 shrink-0 items-center gap-2 pr-2 pl-5" data-tauri-drag-region>
          <h2 className="min-w-0 flex-1 truncate text-[13px] font-semibold" data-tauri-drag-region>
            {filter === "all" ? "Clipboard" : FILTERS.find((f) => f.value === filter)?.label} <span className="ml-1 font-normal text-faint tabular">{results.length}</span>
          </h2>
          <Tooltip>
            <TooltipTrigger asChild>
              <button
                onClick={() => void updateSettings({ clipboardEnabled: !enabled })}
                aria-pressed={enabled}
                aria-label={enabled ? "Recording copies. Press to pause." : "Paused. Press to resume recording."}
                className={cn(
                  "no-drag flex h-6 items-center gap-1.5 rounded-full px-2 text-[11px] font-medium transition-colors",
                  enabled ? "text-muted-foreground hover:bg-foreground/[0.05] hover:text-foreground" : "bg-warning/12 text-warning hover:bg-warning/18",
                )}
              >
                <span className={cn("size-1.5 rounded-full", enabled ? "bg-success shadow-[0_0_0_3px_color-mix(in_oklch,var(--success)_20%,transparent)]" : "bg-warning")} />
                {enabled ? "Recording" : "Paused"}
              </button>
            </TooltipTrigger>
            <TooltipContent>{enabled ? "Pause recording" : "Resume recording"}</TooltipContent>
          </Tooltip>
          <DropdownMenu>
            <Tooltip>
              <TooltipTrigger asChild>
                <DropdownMenuTrigger asChild>
                  <Button variant="ghost" size="icon-sm" className="no-drag text-muted-foreground" aria-label="Clipboard options">
                    <MoreHorizontal />
                  </Button>
                </DropdownMenuTrigger>
              </TooltipTrigger>
              <TooltipContent>Options</TooltipContent>
            </Tooltip>
            <DropdownMenuContent align="end" className="w-60">
              <DropdownMenuItem onSelect={() => void updateSettings({ clipboardEnabled: !enabled })}>
                {enabled ? <Pause /> : <Play />} {enabled ? "Pause recording" : "Resume recording"}
              </DropdownMenuItem>
              <DropdownMenuSub>
                <DropdownMenuSubTrigger>
                  <ClipboardList /> Keep history
                  <span className="ml-auto text-xs text-faint">{RETENTION_OPTIONS.find((o) => o.days === retention)?.label ?? `${retention} days`}</span>
                </DropdownMenuSubTrigger>
                <DropdownMenuSubContent className="w-44">
                  <DropdownMenuLabel>Delete items after</DropdownMenuLabel>
                  <DropdownMenuRadioGroup value={String(retention)} onValueChange={(v) => void updateSettings({ clipboardRetentionDays: Number(v) })}>
                    {RETENTION_OPTIONS.map((o) => (
                      <DropdownMenuRadioItem key={o.days} value={String(o.days)}>
                        {o.label}
                      </DropdownMenuRadioItem>
                    ))}
                  </DropdownMenuRadioGroup>
                </DropdownMenuSubContent>
              </DropdownMenuSub>
              <DropdownMenuItem onSelect={() => setView({ kind: "settings", section: "clipboard" })}>
                <Settings2 /> Clipboard settings…
              </DropdownMenuItem>
              <DropdownMenuSeparator />
              <DropdownMenuItem variant="destructive" disabled={!entries.length} onSelect={() => setClearOpen(true)}>
                <Trash2 /> Clear history…
              </DropdownMenuItem>
            </DropdownMenuContent>
          </DropdownMenu>
        </header>

        <div className="px-3 pb-2">
          <div className="relative flex items-center">
            <Search className="pointer-events-none absolute left-2.5 size-3.5 text-faint" />
            <input
              ref={searchRef}
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "ArrowDown" || e.key === "ArrowUp") {
                  e.preventDefault()
                  move(e.key === "ArrowDown" ? 1 : -1, e.shiftKey)
                } else if (e.key === "Enter") {
                  e.preventDefault()
                  void copy(active, e.shiftKey)
                } else if (e.key === "Escape") {
                  e.preventDefault()
                  e.stopPropagation()
                  if (query) setQuery("")
                  else listRef.current?.focus()
                }
              }}
              placeholder="Search history"
              aria-label="Search clipboard history"
              aria-controls="clip-listbox"
              aria-describedby="clip-search-hint"
              spellCheck={false}
              autoComplete="off"
              className="h-8 w-full rounded-lg bg-foreground/[0.03] pr-14 pl-8 text-[13px] shadow-well outline-none placeholder:text-faint focus:bg-transparent focus:shadow-none focus:ring-2 focus:ring-ring"
            />
            <span id="clip-search-hint" className="sr-only">
              Type app: followed by an app name, or is: followed by a kind such as image or link, to narrow results.
            </span>
            {query ? (
              <button className="absolute right-2 flex size-5 items-center justify-center rounded text-faint hover:text-foreground" onClick={() => setQuery("")} aria-label="Clear search">
                <X className="size-3.5" />
              </button>
            ) : (
              <Shortcut combo="mod+f" className="pointer-events-none absolute right-2" />
            )}
          </div>
        </div>

        {!enabled && entries.length > 0 && (
          <div role="status" className="mx-3 mb-2 flex items-center gap-2 rounded-lg bg-warning/10 px-3 py-2 text-xs text-warning shadow-[inset_0_0_0_1px_color-mix(in_oklch,var(--warning)_18%,transparent)]">
            <Pause className="size-3.5 shrink-0" />
            <span className="flex-1">Recording is paused. New copies aren&apos;t saved.</span>
            <button className="font-medium underline-offset-2 hover:underline" onClick={() => void updateSettings({ clipboardEnabled: true })}>
              Resume
            </button>
          </div>
        )}

        <ClipList
          ref={listRef}
          rows={rows}
          activeId={activeId}
          selection={multi}
          focused={listFocused || multi.size <= 1}
          density="comfortable"
          terms={terms}
          label="Clipboard history"
          listId="clip-listbox"
          tabIndex={0}
          onPointer={onPointer}
          onPick={onPick}
          onKeyDown={onListKey}
          onFocusChange={setListFocused}
          empty={empty}
        />

        {bulk.length > 0 ? (
          <div role="toolbar" aria-label="Selection actions" className="m-2 flex items-center gap-1 rounded-xl bg-surface-raised p-1.5 pl-3 shadow-float animate-rise">
            <span className="flex-1 text-xs font-medium tabular">{bulk.length} selected</span>
            <Action label={bulk.every((e) => e.pinned) ? "Unpin" : "Pin"} combo="mod+p" onClick={() => togglePin(bulk)}>
              {bulk.every((e) => e.pinned) ? <PinOff /> : <Pin />}
            </Action>
            <Action label="Delete" combo="mod+backspace" destructive onClick={() => void remove(bulk)}>
              <Trash2 />
            </Action>
          </div>
        ) : (
          <footer className="flex h-9 shrink-0 items-center gap-1 border-t border-hairline px-4 text-[11px] text-faint">
            <span className="truncate">
              Kept {retentionLabel(retention)}
              {diskUsage > 0 && ` · ${formatBytes(diskUsage)}`}
            </span>
            <button className="ml-auto shrink-0 rounded px-1 text-muted-foreground hover:text-foreground" onClick={() => setView({ kind: "settings", section: "clipboard" })}>
              Change
            </button>
          </footer>
        )}
      </section>

      <section aria-label="Preview" className="flex min-w-0 flex-1 flex-col">
        <header className="flex h-12 shrink-0 items-center gap-2 border-b border-hairline px-6">
          {active && <Breadcrumbs label="Clipboard item breadcrumb" items={[
            { label: filter === "all" ? "History" : FILTERS.find((f) => f.value === filter)?.label ?? "History", onSelect: () => listRef.current?.focus() },
            { label: isSecret(active) && !settings.clipboardShowSecrets ? "Secret" : describe(active).title },
          ]} />}
        </header>
        {active ? (
          <>
            <div className="min-h-0 flex-1 overflow-y-auto px-6 py-5" key={active.id}>
              <div className="flex flex-col gap-6 animate-rise">
                <ClipHeading entry={active} />
                <ClipContent entry={active} terms={terms} />
                <ClipDetails entry={active} retentionDays={retention} />
              </div>
            </div>
            <footer role="toolbar" aria-label="Item actions" className="flex h-14 shrink-0 items-center gap-1 border-t border-hairline px-4">
              <Button size="sm" onClick={() => void copy(active)} aria-keyshortcuts="Enter">
                <Copy /> Copy <Shortcut combo="enter" className="ml-0.5 max-[1100px]:hidden" />
              </Button>
              {active.rich && (
                <Button size="sm" variant="ghost" onClick={() => void copy(active, true)} aria-keyshortcuts="Shift+Enter">
                  <Type /> {settings.clipboardPastePlain ? "With formatting" : "As plain text"} <Shortcut combo="shift+enter" tone="ghost" className="max-[1100px]:hidden" />
                </Button>
              )}
              <span className="flex-1" />
              {hasText(active) && (
                <Action label="Save as snippet" combo="mod+s" onClick={() => void toSnippet(active)}>
                  <TextCursorInput />
                </Action>
              )}
              {(active.kind === "link" || active.kind === "file") && (
                <Action label={active.kind === "link" ? "Open link" : "Open"} combo="mod+o" onClick={() => open(active)}>
                  <ExternalLink />
                </Action>
              )}
              {active.kind === "file" && (
                <Action label="Show in Finder" combo="mod+r" onClick={() => void api.clipboardReveal(active.id)}>
                  <FolderOpen />
                </Action>
              )}
              {hasText(active) && (
                <Action label={isSecret(active) ? "Not a secret — show it" : "Hide as a secret"} onClick={() => setSecretWithFeedback(active, !isSecret(active))}>
                  <SecretToggleIcon entry={active} />
                </Action>
              )}
              <Action label={active.pinned ? "Unpin" : "Pin — keep forever"} combo="mod+p" active={active.pinned} onClick={() => togglePin([active])}>
                {active.pinned ? <PinOff /> : <Pin />}
              </Action>
              <Action label="Delete" combo="mod+backspace" destructive onClick={() => void remove([active])}>
                <Trash2 />
              </Action>
            </footer>
          </>
        ) : (
          <div className="flex flex-1 items-center justify-center text-xs text-faint">{entries.length ? "Select an item to preview it" : ""}</div>
        )}
      </section>

      <Dialog open={quickLook && !!active} onOpenChange={setQuickLook}>
        <DialogContent className="max-h-[86vh] max-w-[min(920px,90vw)] gap-0 overflow-hidden rounded-2xl p-0 shadow-panel ring-0 sm:max-w-[min(920px,90vw)]">
          <DialogTitle className="sr-only">{active ? (isSecret(active) && !settings.clipboardShowSecrets ? "Secret" : describe(active).title) : "Preview"}</DialogTitle>
          <DialogDescription className="sr-only">Large preview. Press Space or Escape to close.</DialogDescription>
          {active && (
            <div
              className="flex max-h-[86vh] flex-col"
              onKeyDown={(e) => {
                if (e.key === " " && !(e.target as HTMLElement).closest(".selectable")) {
                  e.preventDefault()
                  setQuickLook(false)
                } else if (e.key === "ArrowDown" || e.key === "ArrowUp") {
                  e.preventDefault()
                  move(e.key === "ArrowDown" ? 1 : -1)
                }
              }}
            >
              <div className="border-b border-hairline px-6 py-3.5 pr-12">
                <ClipHeading entry={active} />
              </div>
              <div className="min-h-0 flex-1 overflow-y-auto p-6">
                <ClipContent entry={active} terms={terms} large />
              </div>
            </div>
          )}
        </DialogContent>
      </Dialog>

      <AlertDialog open={clearOpen} onOpenChange={setClearOpen}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>Clear clipboard history?</AlertDialogTitle>
            <AlertDialogDescription>Everything Kudzu remembered is deleted from this Mac. This can&apos;t be undone.</AlertDialogDescription>
          </AlertDialogHeader>
          <label className="flex items-center gap-2.5 text-[13px]">
            <Checkbox checked={clearPinned} onCheckedChange={(v) => setClearPinned(v === true)} />
            Also delete pinned items ({counts.pinned})
          </label>
          <AlertDialogFooter>
            <AlertDialogCancel>Cancel</AlertDialogCancel>
            <AlertDialogAction
              variant="destructive"
              onClick={() =>
                void clearAll(clearPinned).then((n) => {
                  toast.success(n === 1 ? "Deleted 1 item" : `Deleted ${n} items`)
                  say(`Cleared ${n} items`)
                })
              }
            >
              Clear history
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>

      <div aria-live="polite" className="sr-only">
        {announce}
      </div>
    </div>
  )
})

function Welcome({ enabled, hotkey, retention, onEnable }: { enabled: boolean; hotkey: string; retention: number; onEnable: () => void }) {
  return (
    <div className="flex flex-col items-center gap-4 px-8 pt-16 text-center animate-rise">
      <span className="flex size-12 items-center justify-center rounded-2xl bg-surface-raised text-muted-foreground shadow-raised sheen">
        <ClipboardList className="size-5" />
      </span>
      {enabled ? (
        <>
          <div className="flex flex-col gap-1.5">
            <p className="text-sm font-medium">Everything you copy, in one place</p>
            <p className="text-xs leading-relaxed text-muted-foreground">
              Copy text, links, images or files and they appear here with the time you copied them. Kudzu keeps them {retentionLabel(retention)}, only on this Mac.
            </p>
          </div>
          <div className="flex items-center gap-2 text-xs text-muted-foreground">
            Paste from history anywhere <Shortcut keys={acceleratorLabels(hotkey)} tone="accent" />
          </div>
        </>
      ) : (
        <>
          <div className="flex flex-col gap-1.5">
            <p className="text-sm font-medium">Clipboard history is off</p>
            <p className="text-xs leading-relaxed text-muted-foreground">Turn it on to find anything you copied, from any app.</p>
          </div>
          <Button size="sm" onClick={onEnable}>
            <Play /> Turn on
          </Button>
        </>
      )}
    </div>
  )
}

