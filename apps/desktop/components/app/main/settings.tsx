"use client"

import { Check, Download, FolderOpen, History, Loader2, RefreshCw, RotateCcw, ShieldCheck, Sparkles, Upload, X } from "lucide-react"
import { useEffect, useState } from "react"
import { toast } from "sonner"

import brand from "@/design/brand.json"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
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
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select"
import { Switch } from "@/components/ui/switch"
import { formatBytes, RETENTION_OPTIONS, retentionLabel, useClipboard } from "@/lib/clipboard"
import { api, isTauri } from "@/lib/backend"
import { appName, useInstalledApps } from "@/lib/apps"
import { acceleratorFromEvent, acceleratorLabels } from "@/lib/keys"
import { useStore, type SettingsSection } from "@/lib/store"
import { relativeTime } from "@/lib/stats"
import { applyTheme } from "@/lib/theme"
import { openExternal, parseNotes, RELEASES_URL, useSyncStatus, useUpdateState } from "@/lib/updates"
import type { BackupInfo, EspansoDetection, InsertionMethod, ThemePreference } from "@/lib/types"
import { cn } from "@/lib/utils"

import { PermissionSetup } from "../permissions/permission-setup"
import { AppIcon } from "../app-icon"
import { Wordmark } from "../wordmark"
import { AppPicker } from "../app-picker"
import { Segmented } from "../segmented"
import { SelectionPill } from "../selection-pill"
import { Shortcut } from "../shortcut"
import { runExport, runImport } from "./data-actions"

export const SETTINGS_SECTIONS: { id: SettingsSection; label: string }[] = [
  { id: "general", label: "General" },
  { id: "expansion", label: "Expansion" },
  { id: "clipboard", label: "Clipboard" },
  { id: "apps", label: "Apps" },
  { id: "data", label: "Backups & data" },
  { id: "about", label: "About" },
]

function Group({ title, description, children }: { title: string; description?: string; children: React.ReactNode }) {
  return (
    <section className="flex flex-col gap-2" aria-label={title}>
      <div className="px-1">
        <h2 className="text-[13px] font-semibold">{title}</h2>
        {description && <p className="text-xs text-muted-foreground">{description}</p>}
      </div>
      <div className="divide-y divide-hairline rounded-xl bg-surface px-4 shadow-card">{children}</div>
    </section>
  )
}

function Row({ title, description, children, htmlFor }: { title: string; description?: React.ReactNode; children?: React.ReactNode; htmlFor?: string }) {
  return (
    <div className="flex min-h-14 items-center gap-6 py-3">
      <div className="min-w-0 flex-1">
        <label htmlFor={htmlFor} className="text-[13px] font-medium">
          {title}
        </label>
        {description && <div className="text-xs text-muted-foreground">{description}</div>}
      </div>
      <div className="flex shrink-0 items-center gap-2">{children}</div>
    </div>
  )
}

export function HotkeyRecorder({ value, onChange, label = "Quick search shortcut" }: { value: string; onChange: (accel: string) => void; label?: string }) {
  const [recording, setRecording] = useState(false)
  return (
    <button
      type="button"
      aria-label={recording ? "Press the new shortcut, or Escape to cancel" : `${label}: ${acceleratorLabels(value).join(" ")}. Press to change.`}
      onClick={() => setRecording(true)}
      onBlur={() => setRecording(false)}
      onKeyDown={(e) => {
        if (!recording) return
        e.preventDefault()
        e.stopPropagation()
        if (e.key === "Escape") return setRecording(false)
        const accel = acceleratorFromEvent(e.nativeEvent)
        if (accel) {
          onChange(accel)
          setRecording(false)
        }
      }}
      className={cn(
        "flex h-8 min-w-36 items-center justify-center gap-1 rounded-lg px-3 text-xs transition",
        recording ? "bg-iris-soft text-iris-foreground ring-2 ring-iris/60" : "bg-surface-raised shadow-raised sheen hover:shadow-raised-hover active:shadow-pressed",
      )}
    >
      {recording ? (
        <span className="animate-pulse">Press shortcut…</span>
      ) : (
        <Shortcut keys={acceleratorLabels(value)} size="md" />
      )}
    </button>
  )
}

export function SettingsView({ section = "general" }: { section?: SettingsSection }) {
  const setView = useStore((s) => s.setView)
  const setSection = (section: SettingsSection) => setView({ kind: "settings", section })

  return (
    <div className="flex h-full min-h-0 flex-1">
      <nav aria-label="Settings sections" className="relative isolate flex w-48 shrink-0 flex-col gap-px border-r border-hairline px-2.5">
        <SelectionPill className="rounded-md bg-accent shadow-selected" />
        <div className="drag flex h-12 shrink-0 items-center px-2.5" data-tauri-drag-region>
          <h1 className="text-[13px] font-semibold" data-tauri-drag-region>
            Settings
          </h1>
        </div>
        {SETTINGS_SECTIONS.map((s) => (
          <button
            key={s.id}
            aria-current={section === s.id ? "page" : undefined}
            onClick={() => setSection(s.id)}
            onKeyDown={(e) => {
              if (e.key !== "ArrowDown" && e.key !== "ArrowUp") return
              e.preventDefault()
              const i = SETTINGS_SECTIONS.findIndex((x) => x.id === s.id)
              const n = SETTINGS_SECTIONS[(i + (e.key === "ArrowDown" ? 1 : -1) + SETTINGS_SECTIONS.length) % SETTINGS_SECTIONS.length]
              setSection(n.id)
              ;(e.currentTarget.parentElement?.querySelector(`[data-sec="${n.id}"]`) as HTMLElement | null)?.focus()
            }}
            data-sec={s.id}
            className={cn(
              "flex h-7 items-center rounded-md px-2.5 text-left text-[13px] text-muted-foreground press [--press-scale:0.985] hover:text-foreground",
              section === s.id ? "text-foreground" : "hover:bg-accent",
            )}
          >
            {s.label}
          </button>
        ))}
      </nav>
      <div className="flex min-w-0 flex-1 flex-col">
        <header className="drag flex h-12 shrink-0 items-center border-b border-hairline px-6" data-tauri-drag-region>
          <h2 className="text-[13px] font-semibold">{SETTINGS_SECTIONS.find((s) => s.id === section)?.label}</h2>
        </header>
        <div className="min-h-0 flex-1 overflow-y-auto">
          <div className="mx-auto flex max-w-[680px] flex-col gap-8 px-8 py-8 stagger" key={section}>
            {section === "general" && <General />}
            {section === "expansion" && <Expansion />}
            {section === "clipboard" && <ClipboardSettings />}
            {section === "apps" && <Apps />}
            {section === "data" && <Data />}
            {section === "about" && <About />}
          </div>
        </div>
      </div>
    </div>
  )
}

function General() {
  const settings = useStore((s) => s.settings)
  const status = useStore((s) => s.status)
  const platform = useStore((s) => s.platform)
  const update = useStore((s) => s.updateSettings)
  return (
    <>
      {(platform === "macos" || platform === "web") && (
        <section aria-label="Permissions" className="flex flex-col gap-2">
          <h2 className="px-1 text-[13px] font-semibold">Permissions</h2>
          <PermissionSetup status={status} />
        </section>
      )}
      <Group title="Quick search">
        <Row
          title="Shortcut"
          description={
            <>
              Opens the search tray over any app. <Shortcut combo="enter" spoken /> inserts, <Shortcut combo="mod+enter" spoken /> copies.
            </>
          }
        >
          <HotkeyRecorder
            value={settings.paletteHotkey}
            onChange={(accel) =>
              api.setPaletteHotkey(accel).then(
                (s) => {
                  useStore.setState({ settings: s })
                  toast.success("Shortcut updated")
                },
                (e) => toast.error("That shortcut is taken", { description: String(e) }),
              )
            }
          />
        </Row>
      </Group>
      <ICloudSync />

      <Group title="App">
        <Row title="Appearance">
          <Segmented<ThemePreference>
            label="Appearance"
            value={settings.theme}
            onChange={(theme) => {
              applyTheme(theme)
              void update({ theme })
            }}
            options={[
              { value: "dark", label: "Dark" },
              { value: "light", label: "Light" },
              { value: "system", label: "System" },
            ]}
          />
        </Row>
        <Row title="Open at login" description="Kudzu lives in the menu bar and starts with your Mac." htmlFor="login">
          <Switch id="login" checked={settings.launchAtLogin} onCheckedChange={(launchAtLogin) => void update({ launchAtLogin })} />
        </Row>
        {platform !== "windows" && platform !== "linux" && (
          <Row title="Show in Dock while open" description="Otherwise Kudzu only appears in the menu bar." htmlFor="dock">
            <Switch id="dock" checked={settings.showInDock} onCheckedChange={(showInDock) => void update({ showInDock })} />
          </Row>
        )}
      </Group>
    </>
  )
}

function Expansion() {
  const settings = useStore((s) => s.settings)
  const status = useStore((s) => s.status)
  const update = useStore((s) => s.updateSettings)
  const setPaused = useStore((s) => s.setPaused)
  return (
    <>
      <Group title="Expanding">
        <Row
          title="Pause all snippets"
          description={
            <>
              Handy while gaming or presenting. <Shortcut combo="mod+shift+p" spoken /> also works.
            </>
          }
          htmlFor="pause"
        >
          <Switch id="pause" checked={status.paused} onCheckedChange={(v) => void setPaused(v)} />
        </Row>
        <Row title="Trigger prefix" description="Suggested at the start of new triggers so they don't fire mid-sentence." htmlFor="prefix">
          <Input
            id="prefix"
            value={settings.defaultTriggerPrefix}
            maxLength={4}
            onChange={(e) => void update({ defaultTriggerPrefix: e.target.value })}
            className="h-8 w-20 text-center font-mono"
          />
        </Row>
        <Row title="Insert by" description="Default for every snippet. Typing is most compatible; pasting is fastest for long text.">
          <Segmented<InsertionMethod>
            label="Default insertion method"
            value={settings.defaultInsertion}
            onChange={(defaultInsertion) => void update({ defaultInsertion })}
            options={[
              { value: "automatic", label: "Automatic" },
              { value: "type", label: "Type" },
              { value: "paste", label: "Paste" },
            ]}
          />
        </Row>
      </Group>
      <Group title="Good to know">
        <div className="flex flex-col gap-2 py-3 text-xs leading-relaxed text-muted-foreground">
          <p>
            <span className="font-medium text-foreground">Undo an expansion</span> — press Backspace right after it to get your trigger back.
          </p>
          <p>
            <span className="font-medium text-foreground">Clipboard is safe</span> — when Kudzu pastes, it restores whatever you had copied, and marks its own text so clipboard managers ignore it.
          </p>
          <p>
            <span className="font-medium text-foreground">Secure Input</span> — while a password field is focused, macOS blocks all text expanders. Kudzu shows this in the menu bar.
          </p>
        </div>
      </Group>
    </>
  )
}

function ClipboardSettings() {
  const settings = useStore((s) => s.settings)
  const update = useStore((s) => s.updateSettings)
  const entries = useClipboard((s) => s.entries)
  const diskUsage = useClipboard((s) => s.diskUsage)
  const clear = useClipboard((s) => s.clear)
  const days = settings.clipboardRetentionDays
  const preset = RETENTION_OPTIONS.some((o) => o.days === days)
  const [custom, setCustom] = useState(!preset)
  const [draft, setDraft] = useState(String(days || 7))
  const [confirm, setConfirm] = useState(false)
  const pinned = entries.filter((e) => e.pinned).length
  useEffect(() => void useClipboard.getState().refresh(), [])

  const commitCustom = () => {
    const n = Math.round(Number(draft))
    if (Number.isFinite(n) && n >= 1 && n <= 3650) void update({ clipboardRetentionDays: n })
    else setDraft(String(days || 7))
  }

  return (
    <>
      <Group title="Clipboard history" description="Kudzu remembers what you copy, so you can find it and paste it again. History stays on this Mac. It is never synced or uploaded.">
        <Row title="Record what I copy" description="Text, links, images, files and colors, with the time each was copied." htmlFor="clip-on">
          <Switch id="clip-on" checked={settings.clipboardEnabled} onCheckedChange={(clipboardEnabled) => void update({ clipboardEnabled })} />
        </Row>
        <Row
          title="Shortcut"
          description={
            <>
              Opens your history over any app. <Shortcut combo="enter" spoken /> pastes, <Shortcut combo="mod+enter" spoken /> only copies.
            </>
          }
        >
          <HotkeyRecorder
            value={settings.clipboardHotkey}
            label="Clipboard history shortcut"
            onChange={(accel) =>
              api.setClipboardHotkey(accel).then(
                (s) => {
                  useStore.setState({ settings: s })
                  toast.success("Shortcut updated")
                },
                (e) => toast.error("That shortcut is taken", { description: String(e) }),
              )
            }
          />
        </Row>
        <Row
          title="Keep history"
          description={days ? `Unpinned items are deleted ${retentionLabel(days).replace(/^for /, "after ")}. Pinned items stay until you unpin them.` : "Items stay until you delete them (up to 5,000). Pinned items always stay."}
        >
          {custom && (
            <span className="flex items-center gap-1.5 text-xs text-muted-foreground">
              <Input
                aria-label="Days to keep history"
                type="number"
                min={1}
                max={3650}
                value={draft}
                onChange={(e) => setDraft(e.target.value)}
                onBlur={commitCustom}
                onKeyDown={(e) => e.key === "Enter" && commitCustom()}
                className="h-8 w-20 text-center tabular"
              />
              days
            </span>
          )}
          <Select
            value={custom ? "custom" : String(days)}
            onValueChange={(v) => {
              if (v === "custom") {
                setCustom(true)
                setDraft(String(days || 7))
              } else {
                setCustom(false)
                void update({ clipboardRetentionDays: Number(v) })
              }
            }}
          >
            <SelectTrigger className="w-32" aria-label="Keep history for">
              <SelectValue />
            </SelectTrigger>
            <SelectContent align="end">
              {RETENTION_OPTIONS.map((o) => (
                <SelectItem key={o.days} value={String(o.days)}>
                  {o.label}
                </SelectItem>
              ))}
              <SelectItem value="custom">Custom…</SelectItem>
            </SelectContent>
          </Select>
        </Row>
        <Row
          title="Paste as plain text"
          description={
            <>
              Drops fonts, colors and links from formatted text. Hold <Shortcut combo="shift" spoken /> to do the opposite.
            </>
          }
          htmlFor="clip-plain"
        >
          <Switch id="clip-plain" checked={settings.clipboardPastePlain} onCheckedChange={(clipboardPastePlain) => void update({ clipboardPastePlain })} />
        </Row>
      </Group>

      <Group title="Privacy" description="Password managers are excluded. Kudzu also skips anything an app marks as private or temporary, like passwords and one-time codes.">
        <div className="py-3">
          <AppPicker value={settings.clipboardIgnoredApps} onChange={(clipboardIgnoredApps) => void update({ clipboardIgnoredApps })} addLabel="Never record from an app" />
        </div>
      </Group>

      <Group title="Storage">
        <Row
          title={`${entries.length.toLocaleString()} item${entries.length === 1 ? "" : "s"}${diskUsage ? ` · ${formatBytes(diskUsage)}` : ""}`}
          description={pinned ? `${pinned} pinned. Up to 5,000 unpinned items are kept; the oldest go first.` : "Up to 5,000 unpinned items are kept; the oldest go first."}
        >
          <Button size="sm" variant="secondary" disabled={!entries.length} onClick={() => setConfirm(true)}>
            Clear history…
          </Button>
        </Row>
      </Group>

      <AlertDialog open={confirm} onOpenChange={setConfirm}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>Clear clipboard history?</AlertDialogTitle>
            <AlertDialogDescription>
              {pinned ? `Deletes everything except your ${pinned} pinned item${pinned === 1 ? "" : "s"}.` : "Deletes everything Kudzu remembered."} This can&apos;t be undone.
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>Cancel</AlertDialogCancel>
            <AlertDialogAction variant="destructive" onClick={() => void clear(false).then((n) => toast.success(n === 1 ? "Deleted 1 item" : `Deleted ${n} items`))}>
              Clear history
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </>
  )
}

function Apps() {
  const settings = useStore((s) => s.settings)
  const update = useStore((s) => s.updateSettings)
  const apps = useInstalledApps()
  const entries = Object.entries(settings.appInsertions).sort((a, b) => appName(apps, a[0]).localeCompare(appName(apps, b[0])))
  const [adding, setAdding] = useState<string[]>([])
  return (
    <>
      <Group title="Never expand in" description="Password managers are excluded by default. Snippet and group rules can't override this list.">
        <div className="py-3">
          <AppPicker value={settings.deniedBundleIDs} onChange={(deniedBundleIDs) => void update({ deniedBundleIDs })} addLabel="Exclude an app" />
        </div>
      </Group>
      <Group title="How to insert, per app" description="Terminals work best with paste. Snippets that choose their own method win.">
        {entries.map(([id, method]) => {
          const app = apps.find((a) => a.bundleId === id)
          return (
            <div key={id} className="flex items-center gap-3 py-2.5">
              <AppIcon path={app?.path} name={appName(apps, id)} size={22} />
              <span className="min-w-0 flex-1 truncate text-[13px]">{appName(apps, id)}</span>
              <Segmented<InsertionMethod>
                size="sm"
                label={`Insertion in ${appName(apps, id)}`}
                value={method}
                onChange={(m) => void update({ appInsertions: { ...settings.appInsertions, [id]: m } })}
                options={[
                  { value: "automatic", label: "Auto" },
                  { value: "type", label: "Type" },
                  { value: "paste", label: "Paste" },
                ]}
              />
              <Button
                variant="ghost"
                size="icon-xs"
                aria-label={`Remove ${appName(apps, id)}`}
                onClick={() => {
                  const next = { ...settings.appInsertions }
                  delete next[id]
                  void update({ appInsertions: next })
                }}
              >
                <X />
              </Button>
            </div>
          )
        })}
        <div className="py-3">
          <AppPicker
            value={adding}
            addLabel="Add an app"
            onChange={(ids) => {
              const fresh = ids.filter((i) => !(i in settings.appInsertions))
              if (fresh.length) void update({ appInsertions: { ...settings.appInsertions, ...Object.fromEntries(fresh.map((i) => [i, "paste" as InsertionMethod])) } })
              setAdding([])
            }}
          />
        </div>
      </Group>
    </>
  )
}

function Data() {
  const [backups, setBackups] = useState<BackupInfo[]>([])
  const [espanso, setEspanso] = useState<EspansoDetection[]>([])
  const reload = () => api.listBackups().then(setBackups, () => setBackups([]))
  useEffect(() => {
    void reload()
    api.detectEspanso().then(setEspanso, () => setEspanso([]))
  }, [])
  const reasonLabel: Record<string, string> = { daily: "Daily", preImport: "Before import", preRestore: "Before restore", manual: "Manual", preUpdate: "Before update" }

  return (
    <>
      <Group title="Import" description="Bring snippets from Espanso or another Kudzu library. A backup is saved first.">
        {espanso.map((d) => (
          <Row key={d.path} title="Espanso found on this Mac" description={`${d.snippetCount} snippets in ${d.groupCount} files · ${d.path}`}>
            <Button
              size="sm"
              onClick={() =>
                api.importEspanso(d.path).then(
                  (r) => toast.success(`Imported ${r.importedSnippets} snippets`),
                  (e) => toast.error("Import failed", { description: String(e) }),
                )
              }
            >
              <Download /> Import
            </Button>
          </Row>
        ))}
        <Row title="From a file" description="Kudzu .json, or Espanso .yml">
          <Button size="sm" variant="secondary" onClick={() => runImport("file")}>
            Choose file…
          </Button>
          <Button size="sm" variant="ghost" onClick={() => runImport("espanso-folder")}>
            Espanso folder…
          </Button>
        </Row>
      </Group>
      <Group title="Export" description="Your snippets are always yours to take elsewhere.">
        <Row title="Kudzu library" description="Everything, lossless — snippets, groups, rules and usage.">
          <Button size="sm" variant="secondary" onClick={() => runExport("json")}>
            <Upload /> Export JSON
          </Button>
        </Row>
        <Row title="Espanso" description="Best-effort YAML so you can switch back any time.">
          <Button size="sm" variant="secondary" onClick={() => runExport("espanso")}>
            <Upload /> Export YAML
          </Button>
        </Row>
      </Group>
      <Group title="Backups" description="Kudzu keeps a snapshot every day and before every import — the last 14.">
        {backups.length === 0 && <p className="py-4 text-xs text-muted-foreground">No backups yet.</p>}
        {backups.slice(0, 14).map((b) => (
          <div key={b.path} className="flex items-center gap-3 py-2.5">
            <History className="size-4 text-faint" />
            <div className="min-w-0 flex-1">
              <div className="text-[13px]">{new Date(b.createdAt).toLocaleString(undefined, { dateStyle: "medium", timeStyle: "short" })}</div>
              <div className="text-[11px] text-faint">
                {reasonLabel[b.reason] ?? b.reason} · {relativeTime(Date.parse(b.createdAt))}
              </div>
            </div>
            <Button
              size="sm"
              variant="ghost"
              onClick={() =>
                api.restoreBackup(b.path).then(
                  () => {
                    toast.success("Library restored", { description: "Your previous library was backed up first." })
                    void reload()
                  },
                  (e) => toast.error("Restore failed", { description: String(e) }),
                )
              }
            >
              <RotateCcw /> Restore
            </Button>
          </div>
        ))}
        <div className="flex gap-2 py-3">
          <Button size="sm" variant="secondary" onClick={() => api.backupNow().then(() => (toast.success("Backup saved"), reload()))}>
            Back up now
          </Button>
          {isTauri() && (
            <Button size="sm" variant="ghost" onClick={() => api.revealDataFolder()}>
              <FolderOpen /> Show data folder
            </Button>
          )}
        </div>
      </Group>
    </>
  )
}

function ICloudSync() {
  const settings = useStore((s) => s.settings)
  const update = useStore((s) => s.updateSettings)
  const setView = useStore((s) => s.setView)
  const status = useSyncStatus()
  const busy = status.state === "starting" || status.state === "syncing"

  const line = (() => {
    switch (status.state) {
      case "off":
        return { text: "Off. Your snippets stay on this Mac.", tone: "" }
      case "unavailable":
        return { text: status.reason, tone: "text-muted-foreground" }
      case "noAccount":
        return { text: status.reason, tone: "text-warning" }
      case "starting":
      case "syncing":
        return { text: "Syncing…", tone: "" }
      case "upToDate":
        return { text: `Up to date · synced ${relativeTime(Date.parse(status.lastSync)).toLowerCase()}`, tone: "" }
      case "needsUpdate":
        return { text: "Another Mac uses a newer Kudzu. Update this one to keep syncing; nothing here is lost meanwhile.", tone: "text-warning" }
      case "error":
        return { text: status.message, tone: "text-destructive" }
    }
  })()

  return (
    <Group title="iCloud" description="The same snippets on every Mac signed in to your iCloud. Stored in your private iCloud database; Kudzu has no servers of its own.">
      <Row
        title="Sync with iCloud"
        htmlFor="icloud"
        description={
          <span className={cn("flex items-center gap-1.5", line.tone)} aria-live="polite">
            {busy && <Loader2 className="size-3 shrink-0 animate-spin" aria-hidden />}
            {status.state === "upToDate" && <Check className="size-3 shrink-0 text-success animate-check" aria-hidden />}
            {line.text}
          </span>
        }
      >
        {status.state === "needsUpdate" ? (
          <Button size="sm" variant="secondary" onClick={() => setView({ kind: "settings", section: "about" })}>
            Update
          </Button>
        ) : (
          (status.state === "upToDate" || status.state === "error") && (
            <Button size="sm" variant="ghost" onClick={() => void api.syncNow()} aria-label="Sync now">
              <RefreshCw /> Sync now
            </Button>
          )
        )}
        <Switch id="icloud" checked={settings.icloudSync} onCheckedChange={(icloudSync) => void update({ icloudSync })} />
      </Row>
    </Group>
  )
}

function About() {
  const settings = useStore((s) => s.settings)
  const update = useStore((s) => s.updateSettings)
  const [version, setVersion] = useState<string | null>(null)
  useEffect(() => {
    api.appVersion().then(setVersion, () => {})
  }, [])

  return (
    <div className="flex flex-col gap-8">
      <div className="flex flex-col items-center gap-4 pt-6 text-center">
        {/* oxlint-disable-next-line nextjs/no-img-element -- static export */}
        <img src="/icon.png" alt="" width={88} height={88} className="-my-2 drop-shadow-[0_12px_24px_oklch(0.488_0.243_264.376/0.3)]" draggable={false} />
        <div>
          <h2><Wordmark variant="small" className="mb-2 w-36 text-foreground" /></h2>
          <p className="text-xs text-muted-foreground">Version {version ?? "…"} · MIT licensed</p>
        </div>
        <p className="max-w-sm text-sm leading-relaxed text-muted-foreground"><span className="font-medium text-foreground">{brand.tagline}</span><br />{brand.description}</p>
      </div>

      <Group title="Updates">
        <UpdateRow />
        <Row title="Check for updates automatically" description="Looks a few times a day and downloads quietly. Kudzu never restarts without you." htmlFor="auto-update">
          <Switch id="auto-update" checked={settings.autoCheckUpdates} onCheckedChange={(autoCheckUpdates) => void update({ autoCheckUpdates })} />
        </Row>
        <Row title="Get beta updates" description="Try new versions early. You'll also get every stable release." htmlFor="beta">
          <Switch id="beta" checked={settings.updateChannel === "beta"} onCheckedChange={(on) => void update({ updateChannel: on ? "beta" : "stable" })} />
        </Row>
      </Group>

      <div className="flex items-start gap-2 rounded-xl bg-surface px-4 py-3 text-left text-xs leading-relaxed text-muted-foreground shadow-card">
        <ShieldCheck className="mt-px size-4 shrink-0 text-success" />
        <span>
          No accounts, no analytics, no Kudzu servers. Your snippets live on this Mac and, if you turn on iCloud sync, in your private iCloud database, which Kudzu&apos;s developer can&apos;t see. Apart from that, Kudzu only goes online to check for updates, and that sends nothing about you.
        </span>
      </div>
    </div>
  )
}

function UpdateRow() {
  const [state, setState] = useUpdateState()
  const check = () => {
    setState({ phase: "checking" })
    api.checkForUpdates().then(setState, (e) => setState({ phase: "error", message: String(e), needsMove: false }))
  }
  const restart = () => api.restartToUpdate().catch((e) => toast.error("Couldn't install the update", { description: String(e) }))

  if (state.phase === "ready" || state.phase === "restarting") {
    const notes = state.phase === "ready" ? parseNotes(state.notes) : []
    return (
      <div className="flex flex-col gap-3 py-3.5">
        <div className="flex items-center gap-6">
          <div className="min-w-0 flex-1">
            <div className="flex items-center gap-2 text-[13px] font-medium">
              <Sparkles className="size-3.5 text-iris" /> Kudzu {state.version} is ready
            </div>
            <div className="text-xs text-muted-foreground" aria-live="polite">
              {state.phase === "ready" ? "Downloaded and verified. It installs when you quit Kudzu, or restart now. Takes a second." : "Installing… Kudzu will reopen by itself."}
            </div>
          </div>
          {state.phase === "ready" ? (
            <Button size="sm" onClick={restart}>
              <RefreshCw /> Restart to update
            </Button>
          ) : (
            <Loader2 className="size-4 animate-spin text-muted-foreground" aria-hidden />
          )}
        </div>
        {notes.length > 0 && (
          <div className="selectable max-h-48 overflow-y-auto rounded-lg bg-foreground/[0.025] px-3 py-2.5 text-xs leading-relaxed shadow-well">
            {notes.map((sec, i) => (
              <div key={i} className={cn(i > 0 && "mt-2")}>
                {sec.heading && <div className="mb-0.5 font-medium text-foreground/90">{sec.heading}</div>}
                <ul className="list-disc space-y-0.5 pl-4 text-muted-foreground marker:text-faint">
                  {sec.items.map((it, j) => (
                    <li key={j}>{it}</li>
                  ))}
                </ul>
              </div>
            ))}
          </div>
        )}
        <button className="self-start text-xs text-muted-foreground underline-offset-2 hover:text-foreground hover:underline" onClick={() => void openExternal(RELEASES_URL)}>
          All release notes
        </button>
      </div>
    )
  }

  if (state.phase === "downloading") {
    const pct = state.total ? Math.min(100, Math.round((state.downloaded / state.total) * 100)) : null
    return (
      <div className="flex flex-col gap-2.5 py-3.5">
        <div className="flex items-center justify-between gap-6">
          <div>
            <div className="text-[13px] font-medium">Downloading Kudzu {state.version}</div>
            <div className="text-xs text-muted-foreground" aria-live="polite">
              In the background{pct !== null && ` · ${pct}%`}. You&apos;ll be asked before anything restarts.
            </div>
          </div>
        </div>
        <div className="h-1 overflow-hidden rounded-full bg-foreground/[0.06] shadow-well" role="progressbar" aria-label="Download progress" aria-valuenow={pct ?? undefined} aria-valuemin={0} aria-valuemax={100}>
          <div className="h-full rounded-full bg-iris transition-[width] duration-300" style={{ width: `${pct ?? 30}%` }} />
        </div>
      </div>
    )
  }

  const description =
    state.phase === "checking"
      ? "Checking…"
      : state.phase === "upToDate"
        ? `You're on the latest version · checked ${relativeTime(Date.parse(state.checkedAt)).toLowerCase()}`
        : state.phase === "error"
          ? state.needsMove
            ? state.message
            : `Couldn't check: ${state.message}`
          : isTauri()
            ? "New versions download in the background and install when you quit."
            : "Updates are available in the desktop app."

  return (
    <Row title="Software update" description={<span className={cn(state.phase === "error" && (state.needsMove ? "text-warning" : "text-destructive"))}>{description}</span>}>
      <Button size="sm" variant="secondary" onClick={check} disabled={state.phase === "checking"}>
        {state.phase === "checking" ? <Loader2 className="animate-spin" /> : <RefreshCw />}
        {state.phase === "error" && !state.needsMove ? "Try again" : "Check now"}
      </Button>
    </Row>
  )
}
