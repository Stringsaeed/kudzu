"use client"

// The clipboard history picker: a floating window summoned by a global shortcut over any app.
// Type to search, see a live preview, and paste into the app you came from without the mouse.

import { ClipboardList, CornerDownLeft, Search } from "lucide-react"
import { useCallback, useDeferredValue, useEffect, useMemo, useRef, useState } from "react"

import { api, Events, on } from "@/lib/backend"
import { buildRows, describe, hasText, parseQuery, searchClips, useClipboard, type ClipFilter } from "@/lib/clipboard"
import { isMac } from "@/lib/keys"
import { applyTheme } from "@/lib/theme"
import type { AppInfo, ClipEntry, Settings } from "@/lib/types"
import { cn } from "@/lib/utils"

import { Shortcut } from "../shortcut"
import { ClipFilters } from "./clip-filters"
import { ClipList } from "./clip-list"
import { ClipContent, ClipDetails, ClipHeading } from "./clip-preview"

const PICKER_FILTERS: ClipFilter[] = ["all", "pinned", "text", "image", "link", "file"]

export function ClipboardPicker() {
  const entries = useClipboard((s) => s.entries)
  const ready = useClipboard((s) => s.ready)
  const removeEntries = useClipboard((s) => s.remove)
  const restoreEntries = useClipboard((s) => s.restore)
  const setPinned = useClipboard((s) => s.setPinned)
  const [settings, setSettings] = useState<Pick<Settings, "clipboardPastePlain" | "clipboardRetentionDays" | "clipboardEnabled"> | null>(null)
  const [target, setTarget] = useState<AppInfo | null>(null)
  const [canPaste, setCanPaste] = useState(true)
  const [query, setQuery] = useState("")
  const [filter, setFilter] = useState<ClipFilter>("all")
  const [activeId, setActiveId] = useState<string | null>(null)
  const [showNumbers, setShowNumbers] = useState(false)
  const [flash, setFlash] = useState<string | null>(null)
  const undo = useRef<ClipEntry[] | null>(null)
  const input = useRef<HTMLInputElement>(null)
  const flashTimer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined)

  useEffect(() => {
    const off = useClipboard.getState().init()
    api.getState().then((s) => {
      setSettings(s.settings)
      applyTheme(s.settings.theme)
    })
    const offs = [
      off,
      on<Settings>(Events.settings, (s) => {
        setSettings(s)
        applyTheme(s.theme)
      }),
      on<{ target?: AppInfo | null; canPaste?: boolean }>(Events.clipboardShow, (p) => {
        setTarget(p.target ?? null)
        setCanPaste(p.canPaste ?? true)
        setQuery("")
        setFilter("all")
        setActiveId(null)
        setFlash(null)
        undo.current = null
        requestAnimationFrame(() => input.current?.focus())
      }),
    ]
    const onBlur = () => {
      setShowNumbers(false)
      void api.hideClipboard()
    }
    window.addEventListener("blur", onBlur)
    return () => {
      offs.forEach((f) => f())
      window.removeEventListener("blur", onBlur)
    }
  }, [])

  const deferredQuery = useDeferredValue(query)
  const terms = useMemo(() => parseQuery(deferredQuery).terms, [deferredQuery])
  const results = useMemo(() => searchClips(entries, filter, deferredQuery), [entries, filter, deferredQuery])
  // Most recent first: what you just copied is one keystroke away. Pinned have their own filter.
  const { rows, order } = useMemo(() => buildRows(results, { pinnedFirst: false, sections: !deferredQuery.trim() }), [results, deferredQuery])
  const index = activeId ? order.findIndex((e) => e.id === activeId) : -1
  const active = index >= 0 ? order[index] : (order[0] ?? null)
  const activeIndex = index >= 0 ? index : 0

  const say = (text: string, ms = 1600) => {
    clearTimeout(flashTimer.current)
    setFlash(text)
    flashTimer.current = setTimeout(() => setFlash(null), ms)
  }

  const plainFor = (e: ClipEntry, shift: boolean) => e.rich && (settings?.clipboardPastePlain ?? false) !== shift

  const paste = useCallback(
    async (e: ClipEntry | undefined | null, shift: boolean) => {
      if (!e) return
      try {
        const r = await api.clipboardPaste(e.id, plainFor(e, shift))
        if (r === "copied") {
          say("Copied. To paste directly, allow Kudzu in Accessibility settings.", 2600)
          setTimeout(() => void api.hideClipboard(), 1800)
        }
      } catch (err) {
        say(String(err), 3000)
      }
    },
    // oxlint-disable-next-line react/exhaustive-deps
    [settings],
  )

  const copyOnly = async (e: ClipEntry | null, shift: boolean) => {
    if (!e) return
    await api.clipboardCopy(e.id, plainFor(e, shift)).catch(() => {})
    say("Copied")
    setTimeout(() => void api.hideClipboard(), 420)
  }

  const move = (delta: number) => {
    if (!order.length) return
    const n = Math.max(0, Math.min(order.length - 1, activeIndex + delta))
    setActiveId(order[n].id)
  }

  const onKeyDown = (e: React.KeyboardEvent) => {
    const mod = isMac ? e.metaKey : e.ctrlKey
    if (mod) setShowNumbers(true)
    const k = e.key
    if (k === "ArrowDown" || (e.ctrlKey && k === "n")) {
      e.preventDefault()
      move(1)
    } else if (k === "ArrowUp" || (e.ctrlKey && k === "p")) {
      e.preventDefault()
      move(-1)
    } else if (k === "PageDown" || k === "PageUp") {
      e.preventDefault()
      move(k === "PageDown" ? 7 : -7)
    } else if (k === "Enter") {
      e.preventDefault()
      if (mod) void copyOnly(active, e.shiftKey)
      else void paste(active, e.shiftKey)
    } else if (mod && /^[1-9]$/.test(k)) {
      e.preventDefault()
      void paste(order[Number(k) - 1], e.shiftKey)
    } else if (mod && k.toLowerCase() === "p") {
      e.preventDefault()
      if (active) {
        void setPinned(active.id, !active.pinned)
        say(active.pinned ? "Unpinned" : "Pinned. It won't expire.")
      }
    } else if (mod && (k === "Backspace" || k === "Delete")) {
      e.preventDefault()
      if (active) {
        if (order[activeIndex + 1]) setActiveId(order[activeIndex + 1].id)
        else if (order[activeIndex - 1]) setActiveId(order[activeIndex - 1].id)
        void removeEntries([active.id]).then((gone) => {
          undo.current = gone
        })
        say(`Deleted. ${isMac ? "⌘" : "Ctrl+"}Z to undo`, 4000)
      }
    } else if (mod && k.toLowerCase() === "z" && undo.current && !query) {
      e.preventDefault()
      void restoreEntries(undo.current)
      undo.current = null
      say("Restored")
    } else if (mod && k.toLowerCase() === "s") {
      e.preventDefault()
      if (active && hasText(active)) void api.clipboardToSnippet(active.id)
    } else if (mod && k.toLowerCase() === "o") {
      e.preventDefault()
      if (active && (active.kind === "link" || active.kind === "file")) {
        void api.clipboardOpen(active.id)
        void api.hideClipboard()
      }
    } else if (mod && k === ",") {
      e.preventDefault()
      void api.showMainView({ kind: "settings", section: "clipboard" })
    } else if (k === "Tab") {
      e.preventDefault()
      const i = PICKER_FILTERS.indexOf(filter)
      setFilter(PICKER_FILTERS[(i + (e.shiftKey ? -1 : 1) + PICKER_FILTERS.length) % PICKER_FILTERS.length])
      setActiveId(null)
    } else if (k === "Escape") {
      e.preventDefault()
      if (query) {
        setQuery("")
        setActiveId(null)
      } else void api.hideClipboard()
    }
  }

  const live = useRef({ order, paste })
  useEffect(() => {
    live.current = { order, paste }
  })
  const onPick = useCallback((id: string) => void live.current.paste(live.current.order.find((x) => x.id === id), false), [])
  const onPointer = useCallback((e: React.MouseEvent, id: string) => {
    if (e.button !== 0) return
    e.preventDefault() // keep focus in the search field
    setActiveId(id)
  }, [])

  const enabled = settings?.clipboardEnabled ?? true
  const pasteLabel = canPaste ? (target ? `Paste into ${target.name}` : "Paste") : "Copy"

  return (
    <div
      className="flex h-full w-full items-start justify-center p-3 vibrant:p-0"
      onKeyUp={(e) => {
        if (e.key === "Meta" || e.key === "Control") setShowNumbers(false)
      }}
    >
      <div
        role="dialog"
        aria-label="Clipboard history"
        className="flex h-full w-full flex-col overflow-hidden rounded-[18px] bg-popover text-popover-foreground shadow-panel backdrop-blur-2xl animate-pop vibrant:animate-none vibrant:rounded-none vibrant:bg-popover/55 vibrant:shadow-none vibrant:backdrop-blur-none"
      >
        {/* Search */}
        <div className="flex h-[58px] shrink-0 items-center gap-3 border-b border-hairline px-4">
          <Search className="size-[18px] shrink-0 text-muted-foreground" />
          <input
            ref={input}
            autoFocus
            value={query}
            onChange={(e) => {
              setQuery(e.target.value)
              setActiveId(null)
            }}
            onKeyDown={onKeyDown}
            placeholder="Search clipboard history…"
            aria-label="Search clipboard history"
            role="combobox"
            aria-expanded
            aria-controls="picker-results"
            aria-activedescendant={active ? `clip-${active.id}` : undefined}
            aria-autocomplete="list"
            spellCheck={false}
            autoComplete="off"
            className="h-full min-w-0 flex-1 bg-transparent text-[17px] tracking-[-0.01em] outline-none placeholder:text-faint"
          />
          <ClipFilters
            value={filter}
            onChange={(f) => {
              setFilter(f)
              setActiveId(null)
              input.current?.focus()
            }}
            options={PICKER_FILTERS}
            focusable={false}
            className="shrink-0 rounded-full bg-foreground/[0.035] p-0.5 shadow-well"
          />
        </div>

        {/* Results + preview */}
        <div className="flex min-h-0 flex-1">
          <div className="flex w-[50%] shrink-0 flex-col pt-1.5">
            <ClipList
              rows={rows}
              activeId={active?.id ?? null}
              focused
              density="compact"
              terms={terms}
              label={query ? "Matches" : "Recent copies"}
              listId="picker-results"
              numbered
              showNumbers={showNumbers}
              onPointer={onPointer}
              onPick={onPick}
              empty={
                ready && (
                  <div className="flex h-full flex-col items-center justify-center gap-2 px-8 text-center">
                    <ClipboardList className="size-5 text-faint" />
                    <p className="text-sm text-muted-foreground">
                      {!enabled ? "Clipboard history is paused." : entries.length === 0 ? "Nothing copied yet." : query ? `Nothing matches “${query}”.` : "Nothing here yet."}
                    </p>
                    <p className="text-xs text-faint">{!enabled ? "Turn it on in Kudzu's settings." : entries.length === 0 ? "Copy something in any app and it shows up here." : "Try another filter, or app: and is: to narrow it down."}</p>
                  </div>
                )
              }
            />
          </div>

          <aside aria-label="Preview" aria-live="polite" className="flex min-w-0 flex-1 flex-col border-l border-hairline bg-foreground/[0.015]">
            {active ? (
              <>
                <div className="px-5 pt-3.5 pb-2.5">
                  <ClipHeading entry={active} />
                </div>
                <div className="min-h-0 flex-1 overflow-y-auto px-5 pb-4" key={active.id}>
                  <ClipContent entry={active} terms={terms} />
                </div>
                <div className="shrink-0 border-t border-hairline px-5 py-3">
                  <ClipDetails entry={active} retentionDays={settings?.clipboardRetentionDays ?? 7} compact />
                </div>
              </>
            ) : (
              <div className="flex h-full items-center justify-center text-xs text-faint" />
            )}
          </aside>
        </div>

        {/* Footer hints */}
        <footer className="flex h-10 shrink-0 items-center gap-4 border-t border-hairline px-4 text-[11.5px] text-muted-foreground">
          <span className="flex min-w-0 items-center gap-1.5" aria-live="polite">
            <span className="flex size-[18px] shrink-0 items-center justify-center rounded-[5px] bg-iris text-iris-contrast shadow-accent sheen-strong">
              <CornerDownLeft className="size-3" />
            </span>
            <span className={cn("truncate", flash && "text-foreground")}>{flash ?? pasteLabel}</span>
          </span>
          {!flash && (
            <>
              {canPaste && (
                <span className="flex shrink-0 items-center gap-1.5">
                  <Shortcut combo="mod+enter" /> Copy
                </span>
              )}
              {active?.rich && (
                <span className="flex shrink-0 items-center gap-1.5">
                  <Shortcut combo="shift+enter" /> {settings?.clipboardPastePlain ? "Formatted" : "Plain text"}
                </span>
              )}
              <span className="flex shrink-0 items-center gap-1.5">
                <Shortcut combo="mod+p" /> {active?.pinned ? "Unpin" : "Pin"}
              </span>
              <span className="flex shrink-0 items-center gap-1.5">
                <Shortcut combo="mod+backspace" /> Delete
              </span>
            </>
          )}
          <span className="ml-auto flex shrink-0 items-center gap-1.5">
            <Shortcut combo="tab" /> Filter
          </span>
          <Shortcut combo="escape" className="shrink-0" />
        </footer>
        <span className="sr-only" aria-live="polite">
          {active ? `${describe(active).title}` : ""}
        </span>
      </div>
    </div>
  )
}
