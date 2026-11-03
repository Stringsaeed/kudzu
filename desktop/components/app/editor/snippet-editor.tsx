"use client"

import {
  CalendarDays,
  Check,
  ChevronRight,
  Clipboard,
  Copy,
  CornerDownLeft,
  Folder,
  FolderInput,
  MoreHorizontal,
  Plus,
  Star,
  TextCursor,
  TextCursorInput,
  Trash2,
  TriangleAlert,
} from "lucide-react"
import { useCallback, useEffect, useMemo, useRef, useState } from "react"
import { toast } from "sonner"

import { Button } from "@/components/ui/button"
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuShortcut,
  DropdownMenuSub,
  DropdownMenuSubContent,
  DropdownMenuSubTrigger,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu"
import { Switch } from "@/components/ui/switch"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { useDebounced } from "@/hooks/use-debounced"
import { renderPreview } from "@/lib/backend"
import { buildTree, flatten, groupPath, isGroupEnabled } from "@/lib/groups"
import { keyLabels } from "@/lib/keys"
import { relativeTime } from "@/lib/stats"
import { useStore } from "@/lib/store"
import { analyze, buildTag, tokenize, type TagToken } from "@/lib/template"
import type { RenderResult, Snippet } from "@/lib/types"
import { cn } from "@/lib/utils"

import { Shortcut } from "../shortcut"
import { BehaviorSection } from "./behavior"
import { ChipEditor, InsertMenu, type InsertChoice } from "./tag-tools"
import { TemplateEditor, type TemplateEditorHandle } from "./template-editor"
import { TriggerInput, type TriggerInputHandle } from "./trigger-input"

const SAVE_DELAY = 350

export function SnippetEditor({ snippet, onRequestDelete }: { snippet: Snippet; onRequestDelete: (ids: string[]) => void }) {
  const library = useStore((s) => s.library)
  const settings = useStore((s) => s.settings)
  const usage = useStore((s) => s.usage.get(snippet.id))
  const conflicts = useStore((s) => s.conflicts)
  const saveSnippet = useStore((s) => s.saveSnippet)
  const duplicate = useStore((s) => s.duplicate)
  const moveSnippets = useStore((s) => s.moveSnippets)
  const focusNameOf = useStore((s) => s.focusNameOf)
  const consumeFocusName = useStore((s) => s.consumeFocusName)
  const select = useStore((s) => s.select)

  // Local draft: edits are instant here and saved after a short pause.
  const [draft, setDraft] = useState(snippet)
  const dirty = useRef(false)
  const timer = useRef<ReturnType<typeof setTimeout>>(undefined)
  const [saved, setSaved] = useState<"idle" | "saving" | "saved">("idle")
  const latest = useRef(draft)
  useEffect(() => {
    latest.current = draft
  }, [draft])

  const flush = useCallback(() => {
    clearTimeout(timer.current)
    if (!dirty.current) return
    dirty.current = false
    setSaved("saving")
    saveSnippet(latest.current).then(
      () => setSaved("saved"),
      (e) => toast.error("Couldn't save", { description: String(e) }),
    )
  }, [saveSnippet])

  // The parent keys this editor by snippet id, so switching snippets remounts it; save on the way out.
  useEffect(() => () => flush(), [flush])

  // External updates (another window, import, restore) when we have no pending edits.
  useEffect(() => {
    if (!dirty.current && snippet.updatedAt !== latest.current.updatedAt) setDraft(snippet)
  }, [snippet])

  const update = useCallback(
    (patch: Partial<Snippet>) => {
      setDraft((d) => ({ ...d, ...patch }))
      dirty.current = true
      setSaved("saving")
      clearTimeout(timer.current)
      timer.current = setTimeout(flush, SAVE_DELAY)
    },
    [flush],
  )

  useEffect(() => {
    const onHide = () => flush()
    window.addEventListener("blur", onHide)
    window.addEventListener("beforeunload", onHide)
    return () => {
      window.removeEventListener("blur", onHide)
      window.removeEventListener("beforeunload", onHide)
    }
  }, [flush])

  // Focus management
  const nameRef = useRef<HTMLInputElement>(null)
  const triggerRef = useRef<TriggerInputHandle>(null)
  const editorRef = useRef<TemplateEditorHandle>(null)
  useEffect(() => {
    if (focusNameOf === snippet.id) {
      nameRef.current?.focus()
      consumeFocusName()
    }
  }, [focusNameOf, snippet.id, consumeFocusName])

  const snippetName = useCallback((id: string) => library.snippets.find((s) => s.id === id)?.name || undefined, [library.snippets])
  const resolve = useCallback((id: string) => library.snippets.find((s) => s.id === id)?.content, [library.snippets])

  // Tag tools
  const [menu, setMenu] = useState<{ rect: DOMRect; slashAt: number | null } | null>(null)
  const [chip, setChip] = useState<{ tag: TagToken; rect: DOMRect; from: number; to: number } | null>(null)

  const openChipAt = useCallback((from: number) => {
    requestAnimationFrame(() => {
      const view = editorRef.current?.view()
      if (!view) return
      const tag = tokenize(view.state.doc.toString()).find((t) => t.kind === "tag" && t.from === from) as TagToken | undefined
      const rect = editorRef.current?.coordsAt(from)
      if (tag && rect) setChip({ tag, rect, from: tag.from, to: tag.to })
    })
  }, [])

  const pick = (choice: InsertChoice) => {
    const ed = editorRef.current
    if (!ed) return
    const slash = menu?.slashAt
    setMenu(null)
    const range = slash != null ? ed.replace(slash, slash + 1, choice.text) : ed.insert(choice.text)
    if (choice.configure) openChipAt(range.from)
  }

  // Conflicts for this snippet
  const myConflicts = useMemo(
    () => conflicts.filter((c) => c.snippetId === snippet.id || c.otherSnippetId === snippet.id),
    [conflicts, snippet.id],
  )
  const conflicted = useMemo(() => {
    const set = new Set<string>()
    for (const c of myConflicts) set.add((c.snippetId === snippet.id ? c.trigger : c.otherTrigger).toLowerCase())
    return set
  }, [myConflicts, snippet.id])

  // Preview
  const debouncedContent = useDebounced(draft.content, 120)
  const [preview, setPreview] = useState<RenderResult | null>(null)
  useEffect(() => {
    let live = true
    renderPreview(debouncedContent, snippet.id, {}, resolve).then((r) => live && setPreview(r))
    return () => {
      live = false
    }
  }, [debouncedContent, snippet.id, resolve])
  const facts = useMemo(() => analyze(draft.content), [draft.content])

  const groups = library.groups
  const flatGroups = useMemo(() => flatten(buildTree(groups)), [groups])
  const path = groupPath(groups, draft.groupID)
  const groupDisabled = !isGroupEnabled(groups, draft.groupID)

  const copyText = async () => {
    if (!preview) return
    await navigator.clipboard.writeText(preview.text)
    toast.success("Copied expanded text")
  }

  return (
    <div className="flex h-full min-h-0 flex-col" onKeyDownCapture={(e) => {
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "s") {
        e.preventDefault()
        flush()
      }
    }}>
      {/* Toolbar */}
      <header className="drag flex h-12 shrink-0 items-center gap-2 border-b border-hairline px-4">
        <DropdownMenu>
          <DropdownMenuTrigger asChild>
            <button className="no-drag flex min-w-0 items-center gap-1 rounded-md px-1.5 py-1 text-xs text-muted-foreground transition-colors hover:bg-accent hover:text-foreground" aria-label="Move to group">
              <Folder className="size-3.5 shrink-0" />
              {path.length ? (
                path.map((p, i) => (
                  <span key={i} className="flex min-w-0 items-center gap-1">
                    {i > 0 && <ChevronRight className="size-3 opacity-50" />}
                    <span className="truncate">{p}</span>
                  </span>
                ))
              ) : (
                <span>No group</span>
              )}
            </button>
          </DropdownMenuTrigger>
          <DropdownMenuContent align="start" className="w-56">
            <DropdownMenuLabel>Move to</DropdownMenuLabel>
            <DropdownMenuItem onSelect={() => update({ groupID: undefined })}>
              No group {!draft.groupID && <Check className="ml-auto" />}
            </DropdownMenuItem>
            {flatGroups.map((n) => (
              <DropdownMenuItem key={n.group.id} onSelect={() => update({ groupID: n.group.id })} style={{ paddingLeft: 8 + n.depth * 14 }}>
                <Folder className="opacity-60" /> {n.group.name}
                {draft.groupID === n.group.id && <Check className="ml-auto" />}
              </DropdownMenuItem>
            ))}
          </DropdownMenuContent>
        </DropdownMenu>

        <div className="ml-auto flex items-center gap-1 no-drag">
          <span aria-live="polite" className={cn("mr-2 text-[11px] text-faint transition-opacity", saved === "idle" && "opacity-0")}>
            {saved === "saving" ? "Saving…" : "Saved"}
          </span>
          <Tooltip>
            <TooltipTrigger asChild>
              <Button
                variant="ghost"
                size="icon-sm"
                aria-pressed={draft.isFavorite}
                aria-label={draft.isFavorite ? "Remove from favorites" : "Add to favorites"}
                onClick={() => update({ isFavorite: !draft.isFavorite })}
                className={cn(draft.isFavorite && "text-favorite hover:text-favorite")}
              >
                <Star className={cn(draft.isFavorite && "fill-current")} />
              </Button>
            </TooltipTrigger>
            <TooltipContent>Favorite <Shortcut combo="mod+shift+f" /></TooltipContent>
          </Tooltip>
          <Tooltip>
            <TooltipTrigger asChild>
              <label className="flex h-7 items-center gap-2 rounded-md px-2 text-xs text-muted-foreground">
                <Switch checked={draft.isEnabled} onCheckedChange={(isEnabled) => update({ isEnabled })} aria-label="Enabled" size="sm" />
                <span className="w-12">{draft.isEnabled ? "Enabled" : "Off"}</span>
              </label>
            </TooltipTrigger>
            <TooltipContent>Turn this snippet on or off <Shortcut combo="mod+shift+e" /></TooltipContent>
          </Tooltip>
          <DropdownMenu>
            <DropdownMenuTrigger asChild>
              <Button variant="ghost" size="icon-sm" aria-label="More actions">
                <MoreHorizontal />
              </Button>
            </DropdownMenuTrigger>
            <DropdownMenuContent align="end" className="w-60">
              <DropdownMenuItem onSelect={copyText}>
                <Clipboard /> Copy expanded text
              </DropdownMenuItem>
              <DropdownMenuItem onSelect={() => duplicate(snippet.id)}>
                <Copy /> Duplicate <DropdownMenuShortcut>{keyLabels("mod+d").join("")}</DropdownMenuShortcut>
              </DropdownMenuItem>
              <DropdownMenuSub>
                <DropdownMenuSubTrigger>
                  <FolderInput /> Move to group
                </DropdownMenuSubTrigger>
                <DropdownMenuSubContent className="w-52">
                  <DropdownMenuItem onSelect={() => moveSnippets([snippet.id], null)}>No group</DropdownMenuItem>
                  {flatGroups.map((n) => (
                    <DropdownMenuItem key={n.group.id} onSelect={() => moveSnippets([snippet.id], n.group.id)} style={{ paddingLeft: 8 + n.depth * 14 }}>
                      {n.group.name}
                    </DropdownMenuItem>
                  ))}
                </DropdownMenuSubContent>
              </DropdownMenuSub>
              <DropdownMenuSeparator />
              <DropdownMenuItem variant="destructive" onSelect={() => onRequestDelete([snippet.id])}>
                <Trash2 /> Delete <DropdownMenuShortcut>{keyLabels("mod+backspace").join("")}</DropdownMenuShortcut>
              </DropdownMenuItem>
            </DropdownMenuContent>
          </DropdownMenu>
        </div>
      </header>

      {/* Body */}
      <div className="min-h-0 flex-1 overflow-y-auto" data-editor-scroll>
        <div className="mx-auto flex max-w-[760px] flex-col gap-6 px-8 pt-7 pb-16 animate-rise" key={snippet.id}>
          {(!draft.isEnabled || groupDisabled) && (
            <div className="flex items-center gap-2 rounded-lg bg-foreground/[0.04] px-3 py-2 text-xs text-muted-foreground">
              <span className="size-1.5 rounded-full bg-faint" />
              {draft.isEnabled ? `This snippet's group “${path.join(" › ")}” is turned off, so it won't expand.` : "This snippet is turned off. It still shows in search but won't expand."}
            </div>
          )}

          <div className="flex flex-col gap-4">
            <input
              ref={nameRef}
              value={draft.name}
              onChange={(e) => update({ name: e.target.value })}
              onKeyDown={(e) => {
                if (e.key === "Enter") {
                  e.preventDefault()
                  triggerRef.current?.focus()
                }
              }}
              placeholder="Untitled snippet"
              aria-label="Snippet name"
              className="w-full bg-transparent text-[22px] leading-tight font-semibold tracking-[-0.015em] outline-none placeholder:text-faint/70"
            />
            <div className="flex flex-col gap-1.5">
              <label htmlFor="trigger-input" className="text-xs font-medium text-muted-foreground">
                Triggers <span className="font-normal text-faint">— type any of these to expand</span>
              </label>
              <TriggerInput
                ref={triggerRef}
                id="trigger-input"
                triggers={draft.triggers}
                onChange={(triggers) => update({ triggers })}
                caseSensitive={draft.caseMode === "exact"}
                prefix={settings.defaultTriggerPrefix}
                conflicted={conflicted}
              />
              {draft.triggers.length === 0 && draft.content && (
                <p className="text-xs text-muted-foreground">Add a trigger so this snippet can expand as you type. You can still insert it from search.</p>
              )}
              {myConflicts.map((c, i) => {
                const mine = c.snippetId === snippet.id
                const otherId = mine ? c.otherSnippetId : c.snippetId
                const other = library.snippets.find((s) => s.id === otherId)
                const message =
                  c.kind === "duplicate"
                    ? `“${mine ? c.trigger : c.otherTrigger}” is also used by`
                    : mine
                      ? `“${c.trigger}” fires before “${c.otherTrigger}” can be finished in`
                      : `“${c.otherTrigger}” can't be typed in full — “${c.trigger}” fires first in`
                return (
                  <p key={i} className="flex flex-wrap items-center gap-1 text-xs text-warning">
                    <TriangleAlert className="size-3.5" />
                    {message}
                    <button className="font-medium underline underline-offset-2" onClick={() => select(otherId)}>
                      {other?.name || "another snippet"}
                    </button>
                  </p>
                )
              })}
            </div>
          </div>

          {/* Content */}
          <section aria-label="Content" className="flex flex-col gap-1.5">
            <div className="flex items-center gap-2">
              <span className="text-xs font-medium text-muted-foreground">Expands to</span>
              <div className="ml-auto flex items-center gap-0.5">
                {[
                  { icon: CalendarDays, label: "Date", text: buildTag("date", { format: "MMMM d, yyyy" }), configure: false },
                  { icon: Clipboard, label: "Clipboard", text: buildTag("clipboard"), configure: false },
                  { icon: TextCursor, label: "Cursor", text: buildTag("cursor"), configure: false },
                  { icon: TextCursorInput, label: "Fill-in", text: buildTag("field", { name: "Name" }), configure: true },
                ].map((b) => (
                  <Tooltip key={b.label}>
                    <TooltipTrigger asChild>
                      <Button variant="ghost" size="icon-xs" className="text-muted-foreground" onClick={() => pick({ text: b.text, configure: b.configure })} aria-label={`Insert ${b.label}`}>
                        <b.icon />
                      </Button>
                    </TooltipTrigger>
                    <TooltipContent>Insert {b.label.toLowerCase()}</TooltipContent>
                  </Tooltip>
                ))}
                <Button
                  variant="ghost"
                  size="xs"
                  className="text-muted-foreground"
                  onClick={(e) => setMenu({ rect: e.currentTarget.getBoundingClientRect(), slashAt: null })}
                >
                  <Plus /> Insert <Shortcut keys={["/"]} className="ml-0.5" />
                </Button>
              </div>
            </div>
            <div className="rounded-xl bg-surface shadow-card transition-shadow focus-within:shadow-[0_0_0_1px_var(--ring),0_0_0_4px_var(--iris-soft)]">
              <TemplateEditor
                ref={editorRef}
                value={draft.content}
                onChange={(content) => update({ content })}
                snippetName={snippetName}
                onChipClick={({ tag, rect }) => setChip({ tag, rect, from: tag.from, to: tag.to })}
                onSlash={({ pos, rect }) => setMenu({ rect, slashAt: pos })}
                placeholder="Type the text to expand. Press / to add a date, clipboard, fill-in field…"
                className="min-h-[168px]"
              />
            </div>
            <InsertMenu
              open={!!menu}
              rect={menu?.rect ?? null}
              onOpenChange={(o) => {
                if (!o) {
                  setMenu(null)
                  editorRef.current?.focus()
                }
              }}
              snippets={library.snippets}
              excludeId={snippet.id}
              onPick={pick}
            />
            <ChipEditor
              tag={chip?.tag ?? null}
              rect={chip?.rect ?? null}
              snippets={library.snippets}
              excludeId={snippet.id}
              onOpenChange={(o) => {
                if (!o) {
                  setChip(null)
                  editorRef.current?.focus()
                }
              }}
              onChange={(source) => {
                if (!chip) return
                const r = editorRef.current!.replace(chip.from, chip.to, source, false)
                setChip((c) => (c ? { ...c, from: r.from, to: r.to } : c))
              }}
              onRemove={() => {
                if (!chip) return
                editorRef.current!.replace(chip.from, chip.to, "")
                setChip(null)
              }}
            />
          </section>

          {/* Preview */}
          <section aria-label="Preview" className="flex flex-col gap-1.5">
            <div className="flex items-center gap-2 text-xs">
              <span className="font-medium text-muted-foreground">Preview</span>
              {facts.isForm && <span className="rounded bg-warning/12 px-1.5 py-px text-[10.5px] font-medium text-warning">Asks for {facts.fieldNames.join(", ")}</span>}
              {facts.usesClipboard && <span className="rounded bg-success/12 px-1.5 py-px text-[10.5px] font-medium text-success">Uses clipboard</span>}
            </div>
            <div className="selectable relative min-h-12 rounded-xl bg-foreground/[0.025] px-4 py-3 text-[13.5px] leading-relaxed whitespace-pre-wrap shadow-well">
              {preview && preview.text ? (
                <PreviewText result={preview} />
              ) : (
                <span className="text-faint">Nothing to show yet.</span>
              )}
            </div>
          </section>

          <BehaviorSection snippet={draft} groups={groups} onChange={update} />

          <footer className="flex flex-wrap gap-x-4 gap-y-1 text-[11px] text-faint tabular">
            <span>Used {usage?.count ?? 0} {usage?.count === 1 ? "time" : "times"}</span>
            {usage?.last ? <span>Last used {relativeTime(usage.last).toLowerCase()}</span> : null}
            <span>Edited {relativeTime(Date.parse(draft.updatedAt)).toLowerCase()}</span>
            <span>Created {new Date(draft.createdAt).toLocaleDateString(undefined, { dateStyle: "medium" })}</span>
          </footer>
        </div>
      </div>
    </div>
  )
}

function PreviewText({ result }: { result: RenderResult }) {
  const { text, cursorOffset } = result
  if (cursorOffset == null) return <>{text}</>
  return (
    <>
      {text.slice(0, cursorOffset)}
      <span aria-label="cursor" className="relative mx-px inline-block h-[1.1em] w-[2px] translate-y-[3px] animate-pulse rounded-full bg-iris" />
      {text.slice(cursorOffset)}
    </>
  )
}

export function EditorEmpty({ onNew }: { onNew: () => void }) {
  return (
    <div className="drag flex h-full flex-col items-center justify-center gap-4 p-8 text-center">
      <div className="flex size-12 items-center justify-center rounded-2xl bg-foreground/[0.04] shadow-hairline">
        <CornerDownLeft className="size-5 text-muted-foreground" />
      </div>
      <div className="flex flex-col gap-1">
        <p className="font-medium">No snippet selected</p>
        <p className="text-xs text-muted-foreground">Pick one from the list, or start a new one.</p>
      </div>
      <Button size="sm" onClick={onNew} className="no-drag">
        <Plus /> New snippet <Shortcut combo="mod+n" />
      </Button>
    </div>
  )
}
