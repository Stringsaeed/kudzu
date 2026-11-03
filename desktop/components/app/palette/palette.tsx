"use client"

// The quick-search tray: a floating window summoned by a global shortcut over any app.
// Type to fuzzy-search (name, trigger, content; usage-boosted), see the expanded result live,
// and insert it into the app you came from without touching the mouse.

import {
  CalendarDays,
  Clipboard,
  CornerDownLeft,
  FileText,
  Folder,
  Plus,
  Search,
  Star,
  TextCursorInput,
} from "lucide-react"
import { memo, useCallback, useDeferredValue, useEffect, useMemo, useRef, useState } from "react"

import { api, Events, on, renderPreview } from "@/lib/backend"
import { groupPath } from "@/lib/groups"
import { isMac } from "@/lib/keys"
import { buildIndex, search, type SearchHit } from "@/lib/search"
import { formatCount, relativeTime, usageById } from "@/lib/stats"
import { analyze } from "@/lib/template"
import { applyTheme } from "@/lib/theme"
import type { AppInfo, AppState, Library, RenderResult, Settings, Snippet } from "@/lib/types"
import { cn } from "@/lib/utils"

import { Shortcut } from "../shortcut"
import { TriggerChip } from "../trigger-chip"

type Scope = "all" | "favorites" | "recent"
const SCOPES: { value: Scope; label: string }[] = [
  { value: "all", label: "All" },
  { value: "favorites", label: "Favorites" },
  { value: "recent", label: "Recent" },
]
const LIMIT = 60

function Highlight({ text, hits }: { text: string; hits: number[] }) {
  if (!hits.length) return <>{text}</>
  const set = new Set(hits)
  return (
    <>
      {[...text].map((c, i) =>
        set.has(i) ? (
          <span key={i} className="text-iris-foreground">
            {c}
          </span>
        ) : (
          c
        ),
      )}
    </>
  )
}

function KindIcon({ snippet }: { snippet: Snippet }) {
  const f = analyze(snippet.content)
  if (f.isForm) return <TextCursorInput className="size-3.5" />
  if (f.usesClipboard) return <Clipboard className="size-3.5" />
  if (/\{\{(date|time)/.test(snippet.content)) return <CalendarDays className="size-3.5" />
  return <FileText className="size-3.5" />
}

const ResultRow = memo(function ResultRow({
  hit,
  active,
  index,
  showNumbers,
  path,
  onHover,
  onPick,
}: {
  hit: SearchHit
  active: boolean
  index: number
  showNumbers: boolean
  path: string
  onHover: (i: number) => void
  onPick: (i: number) => void
}) {
  const s = hit.snippet
  return (
    <div
      id={`pal-${s.id}`}
      role="option"
      aria-selected={active}
      onMouseMove={() => onHover(index)}
      onClick={() => onPick(index)}
      className={cn(
        "relative flex h-11 cursor-default items-center gap-3 rounded-[10px] px-2.5 transition-colors duration-75",
        active ? "bg-foreground/[0.08] shadow-selected" : "hover:bg-foreground/[0.03]",
        !s.isEnabled && "opacity-60",
      )}
    >
      <span className={cn("flex size-7 shrink-0 items-center justify-center rounded-lg transition-colors", active ? "bg-iris text-iris-contrast shadow-accent sheen-strong" : "bg-foreground/[0.06] text-muted-foreground")}>
        <KindIcon snippet={s} />
      </span>
      <span className="flex min-w-0 flex-1 flex-col">
        <span className="flex min-w-0 items-center gap-1.5">
          <span className="truncate text-[13.5px] font-medium">
            <Highlight text={s.name || "Untitled"} hits={s.name ? hit.nameHits : []} />
          </span>
          {s.isFavorite && <Star className="size-3 shrink-0 fill-favorite text-favorite" />}
        </span>
        {path && <span className="truncate text-[11px] text-faint">{path}</span>}
      </span>
      {s.triggers[0] && <TriggerChip trigger={s.triggers[0]} muted={!active} className="shrink-0" />}
      {/* Always shown; lights up while the modifier is held, when the number picks the row. */}
      {index < 9 && <Shortcut combo={`mod+${index + 1}`} tone={showNumbers ? "accent" : "default"} className="shrink-0" />}
    </div>
  )
})

export function Palette() {
  const [library, setLibrary] = useState<Library | null>(null)
  const [target, setTarget] = useState<AppInfo | null>(null)
  const [query, setQuery] = useState("")
  const [scope, setScope] = useState<Scope>("all")
  const [active, setActive] = useState(0)
  const [showNumbers, setShowNumbers] = useState(false)
  const [rendered, setRendered] = useState<{ id: string; result: RenderResult } | null>(null)
  const [flash, setFlash] = useState<string | null>(null)
  const input = useRef<HTMLInputElement>(null)
  const list = useRef<HTMLDivElement>(null)

  // Data
  useEffect(() => {
    api.getState().then((s: AppState) => {
      setLibrary(s.library)
      applyTheme(s.settings.theme)
    })
    const offs = [
      on<Library>(Events.library, setLibrary),
      on<Settings>(Events.settings, (s) => applyTheme(s.theme)),
      on<{ target?: AppInfo | null }>(Events.paletteShow, (p) => {
        setTarget(p.target ?? null)
        setQuery("")
        setScope("all")
        setActive(0)
        setFlash(null)
        requestAnimationFrame(() => input.current?.focus())
      }),
    ]
    const onBlur = () => {
      setShowNumbers(false)
      void api.hidePalette()
    }
    window.addEventListener("blur", onBlur)
    return () => {
      offs.forEach((f) => f())
      window.removeEventListener("blur", onBlur)
    }
  }, [])

  const usage = useMemo(() => (library ? usageById(library) : new Map()), [library])
  const deferredQuery = useDeferredValue(query)

  // "#work invoice" → group filter + text query.
  const { text, groupFilter } = useMemo(() => {
    const m = /(?:^|\s)#(\S*)/.exec(deferredQuery)
    return { text: m ? deferredQuery.replace(m[0], " ").trim() : deferredQuery, groupFilter: m ? m[1].toLowerCase() : null }
  }, [deferredQuery])

  const scoped = useMemo(() => {
    if (!library) return []
    let s = library.snippets
    if (scope === "favorites") s = s.filter((x) => x.isFavorite)
    if (scope === "recent") s = s.filter((x) => usage.get(x.id)?.last)
    if (groupFilter !== null) {
      const ids = new Set(library.groups.filter((g) => g.name.toLowerCase().startsWith(groupFilter)).map((g) => g.id))
      s = s.filter((x) => x.groupID && ids.has(x.groupID))
    }
    return s
  }, [library, scope, usage, groupFilter])

  const index = useMemo(() => buildIndex(scoped, usage), [scoped, usage])
  const hits = useMemo(() => {
    const r = search(index, text, LIMIT)
    if (scope === "recent" && !text) r.sort((a, b) => (usage.get(b.snippet.id)?.last ?? 0) - (usage.get(a.snippet.id)?.last ?? 0))
    else if (!text) r.sort((a, b) => Number(b.snippet.isFavorite) - Number(a.snippet.isFavorite) || b.score - a.score)
    return r
  }, [index, text, scope, usage])

  const createRow = query.trim() && !hits.some((h) => h.snippet.triggers.some((t) => t.toLowerCase() === query.trim().toLowerCase()))
  const total = hits.length + (createRow ? 1 : 0)
  const clamped = Math.min(active, Math.max(0, total - 1))
  const current = hits[clamped]?.snippet ?? null


  // Keep active row in view.
  useEffect(() => {
    const el = list.current?.querySelector<HTMLElement>(`[data-row="${clamped}"]`)
    el?.scrollIntoView({ block: "nearest" })
  }, [clamped])

  // Live preview of the active snippet.
  const resolve = useCallback((id: string) => library?.snippets.find((s) => s.id === id)?.content, [library])
  useEffect(() => {
    if (!current) return
    let live = true
    const t = setTimeout(() => {
      renderPreview(current.content, current.id, {}, resolve).then((r) => live && setRendered({ id: current.id, result: r }))
    }, 40)
    return () => {
      live = false
      clearTimeout(t)
    }
  }, [current, resolve])

  const pick = useCallback(
    (i: number, mode: "insert" | "copy" | "edit" = "insert") => {
      if (i >= hits.length) {
        void api.paletteCreate(query.trim())
        return
      }
      const s = hits[i].snippet
      if (mode === "insert") void api.paletteInsert(s.id)
      else if (mode === "copy") {
        setFlash("Copied")
        void api.paletteCopy(s.id)
      } else void api.showMain(s.id)
    },
    [hits, query],
  )

  const onKeyDown = (e: React.KeyboardEvent) => {
    const mod = isMac ? e.metaKey : e.ctrlKey
    if (mod) setShowNumbers(true)
    if (e.key === "ArrowDown" || (e.ctrlKey && e.key === "n")) {
      e.preventDefault()
      setActive((a) => (Math.min(a, total - 1) + 1) % Math.max(1, total))
    } else if (e.key === "ArrowUp" || (e.ctrlKey && e.key === "p")) {
      e.preventDefault()
      setActive((a) => (Math.min(a, total - 1) - 1 + total) % Math.max(1, total))
    } else if (e.key === "PageDown" || e.key === "PageUp") {
      e.preventDefault()
      setActive((a) => Math.max(0, Math.min(total - 1, a + (e.key === "PageDown" ? 7 : -7))))
    } else if (e.key === "Enter") {
      e.preventDefault()
      if (total) pick(clamped, mod ? "copy" : "insert")
    } else if (mod && e.key.toLowerCase() === "e") {
      e.preventDefault()
      if (current) pick(clamped, "edit")
    } else if (mod && e.key.toLowerCase() === "n") {
      e.preventDefault()
      void api.paletteCreate(query.trim())
    } else if (mod && /^[1-9]$/.test(e.key)) {
      e.preventDefault()
      const i = Number(e.key) - 1
      if (i < hits.length) pick(i)
    } else if (e.key === "Tab") {
      e.preventDefault()
      const i = SCOPES.findIndex((s) => s.value === scope)
      setScope(SCOPES[(i + (e.shiftKey ? -1 : 1) + SCOPES.length) % SCOPES.length].value)
      setActive(0)
    } else if (e.key === "Escape") {
      e.preventDefault()
      if (query) {
        setQuery("")
        setActive(0)
      } else void api.hidePalette()
    }
  }

  const paths = useMemo(() => {
    const m = new Map<string, string>()
    if (library) for (const g of library.groups) m.set(g.id, groupPath(library.groups, g.id).join(" › "))
    return m
  }, [library])

  const preview = current && rendered?.id === current.id ? rendered.result : null
  const facts = current ? analyze(current.content) : null
  const u = current ? usage.get(current.id) : undefined
  const sectionLabel = text ? "Best matches" : scope === "recent" ? "Recently used" : scope === "favorites" ? "Favorites" : "Suggested"

  return (
    <div
      className="flex h-full w-full items-start justify-center p-3 vibrant:p-0"
      onKeyUp={(e) => {
        if (e.key === "Meta" || e.key === "Control") setShowNumbers(false)
      }}
    >
      <div
        role="dialog"
        aria-label="Quick search"
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
              setActive(0)
            }}
            onKeyDown={onKeyDown}
            placeholder="Search snippets…"
            aria-label="Search snippets"
            role="combobox"
            aria-expanded
            aria-controls="palette-results"
            aria-activedescendant={current ? `pal-${current.id}` : undefined}
            aria-autocomplete="list"
            spellCheck={false}
            autoComplete="off"
            className="h-full min-w-0 flex-1 bg-transparent text-[17px] tracking-[-0.01em] outline-none placeholder:text-faint"
          />
          <div role="radiogroup" aria-label="Scope" className="flex shrink-0 rounded-lg bg-foreground/[0.04] p-0.5 shadow-well">
            {SCOPES.map((s) => (
              <button
                key={s.value}
                role="radio"
                tabIndex={-1}
                aria-checked={scope === s.value}
                onMouseDown={(e) => e.preventDefault()}
                onClick={() => {
                  setScope(s.value)
                  setActive(0)
                }}
                className={cn("h-6 rounded-md px-2 text-[11px] font-medium text-muted-foreground transition-colors", scope === s.value && "bg-surface-raised text-foreground shadow-soft")}
              >
                {s.label}
              </button>
            ))}
          </div>
        </div>

        {/* Results + preview */}
        <div className="flex min-h-0 flex-1">
          <div ref={list} id="palette-results" role="listbox" aria-label={sectionLabel} className="w-[54%] shrink-0 overflow-y-auto p-1.5">
            {library === null ? null : total === 0 ? (
              <div className="flex h-full flex-col items-center justify-center gap-1 px-6 text-center">
                <p className="text-sm text-muted-foreground">{scope === "favorites" ? "No favorites yet." : scope === "recent" ? "Nothing used yet." : "No snippets yet."}</p>
                <p className="text-xs text-faint">Type to create one.</p>
              </div>
            ) : (
              <>
                <div className="px-2.5 pt-1.5 pb-1 text-[11px] font-medium text-faint">
                  {sectionLabel}
                  {groupFilter !== null && <span className="ml-1 text-iris-foreground">· #{groupFilter || "…"}</span>}
                </div>
                {hits.map((h, i) => (
                  <div key={h.snippet.id} data-row={i}>
                    <ResultRow hit={h} active={i === clamped} index={i} showNumbers={showNumbers} path={h.snippet.groupID ? (paths.get(h.snippet.groupID) ?? "") : ""} onHover={setActive} onPick={pick} />
                  </div>
                ))}
                {createRow && (
                  <div data-row={hits.length}>
                    <div
                      role="option"
                      aria-selected={clamped === hits.length}
                      onMouseMove={() => setActive(hits.length)}
                      onClick={() => pick(hits.length)}
                      className={cn("flex h-11 items-center gap-3 rounded-[10px] px-2.5 text-[13.5px]", clamped === hits.length ? "bg-foreground/[0.08]" : "text-muted-foreground")}
                    >
                      <span className="flex size-7 items-center justify-center rounded-lg bg-foreground/[0.06]">
                        <Plus className="size-3.5" />
                      </span>
                      <span className="truncate">
                        Create snippet <span className="text-foreground">“{query.trim()}”</span>
                      </span>
                    </div>
                  </div>
                )}
              </>
            )}
          </div>

          <aside aria-label="Preview" aria-live="polite" className="flex min-w-0 flex-1 flex-col border-l border-hairline bg-foreground/[0.015]">
            {current ? (
              <>
                <div className="min-h-0 flex-1 overflow-y-auto px-5 pt-4 pb-3">
                  <div className="mb-2 text-[11px] font-medium text-faint">Will insert</div>
                  <div className="selectable text-[13.5px] leading-relaxed break-words whitespace-pre-wrap text-foreground/90">
                    {preview ? (
                      preview.cursorOffset != null ? (
                        <>
                          {preview.text.slice(0, preview.cursorOffset)}
                          <span className="mx-px inline-block h-[1.05em] w-[2px] translate-y-[2px] rounded-full bg-iris" aria-label="cursor" />
                          {preview.text.slice(preview.cursorOffset)}
                        </>
                      ) : (
                        preview.text || <span className="text-faint">Empty snippet</span>
                      )
                    ) : (
                      <span className="block h-4 w-2/3 rounded bg-[linear-gradient(90deg,transparent,color-mix(in_oklch,var(--foreground)_8%,transparent),transparent)] bg-[length:200%_100%] animate-shimmer" />
                    )}
                  </div>
                  {facts?.isForm && (
                    <div className="mt-3 inline-flex items-center gap-1.5 rounded-md bg-warning/12 px-2 py-1 text-[11px] font-medium text-warning">
                      <TextCursorInput className="size-3" /> Asks for {facts.fieldNames.join(", ")}
                    </div>
                  )}
                </div>
                <dl className="grid shrink-0 grid-cols-[auto_1fr] gap-x-4 gap-y-1.5 border-t border-hairline px-5 py-3 text-[11.5px]">
                  <dt className="text-faint">Triggers</dt>
                  <dd className="flex min-w-0 flex-wrap gap-1">
                    {current.triggers.length ? current.triggers.map((t) => <TriggerChip key={t} trigger={t} muted />) : <span className="text-muted-foreground">None</span>}
                  </dd>
                  {current.groupID && (
                    <>
                      <dt className="text-faint">Group</dt>
                      <dd className="flex items-center gap-1 truncate text-muted-foreground">
                        <Folder className="size-3" /> {paths.get(current.groupID)}
                      </dd>
                    </>
                  )}
                  <dt className="text-faint">Used</dt>
                  <dd className="text-muted-foreground tabular">
                    {u?.count ? `${formatCount(u.count)}× · ${relativeTime(u.last).toLowerCase()}` : "Never"}
                  </dd>
                </dl>
              </>
            ) : (
              <div className="flex h-full items-center justify-center text-xs text-faint">{createRow ? "Opens Kudzu with a new snippet" : ""}</div>
            )}
          </aside>
        </div>

        {/* Footer hints */}
        <footer className="flex h-10 shrink-0 items-center gap-4 border-t border-hairline px-4 text-[11.5px] text-muted-foreground">
          <span className="flex min-w-0 items-center gap-1.5">
            <span className="flex size-[18px] items-center justify-center rounded-[5px] bg-iris text-iris-contrast shadow-accent sheen-strong">
              <CornerDownLeft className="size-3" />
            </span>
            <span className="truncate">{flash ?? (target ? `Insert into ${target.name}` : "Insert")}</span>
          </span>
          <span className="flex items-center gap-1.5">
            <Shortcut combo="mod+enter" /> Copy
          </span>
          <span className="flex items-center gap-1.5">
            <Shortcut combo="mod+e" /> Edit
          </span>
          <span className="flex items-center gap-1.5">
            <Shortcut combo="tab" /> Scope
          </span>
          <span className="ml-auto flex items-center gap-1.5 text-faint">
            <span className="font-mono">#</span>group
          </span>
          <span className="flex items-center gap-1.5">
            <Shortcut combo="escape" />
          </span>
        </footer>
      </div>
    </div>
  )
}
