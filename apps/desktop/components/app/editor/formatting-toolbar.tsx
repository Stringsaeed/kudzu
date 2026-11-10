"use client"

import { isolateHistory } from "@codemirror/commands"
import type { EditorState } from "@codemirror/state"
import type { EditorView } from "@codemirror/view"
import { Bold, Italic, Link, List, ListOrdered, RemoveFormatting, Underline } from "lucide-react"
import { useRef, useState } from "react"

import { ImageTools } from "./image-tools"
import { nudge } from "./motion"
import { Segmented } from "../segmented"
import { Shortcut } from "../shortcut"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { changeFormat, formattingField, selectedRange, setFormatting, toggleList } from "@/lib/editor-formatting"
import { safeLink } from "@/lib/formatting"
import { isMac } from "@/lib/keys"
import type { FormatStyle } from "@/lib/types"

export function FormattingToolbar({ state, view }: { state: EditorState | null; view: () => EditorView | null }) {
  const [linkOpen, setLinkOpen] = useState(false)
  const [url, setUrl] = useState("")
  const [error, setError] = useState("")
  const linkInput = useRef<HTMLInputElement>(null)
  const spans = state?.field(formattingField) ?? null
  const range = state ? selectedRange(state) : { from: 0, to: 0 }
  const selected = range.from !== range.to
  const active = (kind: FormatStyle["kind"]) => spans?.some((s) => s.style.kind === kind && s.start <= range.from && s.end >= range.to) ?? false
  const apply = (style: FormatStyle | "clear") => { const editor = view(); if (editor) changeFormat(editor, style) }

  return <div className="flex flex-wrap items-center gap-0.5 border-b border-hairline px-2 py-1.5" role="group" aria-label="Text formatting">
    <Segmented
      label="Snippet format"
      value={spans === null ? "plain" : "formatted"}
      options={[
        { value: "plain", label: "Plain text" },
        { value: "formatted", label: "Formatted" },
      ]}
      className="mr-2"
      onChange={(format) => {
        const editor = view()
        if (!editor || editor.composing) return
        const formatted = format === "formatted"
        if (formatted === (spans !== null)) return
        editor.dispatch({ effects: setFormatting.of(formatted ? [] : null), annotations: isolateHistory.of("full"), userEvent: "input.format" })
        editor.focus()
      }}
    />
    {/* Switching to Formatted brings the style controls in together; `contents` keeps the toolbar's wrapping. */}
    {spans !== null && <span className="contents [&>*]:animate-rise">
      {([
        { kind: "bold", label: "Bold", icon: Bold, shortcut: "mod+b" },
        { kind: "italic", label: "Italic", icon: Italic, shortcut: "mod+i" },
        { kind: "underline", label: "Underline", icon: Underline, shortcut: "mod+u" },
      ] satisfies { kind: "bold" | "italic" | "underline"; label: string; icon: typeof Bold; shortcut: string }[]).map(({ kind, label, icon: Icon, shortcut }) =>
        <Tooltip key={kind}>
          <TooltipTrigger asChild><Button variant="ghost" size="icon-sm" aria-label={label} aria-pressed={active(kind)} disabled={!selected}
            aria-keyshortcuts={`${isMac ? "Meta" : "Control"}+${shortcut.slice(-1).toUpperCase()}`}
            className={active(kind) ? "bg-accent" : "text-muted-foreground"}
            onMouseDown={(e) => e.preventDefault()} onClick={() => apply({ kind })}><Icon /></Button></TooltipTrigger>
          <TooltipContent>{label} <Shortcut combo={shortcut} /></TooltipContent>
        </Tooltip>)}
      <Popover open={linkOpen} onOpenChange={(open) => {
        if (open) {
          const link = spans.find((s) => s.style.kind === "link" && s.start <= range.from && s.end >= range.to)
          setUrl(link?.style.kind === "link" ? link.style.url : "")
          setError("")
        }
        setLinkOpen(open)
      }}>
        <PopoverTrigger asChild><Button variant="ghost" size="icon-sm" aria-label="Add or edit link" aria-pressed={active("link")} disabled={!selected}
          className={active("link") ? "bg-accent" : "text-muted-foreground"} onMouseDown={(e) => e.preventDefault()}><Link /></Button></PopoverTrigger>
        <PopoverContent className="w-72" align="start" onCloseAutoFocus={(e) => { e.preventDefault(); view()?.focus() }}>
          <form className="flex flex-col gap-2" onSubmit={(e) => {
            e.preventDefault()
            const value = url.trim()
            if (!safeLink(value)) { setError("Use an https://, http://, or mailto: link."); nudge(linkInput.current); return }
            const existing = spans.find((s) => s.start <= range.from && s.end >= range.to && s.style.kind === "link")
            if (existing?.style.kind !== "link" || existing.style.url !== value) apply({ kind: "link", url: value })
            setLinkOpen(false)
          }}>
            <label htmlFor="snippet-link" className="text-xs font-medium">Link address</label>
            <Input ref={linkInput} id="snippet-link" value={url} onChange={(e) => setUrl(e.target.value)} placeholder="https://example.com" autoFocus aria-invalid={!!error} aria-describedby={error ? "snippet-link-error" : undefined} />
            {error && <p id="snippet-link-error" className="text-xs text-destructive animate-rise">{error}</p>}
            <Button type="submit" size="sm">Apply link</Button>
          </form>
        </PopoverContent>
      </Popover>
      <ImageTools view={view} />
      <span className="mx-1 h-4 w-px bg-border" />
      {([{ label: "Bulleted list", icon: List, ordered: false }, { label: "Numbered list", icon: ListOrdered, ordered: true }]).map(({ label, icon: Icon, ordered }) =>
        <Tooltip key={label}><TooltipTrigger asChild><Button variant="ghost" size="icon-sm" aria-label={label} className="text-muted-foreground"
          onMouseDown={(e) => e.preventDefault()} onClick={() => { const editor = view(); if (editor) toggleList(editor, ordered) }}><Icon /></Button></TooltipTrigger><TooltipContent>{label}</TooltipContent></Tooltip>)}
      <Tooltip><TooltipTrigger asChild><Button variant="ghost" size="icon-sm" aria-label="Remove formatting" disabled={!selected} className="text-muted-foreground"
        onMouseDown={(e) => e.preventDefault()} onClick={() => apply("clear")}><RemoveFormatting /></Button></TooltipTrigger><TooltipContent>Remove formatting</TooltipContent></Tooltip>
    </span>}
  </div>
}
