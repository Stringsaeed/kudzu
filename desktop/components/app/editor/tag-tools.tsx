"use client"

// The two ways to work with template tags without knowing the syntax:
//   • InsertMenu — "/" in the editor or the Insert button: searchable list of variables.
//   • ChipEditor — click any chip: a small form for that variable, applied live.

import {
  CalendarDays,
  ChevronDown,
  Clipboard,
  Clock,
  CornerDownRight,
  FileText,
  ListChecks,
  TextCursor,
  TextCursorInput,
  Trash2,
  TriangleAlert,
} from "lucide-react"
import { useEffect, useMemo, useState } from "react"

import { Command, CommandEmpty, CommandGroup, CommandInput, CommandItem, CommandList } from "@/components/ui/command"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { Popover, PopoverAnchor, PopoverContent } from "@/components/ui/popover"
import { Switch } from "@/components/ui/switch"
import { Textarea } from "@/components/ui/textarea"
import { Button } from "@/components/ui/button"
import { formatIcu } from "@/lib/render-local"
import { buildTag, DATE_FORMAT_PRESETS, joinOptions, splitOptions, type TagToken } from "@/lib/template"
import type { Snippet } from "@/lib/types"
import { cn } from "@/lib/utils"

import { Shortcut } from "../shortcut"
import { TriggerChip } from "../trigger-chip"

export interface InsertChoice {
  text: string
  /** Open the chip editor on the inserted tag right away. */
  configure: boolean
}

const VARIABLES = [
  { id: "date", icon: CalendarDays, label: "Date", hint: "Today, or any day relative to it", text: buildTag("date", { format: "MMMM d, yyyy" }), configure: false, keywords: "today day calendar" },
  { id: "time", icon: Clock, label: "Time", hint: "The current time", text: buildTag("time", { format: "h:mm a" }), configure: false, keywords: "clock hour now" },
  { id: "clipboard", icon: Clipboard, label: "Clipboard", hint: "Whatever you last copied", text: buildTag("clipboard"), configure: false, keywords: "paste copy" },
  { id: "cursor", icon: TextCursor, label: "Cursor position", hint: "Where the caret lands after expanding", text: buildTag("cursor"), configure: false, keywords: "caret" },
  { id: "field", icon: TextCursorInput, label: "Fill-in field", hint: "Asks you for text each time", text: buildTag("field", { name: "Name" }), configure: true, keywords: "form input ask prompt variable" },
  { id: "choice", icon: ListChecks, label: "Dropdown", hint: "Pick from a list each time", text: buildTag("choice", { name: "Option", options: "Yes|No" }), configure: true, keywords: "form select list options" },
] as const

export function InsertMenu({
  open,
  onOpenChange,
  rect,
  snippets,
  excludeId,
  onPick,
}: {
  open: boolean
  onOpenChange: (open: boolean) => void
  rect: DOMRect | null
  snippets: Snippet[]
  excludeId?: string
  onPick: (choice: InsertChoice) => void
}) {
  const anchor = useMemo(() => ({ getBoundingClientRect: () => rect ?? new DOMRect() }), [rect])
  return (
    <Popover open={open} onOpenChange={onOpenChange}>
      <PopoverAnchor virtualRef={{ current: anchor }} />
      <PopoverContent align="start" sideOffset={6} className="w-80 p-0 shadow-float ring-0 backdrop-blur-xl" onCloseAutoFocus={(e) => e.preventDefault()}>
        <Command loop className="bg-transparent">
          <CommandInput placeholder="Insert a variable or snippet…" autoFocus />
          <CommandList className="max-h-80">
            <CommandEmpty>Nothing matches.</CommandEmpty>
            <CommandGroup heading="Variables">
              {VARIABLES.map((v) => (
                <CommandItem key={v.id} value={`${v.label} ${v.keywords}`} onSelect={() => onPick({ text: v.text, configure: v.configure })} className="gap-2.5 py-1.5">
                  <span className="flex size-7 shrink-0 items-center justify-center rounded-md bg-foreground/[0.06]">
                    <v.icon className="size-3.5" />
                  </span>
                  <span className="flex min-w-0 flex-col">
                    <span className="font-medium">{v.label}</span>
                    <span className="truncate text-xs text-muted-foreground">{v.hint}</span>
                  </span>
                </CommandItem>
              ))}
            </CommandGroup>
            <CommandGroup heading="Insert another snippet">
              {snippets
                .filter((s) => s.id !== excludeId)
                .slice(0, 400)
                .map((s) => (
                  <CommandItem key={s.id} value={`snippet ${s.name} ${s.triggers.join(" ")} ${s.id}`} onSelect={() => onPick({ text: buildTag("snippet", { id: s.id }), configure: false })} className="gap-2.5">
                    <CornerDownRight className="size-3.5 text-muted-foreground" />
                    <span className="truncate">{s.name || "Untitled"}</span>
                    {s.triggers[0] && <TriggerChip trigger={s.triggers[0]} muted className="ml-auto" />}
                  </CommandItem>
                ))}
            </CommandGroup>
          </CommandList>
          <div className="flex items-center gap-3 border-t px-3 py-2 text-[11px] text-muted-foreground">
            <span className="flex items-center gap-1"><Shortcut combo="enter" /> insert</span>
            <span className="flex items-center gap-1"><Shortcut combo="escape" /> close</span>
          </div>
        </Command>
      </PopoverContent>
    </Popover>
  )
}

// ---------------------------------------------------------------------------------------

const OFFSET_PRESETS = [
  { value: "", label: "Today / now" },
  { value: "+1d", label: "Tomorrow" },
  { value: "-1d", label: "Yesterday" },
  { value: "+1w", label: "In a week" },
  { value: "+1m", label: "In a month" },
  { value: "+1h", label: "In an hour" },
]

function Field({ label, children, hint }: { label: string; children: React.ReactNode; hint?: React.ReactNode }) {
  return (
    <div className="flex flex-col gap-1.5">
      <Label className="text-xs font-medium text-muted-foreground">{label}</Label>
      {children}
      {hint && <p className="text-[11px] text-faint">{hint}</p>}
    </div>
  )
}

function Segmented<T extends string>({ value, options, onChange, label }: { value: T; options: { value: T; label: string }[]; onChange: (v: T) => void; label: string }) {
  return (
    <div role="radiogroup" aria-label={label} className="flex rounded-lg bg-foreground/[0.04] p-0.5 shadow-well">
      {options.map((o) => (
        <button
          key={o.value}
          role="radio"
          aria-checked={value === o.value}
          onClick={() => onChange(o.value)}
          className={cn(
            "h-7 flex-1 rounded-md px-2 text-xs font-medium text-muted-foreground transition-colors",
            value === o.value && "bg-surface-raised text-foreground shadow-soft",
          )}
        >
          {o.label}
        </button>
      ))}
    </div>
  )
}

export function ChipEditor({
  tag,
  rect,
  onOpenChange,
  onChange,
  onRemove,
  snippets,
  excludeId,
}: {
  tag: TagToken | null
  rect: DOMRect | null
  onOpenChange: (open: boolean) => void
  onChange: (source: string) => void
  onRemove: () => void
  snippets: Snippet[]
  excludeId?: string
}) {
  const anchor = useMemo(() => ({ getBoundingClientRect: () => rect ?? new DOMRect() }), [rect])
  return (
    <Popover open={!!tag} onOpenChange={onOpenChange}>
      <PopoverAnchor virtualRef={{ current: anchor }} />
      <PopoverContent
        align="start"
        sideOffset={8}
        className="w-80 gap-0 p-0 shadow-float ring-0 backdrop-blur-xl"
        onCloseAutoFocus={(e) => e.preventDefault()}
        onKeyDown={(e) => {
          if (e.key === "Enter" && !(e.target instanceof HTMLTextAreaElement)) {
            e.preventDefault()
            onOpenChange(false)
          }
        }}
      >
        {tag && <ChipForm key={tag.from} tag={tag} onChange={onChange} onRemove={onRemove} snippets={snippets} excludeId={excludeId} />}
      </PopoverContent>
    </Popover>
  )
}

function ChipForm({ tag, onChange, onRemove, snippets, excludeId }: { tag: TagToken; onChange: (s: string) => void; onRemove: () => void; snippets: Snippet[]; excludeId?: string }) {
  const [attrs, setAttrs] = useState<Record<string, string | boolean | number | undefined>>(tag.attrs)
  const [name, setName] = useState(tag.name)
  const set = (patch: Record<string, string | boolean | number | undefined>, newName = name) => {
    const next = { ...attrs, ...patch }
    setAttrs(next)
    setName(newName)
    onChange(buildTag(newName, next))
  }

  const title = (
    {
      date: "Date",
      time: "Time",
      clipboard: "Clipboard",
      cursor: "Cursor position",
      snippet: "Nested snippet",
      field: "Fill-in field",
      choice: "Dropdown",
    } as Record<string, string>
  )[name] ?? "Unrecognised tag"

  let body: React.ReactNode
  if (tag.verbatim) {
    body = (
      <div className="flex flex-col gap-2">
        <div className="flex items-start gap-2 rounded-md bg-destructive/10 p-2.5 text-xs text-destructive">
          <TriangleAlert className="mt-0.5 size-3.5 shrink-0" />
          <span>Kudzu doesn&apos;t understand this tag, so it will be typed exactly as written.</span>
        </div>
        <code className="selectable rounded-md bg-foreground/[0.05] p-2 font-mono text-xs break-all">{tag.source}</code>
      </div>
    )
  } else if (name === "date" || name === "time") {
    body = <DateForm name={name} attrs={attrs} set={set} />
  } else if (name === "clipboard") {
    body = (
      <label className="flex items-center justify-between gap-3">
        <span className="flex flex-col">
          <span className="text-sm">Trim whitespace</span>
          <span className="text-xs text-muted-foreground">Remove spaces and line breaks at both ends</span>
        </span>
        <Switch checked={!!attrs.trim} onCheckedChange={(v) => set({ trim: v || undefined })} />
      </label>
    )
  } else if (name === "cursor") {
    body = <p className="text-xs leading-relaxed text-muted-foreground">After the text is inserted, the caret jumps here — handy for filling in the middle of a template.</p>
  } else if (name === "field") {
    body = (
      <div className="flex flex-col gap-3">
        <Field label="Label" hint="Fields with the same label share one value.">
          <Input autoFocus value={(attrs.name as string) ?? ""} onChange={(e) => set({ name: e.target.value })} className="h-8" />
        </Field>
        <Field label="Default value">
          <Input value={(attrs.default as string) ?? ""} onChange={(e) => set({ default: e.target.value || undefined })} className="h-8" placeholder="Optional" />
        </Field>
        <label className="flex items-center justify-between">
          <span className="text-sm">Multiple lines</span>
          <Switch checked={!!attrs.multiline} onCheckedChange={(v) => set({ multiline: v || undefined })} />
        </label>
      </div>
    )
  } else if (name === "choice") {
    const options = splitOptions((attrs.options as string) ?? "")
    body = (
      <div className="flex flex-col gap-3">
        <Field label="Label">
          <Input autoFocus value={(attrs.name as string) ?? ""} onChange={(e) => set({ name: e.target.value })} className="h-8" />
        </Field>
        <Field label="Options" hint="One per line.">
          <Textarea
            value={options.join("\n")}
            onChange={(e) => set({ options: joinOptions(e.target.value.split("\n")) })}
            className="min-h-20 font-mono text-xs"
          />
        </Field>
        <Field label="Default">
          <select
            className="h-8 rounded-md border bg-transparent px-2 text-sm"
            value={(attrs.default as string) ?? ""}
            onChange={(e) => set({ default: e.target.value || undefined })}
          >
            <option value="">First option</option>
            {options.filter(Boolean).map((o) => (
              <option key={o} value={o}>
                {o}
              </option>
            ))}
          </select>
        </Field>
      </div>
    )
  } else if (name === "snippet") {
    body = (
      <Command className="-m-1 bg-transparent" value={attrs.id as string}>
        <CommandInput placeholder="Find a snippet…" autoFocus />
        <CommandList className="max-h-56">
          <CommandEmpty>No snippets.</CommandEmpty>
          {snippets
            .filter((s) => s.id !== excludeId)
            .map((s) => (
              <CommandItem key={s.id} value={`${s.name} ${s.triggers.join(" ")} ${s.id}`} onSelect={() => set({ id: s.id })} data-checked={s.id === attrs.id}>
                <FileText className="size-3.5 text-muted-foreground" />
                <span className="truncate">{s.name || "Untitled"}</span>
                {s.id === attrs.id && <span className="ml-auto size-1.5 rounded-full bg-iris" aria-label="selected" />}
              </CommandItem>
            ))}
        </CommandList>
      </Command>
    )
  }

  return (
    <div className="flex flex-col">
      <div className="flex items-center gap-2 border-b px-3 py-2.5">
        <span className="text-[13px] font-semibold">{title}</span>
        <Button variant="ghost" size="icon-xs" className="ml-auto text-muted-foreground hover:text-destructive" onClick={onRemove} aria-label="Remove from snippet">
          <Trash2 />
        </Button>
      </div>
      <div className="p-3">{body}</div>
      <div className="flex items-center gap-3 border-t px-3 py-2 text-[11px] text-muted-foreground">
        <span>Changes apply instantly</span>
        <span className="ml-auto flex items-center gap-1"><Shortcut combo="enter" /> done</span>
      </div>
    </div>
  )
}

function DateForm({ name, attrs, set }: { name: string; attrs: Record<string, unknown>; set: (p: Record<string, string | undefined>, n?: string) => void }) {
  const format = (attrs.format as string) ?? (name === "date" ? "yyyy-MM-dd" : "HH:mm")
  const offset = (attrs.offset as string) ?? ""
  const isPreset = DATE_FORMAT_PRESETS.some((p) => p.format === format)
  const [custom, setCustom] = useState(!isPreset)
  const [now, setNow] = useState(() => new Date())
  useEffect(() => {
    const t = setInterval(() => setNow(new Date()), 15_000)
    return () => clearInterval(t)
  }, [])
  const example = useMemo(() => {
    try {
      return formatIcu(now, format, (attrs.locale as string | undefined)?.replace("_", "-"))
    } catch {
      return format
    }
  }, [now, format, attrs.locale])

  return (
    <div className="flex flex-col gap-3">
      <div className="rounded-lg bg-foreground/[0.04] px-3 py-2.5 text-center">
        <div className="text-[11px] tracking-wide text-faint uppercase">Example</div>
        <div className="selectable truncate font-medium tabular">{example || "—"}</div>
      </div>
      <Segmented
        label="Kind"
        value={name as "date" | "time"}
        options={[
          { value: "date", label: "Date" },
          { value: "time", label: "Time" },
        ]}
        onChange={(v) => set({}, v)}
      />
      <Field label="Format">
        <div className="relative">
          <select
            aria-label="Format"
            className="h-8 w-full appearance-none rounded-md border bg-transparent pr-7 pl-2 text-sm"
            value={custom ? "__custom" : format}
            onChange={(e) => {
              if (e.target.value === "__custom") setCustom(true)
              else {
                setCustom(false)
                set({ format: e.target.value })
              }
            }}
          >
            {DATE_FORMAT_PRESETS.map((p) => (
              <option key={p.format} value={p.format}>
                {p.label} — {formatIcu(now, p.format)}
              </option>
            ))}
            <option value="__custom">Custom pattern…</option>
          </select>
          <ChevronDown className="pointer-events-none absolute top-2 right-2 size-4 text-muted-foreground" />
        </div>
        {custom && (
          <Input autoFocus value={format} onChange={(e) => set({ format: e.target.value })} className="h-8 font-mono text-xs" placeholder="EEEE, MMM d · h:mm a" />
        )}
      </Field>
      <Field label="Shift" hint={<>Combine units, e.g. <code className="font-mono">+1w2d</code> or <code className="font-mono">-30min</code>.</>}>
        <div className="flex flex-wrap gap-1">
          {OFFSET_PRESETS.map((p) => (
            <button
              key={p.value}
              onClick={() => set({ offset: p.value || undefined })}
              aria-pressed={offset === p.value}
              className={cn(
                "h-6 rounded-md bg-surface-raised px-2 text-xs text-muted-foreground shadow-raised transition-[color,box-shadow] hover:text-foreground hover:shadow-raised-hover",
                offset === p.value && "bg-iris-soft text-iris-foreground shadow-pressed",
              )}
            >
              {p.label}
            </button>
          ))}
        </div>
        <Input value={offset} onChange={(e) => set({ offset: e.target.value || undefined })} className="h-8 font-mono text-xs" placeholder="+3d" aria-label="Custom shift" />
      </Field>
      <Field label="Language">
        <Input value={(attrs.locale as string) ?? ""} onChange={(e) => set({ locale: e.target.value || undefined })} className="h-8 font-mono text-xs" placeholder="System default (e.g. en_US, fr_FR)" />
      </Field>
    </div>
  )
}
