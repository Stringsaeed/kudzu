"use client"

import { ChevronRight, Pin } from "lucide-react"
import { useMemo, useState } from "react"

import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { clipTime, exactTime, expiresIn, formatBytes, KIND_LABEL, wordCount } from "@/lib/clipboard"
import type { ClipEntry } from "@/lib/types"
import { cn } from "@/lib/utils"

import { AppIcon } from "../app-icon"
import { ClipContent, useFullText } from "./clip-content"
import { SecretBadge } from "./secret"

const DAY = 86_400_000

/**
 * Every copy of this content on a track spanning the retention window (or since it was first
 * copied when history is kept forever). Hover a tick for its exact time.
 */
export function CopyTimeline({ entry, retentionDays }: { entry: ClipEntry; retentionDays: number }) {
  // The window ends when this preview opened; it remounts per item.
  const [now] = useState(() => Date.now())
  const times = useMemo(() => entry.timings.map((t) => Date.parse(t)).filter((t) => !Number.isNaN(t)), [entry.timings])
  const first = times[0] ?? Date.parse(entry.copiedAt)
  const span = retentionDays ? Math.max(retentionDays * DAY, now - first) : Math.max(DAY, now - first)
  const start = now - span
  const days = Math.max(1, Math.round(span / DAY))
  // Day separators when the window is a few weeks or less.
  const ticks = days <= 31 ? Array.from({ length: days - 1 }, (_, i) => (i + 1) / days) : []
  return (
    <figure className="flex flex-col gap-1.5" aria-label={`Copied ${entry.copyCount} time${entry.copyCount === 1 ? "" : "s"}`}>
      <div className="relative h-6 rounded-md bg-foreground/[0.03] shadow-well">
        {ticks.map((p) => (
          <span key={p} className="absolute top-1.5 bottom-1.5 w-px bg-hairline" style={{ left: `${p * 100}%` }} />
        ))}
        {times.map((t, i) => {
          const p = Math.min(1, Math.max(0, (t - start) / span))
          const latest = i === times.length - 1
          return (
            <Tooltip key={t + ":" + i}>
              <TooltipTrigger asChild>
                <span
                  className={cn("absolute top-1/2 size-2 -translate-x-1/2 -translate-y-1/2 rounded-full ring-2 ring-background transition-transform hover:scale-150", latest ? "bg-iris" : "bg-iris/45")}
                  style={{ left: `calc(${p * 100}% * 0.96 + 2%)` }}
                />
              </TooltipTrigger>
              <TooltipContent>{exactTime(new Date(t).toISOString())}</TooltipContent>
            </Tooltip>
          )
        })}
      </div>
      <figcaption className="flex justify-between text-[10.5px] text-faint tabular">
        <span>{retentionDays ? `${days} day${days === 1 ? "" : "s"} ago` : new Date(start).toLocaleDateString(undefined, { month: "short", day: "numeric" })}</span>
        <span>Now</span>
      </figcaption>
    </figure>
  )
}

function Fact({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <>
      <dt className="text-faint">{label}</dt>
      <dd className="min-w-0 truncate text-foreground/85 tabular">{children}</dd>
    </>
  )
}

function TextFacts({ entry }: { entry: ClipEntry }) {
  const full = useFullText(entry)
  const words = useMemo(() => (full === null ? null : wordCount(full)), [full])
  return (
    <Fact label="Length">
      {entry.textLen.toLocaleString()} character{entry.textLen === 1 ? "" : "s"}
      {words !== null && ` · ${words.toLocaleString()} word${words === 1 ? "" : "s"}`}
      {entry.lineCount > 1 && ` · ${entry.lineCount.toLocaleString()} lines`}
    </Fact>
  )
}

/** When, where from, how big, how long it stays — plus the raw formats for power users. */
export function ClipDetails({ entry, retentionDays, compact }: { entry: ClipEntry; retentionDays: number; compact?: boolean }) {
  const textual = entry.kind === "text" || entry.kind === "code"
  return (
    <div className="flex flex-col gap-4">
      {!compact && entry.copyCount > 1 && (
        <section aria-label="Copy activity" className="flex flex-col gap-2">
          <h3 className="text-[11px] font-medium text-faint">
            Copied {entry.copyCount} times
          </h3>
          <CopyTimeline entry={entry} retentionDays={retentionDays} />
        </section>
      )}
      <dl className={cn("grid grid-cols-[auto_minmax(0,1fr)] items-baseline gap-x-5 gap-y-1.5", compact ? "text-[11.5px]" : "text-xs")}>
        <Fact label="Copied">{exactTime(entry.copiedAt)}</Fact>
        {entry.copyCount > 1 && <Fact label="First copied">{exactTime(entry.firstCopiedAt)}</Fact>}
        {compact && entry.copyCount > 1 && <Fact label="Times">{entry.copyCount}×</Fact>}
        {entry.source && (
          <>
            <dt className="text-faint">From</dt>
            <dd className="flex min-w-0 items-center gap-1.5 text-foreground/85">
              <AppIcon path={entry.source.path} name={entry.source.name} size={14} />
              <span className="truncate">{entry.source.name}</span>
            </dd>
          </>
        )}
        {textual && <TextFacts entry={entry} />}
        {entry.image && <Fact label="Dimensions">{`${entry.image.width} × ${entry.image.height} px`}</Fact>}
        {entry.kind === "file" && <Fact label="Items">{entry.files?.length ?? 0}</Fact>}
        {(entry.kind === "image" || entry.kind === "other") && <Fact label="Size">{formatBytes(entry.size)}</Fact>}
        {entry.useCount > 0 && entry.lastUsedAt && (
          <Fact label="Reused">
            {entry.useCount}× · last {clipTime(entry.lastUsedAt).toLowerCase()}
          </Fact>
        )}
        <Fact label="Expires">
          <span className={cn(entry.pinned && "text-iris-foreground")}>{expiresIn(entry, retentionDays)}</span>
        </Fact>
      </dl>
      {!compact && entry.types.length > 0 && (
        <details className="group/fmt text-xs">
          <summary className="flex w-fit cursor-default list-none items-center gap-1 rounded text-faint outline-none hover:text-muted-foreground focus-visible:ring-2 focus-visible:ring-ring [&::-webkit-details-marker]:hidden">
            <ChevronRight className="size-3 transition-[rotate] duration-(--motion-moderate) ease-spring group-open/fmt:rotate-90" />
            {entry.types.length} format{entry.types.length === 1 ? "" : "s"}
            {entry.rich && <span className="ml-1 rounded bg-foreground/[0.06] px-1 text-[10px]">Formatted</span>}
          </summary>
          <ul className="selectable mt-2 flex flex-col gap-0.5 pl-4 font-mono text-[11px] text-muted-foreground">
            {entry.types.map((t) => (
              <li key={t} className="truncate">
                {t}
              </li>
            ))}
          </ul>
        </details>
      )}
    </div>
  )
}

/** Header line: kind, pin state and the source app. */
export function ClipHeading({ entry }: { entry: ClipEntry }) {
  return (
    <div className="flex min-w-0 items-center gap-2 text-xs">
      <span className="rounded-md bg-foreground/[0.06] px-1.5 py-0.5 text-[11px] font-medium text-muted-foreground">{KIND_LABEL[entry.kind]}</span>
      {entry.rich && <span className="rounded-md bg-foreground/[0.04] px-1.5 py-0.5 text-[11px] text-faint">Formatted</span>}
      <SecretBadge entry={entry} />
      {entry.pinned && (
        <span className="flex items-center gap-1 rounded-md bg-iris-soft px-1.5 py-0.5 text-[11px] font-medium text-iris-foreground">
          <Pin className="size-3 -rotate-45 fill-current" /> Pinned
        </span>
      )}
      <span className="ml-auto truncate text-faint tabular">{clipTime(entry.copiedAt)}</span>
    </div>
  )
}

export { ClipContent }
