"use client"

// Settings ▸ Activity: "Why didn't this expand?". Recent expansion attempts with the outcome,
// one sentence explaining a skip, and the action that fixes it. The explanations come from
// kudzu-core (`activity::explain`); this file only renders them and runs the fix.

import { Check, Keyboard, ListX, ShieldAlert } from "lucide-react"
import { useState } from "react"

import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogTitle } from "@/components/ui/dialog"
import { activityTime, useActivity } from "@/lib/activity"
import { api } from "@/lib/backend"
import { acceleratorLabels } from "@/lib/keys"
import { useStore } from "@/lib/store"
import type { ActivityEntry, ActivityExplanation, Group } from "@/lib/types"
import { cn } from "@/lib/utils"

import { AppIcon } from "../app-icon"
import { Shortcut } from "../shortcut"
import { TriggerChip } from "../trigger-chip"
import { health, type Health } from "./status"

const TONE: Record<ActivityExplanation["tone"], string> = {
  ok: "bg-success/15 text-success",
  info: "bg-foreground/[0.06] text-muted-foreground",
  skipped: "bg-warning/15 text-warning",
  blocked: "bg-destructive/10 text-destructive dark:bg-destructive/20",
  failed: "bg-destructive/10 text-destructive dark:bg-destructive/20",
}

const SOURCE_LABEL = { typed: null, quickSearch: "Quick Search", form: "Form" } as const

/** What a fix button does right now, or `null` when its cause is already gone. */
interface FixAction {
  label: string
  run: () => void
}

function useFix(entry: ActivityEntry, onOpenGroupSettings: ((group: Group) => void) | undefined, explainSecureInput: (owner: string | null) => void): FixAction | "resolved" | null {
  const snippet = useStore((s) => (entry.snippetId ? s.library.snippets.find((x) => x.id === entry.snippetId) : undefined))
  const groups = useStore((s) => s.library.groups)
  const paused = useStore((s) => s.status.paused)
  const fix = entry.explanation.fix
  if (!fix) return null
  const st = useStore.getState
  switch (fix.kind) {
    case "resume":
      return paused ? { label: "Resume Kudzu", run: () => void st().setPaused(false) } : "resolved"
    case "turnOnSnippet":
      if (!snippet) return null
      return snippet.isEnabled ? "resolved" : { label: "Turn on", run: () => void st().toggleEnabled(snippet.id) }
    case "openSnippet":
      if (!snippet) return null
      return {
        label: "Snippet rules",
        run: () => useStore.setState({ selectedId: snippet.id, filter: "", view: snippet.groupID ? { kind: "group", id: snippet.groupID } : { kind: "all" } }),
      }
    case "openGroup": {
      const group = groups.find((g) => g.id === fix.groupId)
      if (!group || !onOpenGroupSettings) return null
      if (entry.outcome.kind === "groupOff" && group.isEnabled) return "resolved"
      return { label: entry.outcome.kind === "groupOff" ? "Group settings" : "Group rules", run: () => onOpenGroupSettings(group) }
    }
    case "openAppSettings":
      return { label: "App settings", run: () => st().setView({ kind: "settings", section: "apps" }) }
    case "openPermissions":
      return { label: "Fix permission", run: () => void api.showPermissions(fix.permission) }
    case "explainSecureInput":
      return { label: "What's this?", run: () => explainSecureInput(entry.outcome.kind === "secureInput" ? (entry.outcome.owner ?? null) : null) }
  }
}

function Row({ entry, onOpenGroupSettings, explainSecureInput }: { entry: ActivityEntry; onOpenGroupSettings?: (group: Group) => void; explainSecureInput: (owner: string | null) => void }) {
  const snippet = useStore((s) => (entry.snippetId ? s.library.snippets.find((x) => x.id === entry.snippetId) : undefined))
  const fix = useFix(entry, onOpenGroupSettings, explainSecureInput)
  const { explanation } = entry
  const isHealth = !entry.snippetId
  const name = isHealth ? "Keyboard listener" : snippet ? snippet.name || "Untitled" : "Deleted snippet"
  const source = entry.source ? SOURCE_LABEL[entry.source] : null

  return (
    <li className="grid grid-cols-[4.25rem_minmax(0,1fr)_auto] items-start gap-x-3 py-3">
      <time dateTime={entry.at} title={new Date(entry.at).toLocaleString()} className="pt-0.5 text-[11px] text-faint tabular-nums">
        {activityTime(entry.at)}
      </time>
      <div className="flex min-w-0 flex-col gap-1">
        <div className="flex min-w-0 items-center gap-2">
          {isHealth && <Keyboard className="size-3.5 shrink-0 text-muted-foreground" aria-hidden />}
          <span className={cn("truncate text-[13px] font-medium", !isHealth && !snippet && "text-muted-foreground")}>{name}</span>
          {entry.trigger && <TriggerChip trigger={entry.trigger} muted={explanation.tone !== "ok"} className="shrink-0" />}
          {source && <span className="shrink-0 text-[11px] text-faint">{source}</span>}
        </div>
        {explanation.sentence && <p className="text-xs leading-relaxed text-muted-foreground">{explanation.sentence}</p>}
        {entry.app && (
          <div className="flex min-w-0 items-center gap-1.5 text-[11px] text-faint">
            <AppIcon path={entry.app.path} name={entry.app.name || entry.app.bundleId} size={14} />
            <span className="truncate">{entry.app.name || entry.app.bundleId}</span>
          </div>
        )}
      </div>
      <div className="flex flex-col items-end gap-2">
        <Badge variant="ghost" className={cn("pointer-events-none", TONE[explanation.tone])}>
          {explanation.label}
        </Badge>
        {fix === "resolved" ? (
          <span className="flex items-center gap-1 text-[11px] text-faint">
            <Check className="size-3" aria-hidden /> Fixed since
          </span>
        ) : (
          fix && (
            <Button variant="outline" size="xs" onClick={fix.run}>
              {fix.label}
            </Button>
          )
        )}
      </div>
    </li>
  )
}

const HEALTH_COPY: Record<Exclude<Health, "ready">, { title: string; detail: string }> = {
  paused: { title: "Kudzu is paused", detail: "Nothing expands until you resume." },
  secure: { title: "Secure Input is on", detail: "macOS blocks every text expander while a password field or app holds Secure Input." },
  blocked: { title: "Kudzu isn't listening to the keyboard", detail: "Without Accessibility access it can't see triggers, so nothing is recorded here either." },
  limited: { title: "Kudzu has limited access", detail: "Grant Accessibility so it can type expansions reliably." },
}

/** What explains total silence right now, from the live engine status. */
function EngineHealth({ explainSecureInput }: { explainSecureInput: (owner: string | null) => void }) {
  const status = useStore((s) => s.status)
  const platform = useStore((s) => s.platform)
  const setPaused = useStore((s) => s.setPaused)
  const h = health(status, platform)

  if (h === "ready") {
    return (
      <div className="flex items-center gap-2.5 rounded-xl bg-surface px-4 py-3 text-xs text-muted-foreground shadow-card" role="status">
        <span className="size-1.5 shrink-0 rounded-full bg-success shadow-[0_0_8px_var(--success)]" aria-hidden />
        Kudzu is listening for your triggers in every app.
      </div>
    )
  }
  const copy = HEALTH_COPY[h]
  const detail = h === "secure" && status.secureInputOwner ? `${status.secureInputOwner} holds Secure Input, so macOS blocks every text expander until it is released.` : copy.detail
  return (
    <div className="flex items-center gap-3 rounded-xl bg-surface px-4 py-3 shadow-card" role="status">
      <ShieldAlert className="size-4 shrink-0 text-warning" aria-hidden />
      <div className="min-w-0 flex-1">
        <div className="text-[13px] font-medium">{copy.title}</div>
        <div className="text-xs text-muted-foreground">{detail}</div>
      </div>
      {h === "paused" && (
        <Button size="sm" onClick={() => void setPaused(false)} aria-keyshortcuts="Meta+Shift+P">
          Resume <Shortcut combo="mod+shift+p" />
        </Button>
      )}
      {h === "secure" && (
        <Button variant="outline" size="sm" onClick={() => explainSecureInput(status.secureInputOwner ?? null)}>
          What&apos;s this?
        </Button>
      )}
      {(h === "blocked" || h === "limited") && (
        <Button size="sm" onClick={() => void api.showPermissions("accessibility")}>
          Set up permissions
        </Button>
      )}
    </div>
  )
}

function SecureInputDialog({ owner, onClose }: { owner: string | null | undefined; onClose: () => void }) {
  return (
    <Dialog open={owner !== undefined} onOpenChange={(open) => !open && onClose()}>
      <DialogContent className="max-w-[460px] gap-4 rounded-2xl shadow-float ring-0 sm:max-w-[460px]">
        <div className="flex flex-col gap-1">
          <DialogTitle>Secure Input</DialogTitle>
          <DialogDescription>
            A macOS protection for password fields. While an app holds it, no other app can read or send keystrokes, so every text expander stops.
          </DialogDescription>
        </div>
        <div className="flex flex-col gap-2 text-xs leading-relaxed text-muted-foreground">
          <p>
            {owner ? (
              <>
                <span className="font-medium text-foreground">{owner}</span> held it when this happened.
              </>
            ) : (
              "macOS didn't say which app held it."
            )}{" "}
            It usually ends when you leave the password field.
          </p>
          <p>
            If it stays on, an app has forgotten to release it. Quit {owner ?? "the app you last typed a password in"} (terminals with “Secure Keyboard Entry” turned on, and locked password managers, are the usual causes) or lock and unlock your Mac.
          </p>
          <p>Kudzu never turns Secure Input off itself, and nothing you type while it is on is seen or recorded.</p>
        </div>
        <DialogFooter>
          <Button onClick={onClose}>Got it</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}

export function Activity({ onOpenGroupSettings }: { onOpenGroupSettings?: (group: Group) => void }) {
  const { entries, ready, clear } = useActivity()
  const hotkey = useStore((s) => s.settings.paletteHotkey)
  // `undefined` closed; `null` open without a known owner.
  const [secureOwner, setSecureOwner] = useState<string | null | undefined>(undefined)

  return (
    <>
      <EngineHealth explainSecureInput={setSecureOwner} />
      <section className="flex flex-col gap-2" aria-label="Recent activity">
        <div className="flex items-end gap-4 px-1">
          <div className="min-w-0 flex-1">
            <h2 className="text-[13px] font-semibold">Why didn&apos;t this expand?</h2>
            <p className="text-xs text-muted-foreground">
              Only triggers typed in full are listed. What you type and what expands is never recorded, and this list is forgotten when Kudzu quits.
            </p>
          </div>
          <Button variant="outline" size="sm" onClick={clear} disabled={!entries.length}>
            Clear
          </Button>
        </div>
        {entries.length ? (
          <ul className="divide-y divide-hairline rounded-xl bg-surface px-4 shadow-card">
            {entries.map((entry) => (
              <Row key={entry.id} entry={entry} onOpenGroupSettings={onOpenGroupSettings} explainSecureInput={setSecureOwner} />
            ))}
          </ul>
        ) : (
          ready && (
            <div className="flex flex-col items-center gap-2 rounded-xl bg-surface px-6 py-12 text-center shadow-card">
              <ListX className="size-5 text-faint" aria-hidden />
              <p className="text-[13px] font-medium">Nothing yet</p>
              <p className="max-w-80 text-xs leading-relaxed text-muted-foreground">
                Type a trigger in any app, or insert a snippet with <Shortcut keys={acceleratorLabels(hotkey)} spoken />. It shows up here with what happened and why.
              </p>
            </div>
          )
        )}
      </section>
      <SecureInputDialog owner={secureOwner} onClose={() => setSecureOwner(undefined)} />
    </>
  )
}
