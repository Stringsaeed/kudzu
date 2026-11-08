"use client"

import { Check, Copy, ExternalLink, FolderOpen, Globe, Package } from "lucide-react"
import { useEffect, useMemo, useState } from "react"

import { Button } from "@/components/ui/button"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { api, clipMediaUrl } from "@/lib/backend"
import { basename, describe, dirname, loadFullText, tildify } from "@/lib/clipboard"
import type { ClipEntry } from "@/lib/types"
import { cn } from "@/lib/utils"

import { Marked } from "./clip-list"
import { checkerboard, ClipImage } from "./clip-tile"
import { Concealed } from "./secret"

/** Characters rendered before "…and N more" (keeps huge pastes from stalling layout). */
const RENDER_LIMIT = 60_000
const CODE_LINE_LIMIT = 1_500

/** The full text of an entry (fetched when the list only carries an excerpt). */
export function useFullText(entry: ClipEntry): string | null {
  const [state, setState] = useState<{ id: string; text: string } | null>(null)
  useEffect(() => {
    let live = true
    loadFullText(entry).then((text) => live && setState({ id: entry.id + entry.hash, text }))
    return () => {
      live = false
    }
  }, [entry])
  if (state?.id === entry.id + entry.hash) return state.text
  return entry.text !== undefined && entry.textLen <= entry.text.length ? entry.text : null
}

function Truncated({ hidden }: { hidden: number }) {
  if (hidden <= 0) return null
  return <div className="mt-3 text-xs text-faint">…and {hidden.toLocaleString()} more characters. Copy it to get everything.</div>
}

function TextBody({ entry, terms, large }: { entry: ClipEntry; terms: string[]; large?: boolean }) {
  const full = useFullText(entry)
  const text = full ?? entry.text ?? ""
  const shown = text.slice(0, RENDER_LIMIT)
  return (
    <div className={cn("selectable leading-relaxed break-words whitespace-pre-wrap text-foreground/90", large ? "text-[15px]" : "text-[13.5px]")}>
      <Marked text={shown} terms={terms} />
      {full === null && entry.textLen > text.length && <span className="ml-1 inline-block h-3 w-16 animate-shimmer rounded bg-[linear-gradient(90deg,transparent,color-mix(in_oklch,var(--foreground)_10%,transparent),transparent)] bg-[length:200%_100%] align-middle" />}
      <Truncated hidden={(full ? full.length : 0) - shown.length} />
    </div>
  )
}

function CodeBody({ entry, terms, large }: { entry: ClipEntry; terms: string[]; large?: boolean }) {
  const full = useFullText(entry)
  const lines = useMemo(() => (full ?? entry.text ?? "").split("\n"), [full, entry.text])
  const shown = lines.slice(0, CODE_LINE_LIMIT)
  const gutter = String(shown.length).length
  return (
    <div className={cn("selectable overflow-x-auto rounded-xl bg-foreground/[0.025] py-3 font-mono leading-[1.65] shadow-well", large ? "text-[13.5px]" : "text-[12.5px]")}>
      <table className="border-collapse">
        <tbody>
          {shown.map((line, i) => (
            <tr key={i}>
              <td aria-hidden className="pr-4 pl-3 text-right align-top text-faint/70 tabular select-none" style={{ minWidth: `${gutter + 2.5}ch` }}>
                {i + 1}
              </td>
              <td className="pr-4 whitespace-pre text-foreground/90">
                <Marked text={line || " "} terms={terms} />
              </td>
            </tr>
          ))}
        </tbody>
      </table>
      {lines.length > shown.length && <div className="px-3 pt-2 text-xs text-faint">…and {(lines.length - shown.length).toLocaleString()} more lines.</div>}
    </div>
  )
}

function LinkBody({ entry, large }: { entry: ClipEntry; large?: boolean }) {
  const url = (entry.text ?? "").trim()
  const { title } = describe(entry)
  return (
    <div className="flex flex-col gap-4">
      <div className="flex items-center gap-3">
        <span className="flex size-11 shrink-0 items-center justify-center rounded-xl bg-foreground/[0.05] text-muted-foreground shadow-[inset_0_0_0_1px_var(--hairline)]">
          <Globe className="size-5" />
        </span>
        <div className="min-w-0">
          <div className={cn("truncate font-semibold tracking-tight", large ? "text-xl" : "text-[17px]")}>{title}</div>
          <div className="text-xs text-muted-foreground">{url.toLowerCase().startsWith("mailto:") ? "Email address" : "Web link"}</div>
        </div>
      </div>
      <div className="selectable rounded-xl bg-foreground/[0.025] px-3.5 py-3 font-mono text-[12.5px] leading-relaxed break-all text-foreground/85 shadow-well">{url}</div>
      <div>
        <Button size="sm" variant="secondary" onClick={() => void api.clipboardOpen(entry.id)}>
          <ExternalLink /> {url.toLowerCase().startsWith("mailto:") ? "Write email" : "Open in browser"}
        </Button>
      </div>
    </div>
  )
}

// MARK: Colour

function srgbToLinear(c: number) {
  return c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4
}

/** Parses any CSS colour through the browser, returning 0–1 sRGB + alpha. */
function cssToRgb(color: string): [number, number, number, number] | null {
  if (typeof document === "undefined") return null
  const c = document.createElement("canvas").getContext("2d")
  if (!c) return null
  c.fillStyle = "#000"
  c.fillStyle = color
  c.fillRect(0, 0, 1, 1)
  const [r, g, b, a] = c.getImageData(0, 0, 1, 1).data
  return [r / 255, g / 255, b / 255, a / 255]
}

function formats([r, g, b, a]: [number, number, number, number]) {
  const to255 = (v: number) => Math.round(v * 255)
  const hex = `#${[r, g, b].map((v) => to255(v).toString(16).padStart(2, "0")).join("")}${a < 1 ? to255(a).toString(16).padStart(2, "0") : ""}`.toUpperCase()
  const max = Math.max(r, g, b)
  const min = Math.min(r, g, b)
  const l = (max + min) / 2
  const d = max - min
  const s = d === 0 ? 0 : d / (1 - Math.abs(2 * l - 1))
  let h = 0
  if (d) h = max === r ? ((g - b) / d) % 6 : max === g ? (b - r) / d + 2 : (r - g) / d + 4
  h = Math.round(h * 60 + 360) % 360
  // OKLCH (Björn Ottosson's matrices).
  const [lr, lg, lb] = [r, g, b].map(srgbToLinear)
  const L = Math.cbrt(0.4122214708 * lr + 0.5363325363 * lg + 0.0514459929 * lb)
  const M = Math.cbrt(0.2119034982 * lr + 0.6806995451 * lg + 0.1073969566 * lb)
  const S = Math.cbrt(0.0883024619 * lr + 0.2817188376 * lg + 0.6299787005 * lb)
  const ok = [0.2104542553 * L + 0.793617785 * M - 0.0040720468 * S, 1.9779984951 * L - 2.428592205 * M + 0.4505937099 * S, 0.0259040371 * L + 0.7827717662 * M - 0.808675766 * S]
  const C = Math.hypot(ok[1], ok[2])
  const H = ((Math.atan2(ok[2], ok[1]) * 180) / Math.PI + 360) % 360
  const alpha = a < 1 ? ` / ${Math.round(a * 100)}%` : ""
  const lum = 0.2126 * lr + 0.7152 * lg + 0.0722 * lb
  return {
    values: [
      ["HEX", hex],
      ["RGB", `rgb(${to255(r)} ${to255(g)} ${to255(b)}${alpha})`],
      ["HSL", `hsl(${h} ${Math.round(s * 100)}% ${Math.round(l * 100)}%${alpha})`],
      ["OKLCH", `oklch(${ok[0].toFixed(3)} ${C.toFixed(3)} ${C < 0.0005 ? 0 : H.toFixed(1)}${alpha})`],
    ] as [string, string][],
    onWhite: (1.05 / (lum + 0.05)).toFixed(2),
    onBlack: ((lum + 0.05) / 0.05).toFixed(2),
    light: lum > 0.4,
  }
}

function CopyValue({ label, value }: { label: string; value: string }) {
  const [done, setDone] = useState(false)
  return (
    <div className="group/v flex h-9 items-center gap-3 px-3">
      <span className="w-12 shrink-0 text-[11px] font-medium text-faint">{label}</span>
      <span className="selectable min-w-0 flex-1 truncate font-mono text-[12.5px]">{value}</span>
      <Tooltip>
        <TooltipTrigger asChild>
          <Button
            variant="ghost"
            size="icon-xs"
            aria-label={`Copy ${label} value`}
            className="opacity-0 group-hover/v:opacity-100 focus-visible:opacity-100"
            onClick={() => {
              void navigator.clipboard?.writeText(value).then(() => {
                setDone(true)
                setTimeout(() => setDone(false), 1200)
              })
            }}
          >
            {done ? <Check className="text-success" /> : <Copy />}
          </Button>
        </TooltipTrigger>
        <TooltipContent>Copy {label}</TooltipContent>
      </Tooltip>
    </div>
  )
}

function ColorBody({ entry, large }: { entry: ClipEntry; large?: boolean }) {
  const color = entry.color ?? entry.text ?? ""
  const info = useMemo(() => {
    const rgb = cssToRgb(color)
    return rgb ? formats(rgb) : null
  }, [color])
  return (
    <div className="flex flex-col gap-4">
      <div
        className={cn("relative flex items-end justify-between overflow-hidden rounded-2xl p-4 shadow-[inset_0_0_0_1px_oklch(0_0_0/0.1),inset_0_1px_0_oklch(1_0_0/0.2),var(--elev-card)]", large ? "h-64" : "h-40", checkerboard)}
      >
        <span className="absolute inset-0" style={{ background: color }} />
        {info && (
          <>
            <span className={cn("relative font-mono text-sm font-medium", info.light ? "text-black/75" : "text-white/90")}>{info.values[0][1]}</span>
            <span className="relative flex gap-1.5">
              <span className="rounded-md bg-white px-1.5 py-0.5 text-[11px] font-semibold text-black/80 tabular" title="Contrast with white text">
                <span style={{ color }}>Aa</span> {info.onWhite}
              </span>
              <span className="rounded-md bg-black px-1.5 py-0.5 text-[11px] font-semibold text-white/80 tabular" title="Contrast with black text">
                <span style={{ color }}>Aa</span> {info.onBlack}
              </span>
            </span>
          </>
        )}
      </div>
      {info && (
        <div className="divide-y divide-hairline rounded-xl bg-surface shadow-card">
          {info.values.map(([l, v]) => (
            <CopyValue key={l} label={l} value={v} />
          ))}
        </div>
      )}
    </div>
  )
}

// MARK: Media

function ImageBody({ entry, large }: { entry: ClipEntry; large?: boolean }) {
  const thumb = clipMediaUrl("thumb", entry.id)
  const full = clipMediaUrl("image", entry.id)
  const alt = `Copied image${entry.image ? `, ${entry.image.width} by ${entry.image.height} pixels` : ""}${entry.source ? `, from ${entry.source.name}` : ""}`
  const ratio = entry.image ? entry.image.width / Math.max(1, entry.image.height) : 16 / 10
  return (
    <div className={cn("flex items-center justify-center rounded-xl bg-foreground/[0.025] p-4 shadow-well", large ? "h-[min(64vh,640px)]" : "max-h-[420px] min-h-40")} style={large ? undefined : { aspectRatio: Math.max(ratio, 0.9) }}>
      {/* The checkerboard sits only behind the image, so transparency reads and opaque images stay clean. */}
      <ClipImage src={full} fit="natural" alt={alt} className={cn("rounded-[6px] shadow-[0_0_0_1px_var(--hairline),0_8px_24px_-12px_oklch(0_0_0/0.5)]", checkerboard)} fallback={<ClipImage src={thumb} fit="natural" alt={alt} className={checkerboard} />} />
    </div>
  )
}

function FileBody({ entry, large }: { entry: ClipEntry; large?: boolean }) {
  const files = entry.files ?? []
  const single = files.length === 1 && entry.image
  return (
    <div className="flex flex-col gap-3">
      {single && (
        <div className={cn("flex items-center justify-center rounded-xl bg-foreground/[0.025] p-4 shadow-well", large ? "h-[min(56vh,560px)]" : "h-56")}>
          <ClipImage src={clipMediaUrl("file", entry.id, 0)} fit="natural" className={cn("rounded-[6px] shadow-[0_0_0_1px_var(--hairline)]", checkerboard)} alt={`Preview of ${basename(files[0])}`} fallback={<ClipImage src={clipMediaUrl("thumb", entry.id)} fit="natural" className={checkerboard} />} />
        </div>
      )}
      <ul className="divide-y divide-hairline rounded-xl bg-surface shadow-card" aria-label="Copied files">
        {files.slice(0, 200).map((path, i) => (
          <li key={path + i} className="group/f flex items-center gap-3 px-3 py-2.5">
            <span className="size-8 shrink-0">
              <ClipImage src={clipMediaUrl("icon", entry.id, i)} fit="contain" fallback={<span className="block size-8 rounded-lg bg-foreground/[0.06]" />} />
            </span>
            <div className="min-w-0 flex-1">
              <div className="truncate text-[13px] font-medium">{basename(path)}</div>
              <div className="selectable truncate font-mono text-[11px] text-muted-foreground">{tildify(dirname(path))}</div>
            </div>
            <span className="flex shrink-0 gap-0.5 opacity-0 transition-opacity group-hover/f:opacity-100 focus-within:opacity-100">
              <Tooltip>
                <TooltipTrigger asChild>
                  <Button variant="ghost" size="icon-xs" aria-label={`Open ${basename(path)}`} onClick={() => void api.clipboardOpen(entry.id, i)}>
                    <ExternalLink />
                  </Button>
                </TooltipTrigger>
                <TooltipContent>Open</TooltipContent>
              </Tooltip>
              <Tooltip>
                <TooltipTrigger asChild>
                  <Button variant="ghost" size="icon-xs" aria-label={`Show ${basename(path)} in Finder`} onClick={() => void api.clipboardReveal(entry.id, i)}>
                    <FolderOpen />
                  </Button>
                </TooltipTrigger>
                <TooltipContent>Show in Finder</TooltipContent>
              </Tooltip>
            </span>
          </li>
        ))}
      </ul>
      {files.length > 200 && <p className="text-xs text-faint">…and {files.length - 200} more.</p>}
    </div>
  )
}

function OtherBody({ entry }: { entry: ClipEntry }) {
  return (
    <div className="flex flex-col items-start gap-3 rounded-xl bg-foreground/[0.025] p-4 shadow-well">
      <span className="flex size-10 items-center justify-center rounded-xl bg-foreground/[0.06] text-muted-foreground">
        <Package className="size-5" />
      </span>
      <div>
        <div className="text-[13.5px] font-medium">{entry.source ? `Something copied in ${entry.source.name}` : "App-specific data"}</div>
        <p className="mt-1 max-w-sm text-xs leading-relaxed text-muted-foreground">
          Kudzu can&apos;t show this kind of content, but it kept every format. Copy it again and it pastes {entry.source ? `into ${entry.source.name}` : "back"} exactly as before.
        </p>
      </div>
    </div>
  )
}

/** The main preview of an entry, by kind. Secrets stay covered until hovered. */
export function ClipContent({ entry, terms = [], large }: { entry: ClipEntry; terms?: string[]; large?: boolean }) {
  return (
    <Concealed entry={entry}>
      <ClipBody entry={entry} terms={terms} large={large} />
    </Concealed>
  )
}

function ClipBody({ entry, terms, large }: { entry: ClipEntry; terms: string[]; large?: boolean }) {
  switch (entry.kind) {
    case "code":
      return <CodeBody entry={entry} terms={terms} large={large} />
    case "link":
      return <LinkBody entry={entry} large={large} />
    case "color":
      return <ColorBody entry={entry} large={large} />
    case "image":
      return <ImageBody entry={entry} large={large} />
    case "file":
      return <FileBody entry={entry} large={large} />
    case "other":
      return <OtherBody entry={entry} />
    default:
      return <TextBody entry={entry} terms={terms} large={large} />
  }
}
