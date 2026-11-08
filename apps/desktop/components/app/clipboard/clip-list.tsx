"use client"

import { useVirtualizer } from "@tanstack/react-virtual"
import { Pin } from "lucide-react"
import { forwardRef, memo, useEffect, useImperativeHandle, useMemo, useRef } from "react"

import { Item, ItemActions, ItemContent, ItemDescription, ItemMedia, ItemTitle } from "@/components/ui/item"
import { clipTime, describe, matchRanges, secretReason, spokenLabel, type ClipRow } from "@/lib/clipboard"
import type { ClipEntry } from "@/lib/types"
import { cn } from "@/lib/utils"

import { Shortcut } from "../shortcut"
import { ClipTile } from "./clip-tile"
import { dots, RowSecret, useConcealed } from "./secret"

export type Density = "comfortable" | "compact"

const SIZES = {
  comfortable: { row: 58, header: 30, tile: 36 },
  compact: { row: 50, header: 28, tile: 32 },
} as const

/** Text with search matches underlined in the accent colour. */
export function Marked({ text, terms }: { text: string; terms: string[] }) {
  const ranges = useMemo(() => matchRanges(text, terms), [text, terms])
  if (!ranges.length) return <>{text}</>
  const out: React.ReactNode[] = []
  let at = 0
  for (const [s, e] of ranges) {
    if (s > at) out.push(text.slice(at, s))
    out.push(
      <mark key={s} className="rounded-[2px] bg-iris-soft text-iris-foreground">
        {text.slice(s, e)}
      </mark>,
    )
    at = e
  }
  out.push(text.slice(at))
  return <>{out}</>
}

const Row = memo(function Row({
  entry,
  active,
  selected,
  focused,
  density,
  terms,
  number,
  showNumbers,
  dated,
  onPointer,
  onPick,
}: {
  dated: boolean
  entry: ClipEntry
  active: boolean
  selected: boolean
  focused: boolean
  density: Density
  terms: string[]
  number: number | null
  showNumbers: boolean
  onPointer: (e: React.MouseEvent, id: string) => void
  onPick: (id: string) => void
}) {
  const { title, subtitle } = describe(entry)
  const mono = entry.kind === "code" || entry.kind === "color"
  const concealed = useConcealed(entry)
  return (
    <Item
      id={`clip-${entry.id}`}
      role="option"
      aria-selected={active || selected}
      aria-label={spokenLabel(entry, concealed)}
      onMouseDown={(e) => onPointer(e, entry.id)}
      onDoubleClick={() => onPick(entry.id)}
      className={cn(
        "group/row relative mx-1.5 h-full w-auto cursor-default flex-nowrap gap-3 rounded-[10px] px-2.5 py-0 duration-75",
        "hover:bg-foreground/[0.035]",
        selected && "bg-foreground/[0.06] shadow-selected",
        active && (focused ? "bg-iris-soft hover:bg-iris-soft after:pointer-events-none after:absolute after:inset-0 after:rounded-[10px] after:ring-1 after:ring-iris/30 after:ring-inset" : "bg-foreground/[0.07] shadow-selected hover:bg-foreground/[0.07]"),
      )}
    >
      <ItemMedia className="group-has-data-[slot=item-description]/item:translate-y-0 group-has-data-[slot=item-description]/item:self-center">
        <ClipTile entry={entry} active={active && focused} size={SIZES[density].tile} />
      </ItemMedia>
      <ItemContent className="min-w-0 gap-0.5">
        <ItemTitle className={cn("block w-full truncate text-[13px] leading-[18px]", mono && "font-mono text-[12.5px]", !title && "text-muted-foreground italic")}>
          <RowSecret concealed={concealed} stand={dots(title.length)}>
            <Marked text={title.slice(0, 200) || "Empty"} terms={terms} />
          </RowSecret>
        </ItemTitle>
        <ItemDescription className={cn("truncate text-[11.5px] leading-4 text-muted-foreground/85", entry.kind === "link" && "font-mono text-[11px]")}>
          <RowSecret concealed={concealed} stand={`${secretReason(entry)} · hover to show`}>
            <Marked text={subtitle.slice(0, 200)} terms={terms} />
          </RowSecret>
        </ItemDescription>
      </ItemContent>
      <ItemActions className="shrink-0 flex-col items-end gap-1 self-center">
        <span className="text-[11px] leading-none text-faint tabular">{clipTime(entry.copiedAt, undefined, dated)}</span>
        <span className="flex h-3.5 items-center gap-1">
          {entry.copyCount > 1 && (
            <span className="text-[10.5px] leading-none text-faint tabular" title={`Copied ${entry.copyCount} times`}>
              ×{entry.copyCount}
            </span>
          )}
          {entry.pinned && <Pin className="size-3 -rotate-45 fill-current text-iris-foreground/80" aria-hidden />}
        </span>
      </ItemActions>
      {number !== null && <Shortcut combo={`mod+${number}`} tone={showNumbers ? "accent" : "ghost"} className="shrink-0" />}
    </Item>
  )
})

export interface ClipListHandle {
  focus: () => void
  /** Scrolls an entry into view. */
  reveal: (id: string) => void
}

/**
 * The virtualized history list (rows grouped by day). Selection and keys are owned by the parent;
 * this renders and reports pointer input. Fixed row heights: no measuring, no layout thrash.
 */
export const ClipList = forwardRef<
  ClipListHandle,
  {
    rows: ClipRow[]
    activeId: string | null
    selection?: Set<string>
    focused: boolean
    density: Density
    terms: string[]
    label: string
    listId: string
    /** Show ⌘1–9 next to the first nine entries (picker). */
    numbered?: boolean
    showNumbers?: boolean
    onPointer: (e: React.MouseEvent, id: string) => void
    onPick: (id: string) => void
    onKeyDown?: (e: React.KeyboardEvent<HTMLDivElement>) => void
    onFocusChange?: (focused: boolean) => void
    tabIndex?: number
    empty?: React.ReactNode
    className?: string
  }
>(function ClipList({ rows, activeId, selection, focused, density, terms, label, listId, numbered, showNumbers = false, onPointer, onPick, onKeyDown, onFocusChange, tabIndex, empty, className }, ref) {
  const scroller = useRef<HTMLDivElement>(null)
  const sizes = SIZES[density]
  const virtualizer = useVirtualizer({
    count: rows.length,
    getScrollElement: () => scroller.current,
    estimateSize: (i) => (rows[i].type === "header" ? sizes.header : sizes.row),
    getItemKey: (i) => rows[i].key,
    overscan: 10,
    paddingStart: 2,
    paddingEnd: 8,
  })
  const rowIndex = useMemo(() => {
    const m = new Map<string, number>()
    rows.forEach((r, i) => r.type === "entry" && m.set(r.entry.id, i))
    return m
  }, [rows])

  useImperativeHandle(ref, () => ({
    focus: () => scroller.current?.focus(),
    reveal: (id) => {
      const i = rowIndex.get(id)
      if (i === undefined) return
      // Keep the section header visible when the first row of a section is reached.
      const prev = rows[i - 1]
      virtualizer.scrollToIndex(prev?.type === "header" && i - 1 === 0 ? 0 : i, { align: "auto" })
    },
  }))

  useEffect(() => {
    if (!activeId) return
    const i = rowIndex.get(activeId)
    if (i === undefined) return
    const prev = rows[i - 1]
    virtualizer.scrollToIndex(prev?.type === "header" ? i - 1 : i, { align: "auto" })
  }, [activeId, rowIndex, rows, virtualizer])

  return (
    <div
      ref={scroller}
      id={listId}
      role="listbox"
      aria-label={label}
      aria-multiselectable={selection ? true : undefined}
      aria-activedescendant={activeId && rowIndex.has(activeId) ? `clip-${activeId}` : undefined}
      tabIndex={tabIndex}
      onKeyDown={onKeyDown}
      onFocus={() => onFocusChange?.(true)}
      onBlur={() => onFocusChange?.(false)}
      className={cn("relative min-h-0 flex-1 overflow-y-auto outline-none [contain:strict]", className)}
    >
      {rows.length === 0 ? (
        empty
      ) : (
        <div style={{ height: virtualizer.getTotalSize(), position: "relative" }}>
          {virtualizer.getVirtualItems().map((v) => {
            const r = rows[v.index]
            return (
              <div key={v.key} style={{ position: "absolute", top: 0, left: 0, right: 0, height: v.size, transform: `translateY(${v.start}px)` }}>
                {r.type === "header" ? (
                  <div role="presentation" className="flex h-full items-end gap-1.5 px-4 pb-1.5 text-[11px] font-medium text-faint">
                    <span>{r.label}</span>
                    <span className="tabular opacity-70">{r.count}</span>
                  </div>
                ) : (
                  <div className="h-full py-[1px]">
                    <Row
                      entry={r.entry}
                      active={r.entry.id === activeId}
                      selected={!!selection && selection.size > 1 && selection.has(r.entry.id)}
                      focused={focused}
                      density={density}
                      terms={terms}
                      number={numbered && r.index < 9 ? r.index + 1 : null}
                      showNumbers={showNumbers}
                      dated={r.dated}
                      onPointer={onPointer}
                      onPick={onPick}
                    />
                  </div>
                )}
              </div>
            )
          })}
        </div>
      )}
    </div>
  )
})
