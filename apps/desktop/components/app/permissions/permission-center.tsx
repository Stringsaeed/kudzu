"use client"

import { ArrowUpRight, Check, CircleHelp, FolderOpen, Keyboard, Loader2, MousePointer2, ShieldCheck, X } from "lucide-react"
import { useCallback, useEffect, useRef, useState } from "react"

import { Button } from "@/components/ui/button"
import { Item, ItemActions, ItemContent, ItemDescription, ItemGroup, ItemMedia, ItemTitle } from "@/components/ui/item"
import { api, Events, isTauri, on } from "@/lib/backend"
import { applyTheme } from "@/lib/theme"
import type { EngineStatus, Settings } from "@/lib/types"
import { cn } from "@/lib/utils"
import { permissionsReady } from "./permission-setup"

type PermissionKind = Parameters<typeof api.beginPermission>[0]
type Handoff = { kind: "idle" } | { kind: "opening" } | { kind: "waiting" } | { kind: "error"; message: string }

export function PermissionCenter() {
  const [status, setStatus] = useState<EngineStatus | null>(null)
  const [selected, setSelected] = useState<PermissionKind>("accessibility")
  const [appName, setAppName] = useState("Kudzu")
  const [accessibilityName, setAccessibilityName] = useState("Accessibility")
  const permissionNames = { accessibility: accessibilityName, inputMonitoring: "Input Monitoring" }
  const [handoff, setHandoff] = useState<Handoff>({ kind: "idle" })
  const [help, setHelp] = useState(false)
  const [dragAvailable, setDragAvailable] = useState(false)
  const [preview, setPreview] = useState(false)
  const icon = useRef<HTMLButtonElement>(null)
  const permissionReady = status !== null && permissionsReady(status)
  const selectedGranted = status?.permissions[selected] ?? false
  const needsConnection = selected === "accessibility" && selectedGranted && !permissionReady
  const [restartOffered, setRestartOffered] = useState(false)

  useEffect(() => {
    const timer = setTimeout(() => setRestartOffered(needsConnection), needsConnection ? 5000 : 0)
    return () => clearTimeout(timer)
  }, [needsConnection])

  const fail = useCallback((e: unknown) => setHandoff({ kind: "error", message: String(e) }), [])
  useEffect(() => {
    let live = true
    // The poll also recovers an event missed while the webview is loading or suspended.
    let inFlight = false
    const refresh = async () => {
      if (inFlight) return
      inFlight = true
      try { const value = await api.engineStatus(); if (live) setStatus(value) }
      catch (e) { if (live) fail(e) }
      finally { inFlight = false }
    }
    const offs = [
      on<EngineStatus>(Events.status, setStatus),
      on<Settings>(Events.settings, (settings) => applyTheme(settings.theme)),
      on<PermissionKind>(Events.permissionShow, (kind) => {
        setSelected(kind); setHandoff({ kind: "idle" }); setHelp(false); void refresh()
      }),
    ]
    void api.permissionContext().then((context) => {
      if (live) { setSelected(context.kind); setAppName(context.appName); setAccessibilityName(context.accessibilityName); setPreview(!isTauri()) }
    }, fail)
    void api.getState().then((state) => { if (live) applyTheme(state.settings.theme) }, fail)
    void refresh()
    const timer = setInterval(() => { if (!document.hidden) void refresh() }, 1000)
    window.addEventListener("focus", refresh)
    return () => { live = false; offs.forEach((off) => off()); clearInterval(timer); window.removeEventListener("focus", refresh) }
  }, [fail])

  useEffect(() => {
    const element = icon.current
    if (!element || !isTauri()) return
    let live = true
    const update = () => {
      const { x, y, width, height } = element.getBoundingClientRect()
      void api.installPermissionIcon({ x, y, width, height }).then((available) => {
        if (live) setDragAvailable(available)
      }, fail)
    }
    const observer = new ResizeObserver(update)
    observer.observe(element)
    window.addEventListener("resize", update)
    update()
    return () => { live = false; observer.disconnect(); window.removeEventListener("resize", update) }
  }, [fail])

  useEffect(() => {
    if (selectedGranted) void api.stopPermissionGuide().catch(fail)
  }, [selectedGranted, fail])

  const close = () => void api.closePermissions().catch(fail)
  const begin = async () => {
    setHandoff({ kind: "opening" })
    try { await api.beginPermission(selected); setHandoff({ kind: "waiting" }) }
    catch (e) { fail(e) }
  }
  const choose = (kind: PermissionKind) => {
    setSelected(kind); setHandoff({ kind: "idle" })
    void api.stopPermissionGuide().catch(fail)
  }
  const headline = permissionReady ? "You’re all set." : needsConnection ? "Access is allowed." : "Let Kudzu do the typing."
  const detail = !status ? "Checking access on this Mac…"
    : permissionReady ? status.canExpand ? "Your shortcuts can now become full sentences." : status.paused ? "Permissions are set. Expansion is currently paused." : "Permissions are set. Close the secure input field to expand."
    : needsConnection ? "One last step to connect Kudzu to your keyboard."
    : "A little permission to turn your triggers into text, wherever you write."

  return (
    <main className="flex min-h-screen items-center justify-center bg-background p-4 vibrant:bg-transparent vibrant:p-0">
      <section aria-label="Kudzu permissions" onKeyDown={(event) => { if (event.key === "Escape") { event.stopPropagation(); close() } }}
        className="relative flex min-h-[660px] w-full max-w-[420px] flex-col overflow-hidden rounded-[18px] border border-hairline bg-popover text-popover-foreground shadow-raised vibrant:border-0">
        <header className="flex h-12 shrink-0 items-center gap-2 px-5 text-xs text-muted-foreground" data-tauri-drag-region>
          <ShieldCheck className="size-3.5" /><span className="flex-1" data-tauri-drag-region>Permissions for {appName}</span>
          <Button variant="ghost" size="icon" className="size-6 rounded-full" aria-label="Close permission setup" onClick={close}><X className="size-3.5" /></Button>
        </header>
        <div className="relative flex flex-1 flex-col px-6 pb-5">
          <div className="pointer-events-none absolute inset-x-0 top-0 h-44 bg-[radial-gradient(ellipse_at_top,var(--iris-soft),transparent_70%)]" />
          <div className="relative flex flex-col items-center pt-3 text-center">
            <button ref={icon} type="button" aria-label={`Show ${appName} in Finder to add it to System Settings`} title="Drag this app into System Settings"
              onClick={() => { if (isTauri()) void api.revealPermissionApp().catch(fail); else setHelp(true) }}
              className="permission-app-icon relative size-20 cursor-grab rounded-[20px] outline-none transition-transform hover:scale-105 focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-4 active:cursor-grabbing">
              {/* oxlint-disable-next-line nextjs/no-img-element -- bundled app artwork, also used for native dragging */}
              <img src="/icon.png" alt="" width={80} height={80} draggable={false} className="pointer-events-none size-20 select-none" />
            </button>
            <p className="mt-2 flex h-4 items-center gap-1 text-[10px] text-muted-foreground"><MousePointer2 className="size-3" />{dragAvailable ? "Drag into System Settings to add the app" : "Your app. Your permission."}</p>
            <h1 className="mt-5 text-[23px] font-semibold tracking-[-0.035em]" aria-live="polite">{headline}</h1>
            <p className="mt-2 max-w-[310px] text-[13px] leading-relaxed text-muted-foreground">{detail}</p>
          </div>

          <ItemGroup role="group" className="mt-6 gap-2 has-data-[size=sm]:gap-2" aria-label="Permissions">
            {(["accessibility", "inputMonitoring"] satisfies PermissionKind[]).map((kind) => {
              const granted = status?.permissions[kind] ?? false
              const Icon = kind === "accessibility" ? ShieldCheck : Keyboard
              return (
                <Item key={kind} asChild variant="outline" size="sm"
                  className={cn("flex-nowrap gap-3 rounded-xl px-3.5 py-3 text-left", selected === kind ? "border-iris/40 bg-iris-soft/50 shadow-card" : "border-hairline bg-surface/50 hover:bg-surface")}>
                  <button type="button" onClick={() => choose(kind)} aria-pressed={selected === kind}>
                    <ItemMedia variant="icon" className={cn("size-8 rounded-lg group-has-data-[slot=item-description]/item:translate-y-0 group-has-data-[slot=item-description]/item:self-center", granted ? "bg-success/10 text-success" : "bg-foreground/5 text-muted-foreground")}>
                      <Icon className="size-4" />
                    </ItemMedia>
                    <ItemContent className="min-w-0 gap-0.5">
                      <ItemTitle className="line-clamp-none text-[13px]">{permissionNames[kind]}</ItemTitle>
                      <ItemDescription className="text-[11px]">{kind === "accessibility" ? "Insert snippets into other apps." : "Additional keyboard monitoring access."}</ItemDescription>
                    </ItemContent>
                    <ItemActions className={cn("shrink-0 gap-1 text-[10px] font-medium", granted ? "text-success" : "text-muted-foreground")}>
                      {granted ? <><Check className="size-3.5 animate-check" />Allowed</> : kind === "accessibility" ? "Required" : "Optional"}
                    </ItemActions>
                  </button>
                </Item>
              )
            })}
          </ItemGroup>

          <div className="mt-4 min-h-[72px] text-center" aria-live="polite">
            {handoff.kind === "error" ? <p role="alert" className="rounded-lg bg-destructive/10 p-3 text-xs leading-relaxed text-destructive">{handoff.message}</p>
              : permissionReady && selectedGranted ? <div className="permission-success flex flex-col items-center gap-2 py-2"><span className="flex items-center gap-1.5 text-xs font-medium text-success"><Check className="size-4" />Access confirmed</span><p className="text-[11px] text-muted-foreground">{status?.canExpand ? "Ready to expand in your other apps." : status?.summary}</p></div>
              : selectedGranted ? <div className="flex flex-col items-center gap-2 py-1"><p className="flex items-center gap-1.5 text-xs font-medium text-success"><Check className="size-4" />Access confirmed</p><p className="text-[11px] leading-relaxed text-muted-foreground">{needsConnection ? restartOffered ? `macOS may need a restart of ${appName} to apply access. We’ll bring you back here.` : "Connecting to your keyboard…" : `Next, allow ${accessibilityName} so Kudzu can insert text.`}</p></div>
              : handoff.kind === "waiting" ? <div className="flex flex-col items-center gap-2 py-1"><p className="flex items-center gap-1.5 text-xs font-medium"><Loader2 className="size-3.5 animate-spin motion-reduce:animate-none" />Waiting for macOS</p><p className="text-[11px] leading-relaxed text-muted-foreground">Turn on <strong className="font-medium text-foreground">{appName}</strong> in {permissionNames[selected]}.<br />If it isn’t listed, drag the icon into the list.</p></div>
              : <p className="px-3 py-2 text-[11px] leading-relaxed text-muted-foreground">{selectedGranted ? "Access is allowed. If typing is still unavailable, quit and reopen Kudzu after checking the switch." : "We’ll open the right page and stay beside you. macOS may ask you to confirm with Touch ID or your password."}</p>}
          </div>

          <div className="mt-auto flex flex-col gap-2 pt-2">
            {permissionReady && selectedGranted ? <Button className="h-10 w-full" onClick={close}>Back to Kudzu<Check className="size-4" /></Button>
              : needsConnection && restartOffered ? <Button className="h-10 w-full" onClick={() => void api.restartPermissionApp().catch(fail)}>Restart {appName}</Button>
              : selected === "inputMonitoring" && selectedGranted ? <Button className="h-10 w-full" onClick={() => choose("accessibility")}>Set up app control<ArrowUpRight className="size-4" /></Button>
              : <Button className="h-10 w-full" disabled={!status || handoff.kind === "opening"} onClick={() => void begin()}>{handoff.kind === "opening" ? "Opening Settings…" : "Open System Settings"}<ArrowUpRight className="size-4" /></Button>}
            <button className="flex items-center justify-center gap-1.5 rounded-md py-1.5 text-[11px] text-muted-foreground outline-none hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring" aria-expanded={help} onClick={() => setHelp(!help)}><CircleHelp className="size-3.5" />{help ? "Hide help" : "Don’t see the app, or already allowed?"}</button>
          </div>
          {help && <div className="absolute inset-x-4 bottom-4 z-10 rounded-xl border border-hairline bg-popover p-4 shadow-raised">
            <div className="flex items-center justify-between"><h2 className="text-[13px] font-semibold">Add {appName} to the list</h2><Button variant="ghost" size="icon" className="size-6" aria-label="Close help" onClick={() => setHelp(false)}><X className="size-3.5" /></Button></div>
            <ol className="mt-2 list-decimal space-y-2 pl-4 text-xs leading-relaxed text-muted-foreground"><li>Open Privacy &amp; Security → {permissionNames[selected]}.</li><li>Drag the app icon into the list, or use the + button to select {appName}.</li><li>Turn on its switch and approve the macOS prompt.</li></ol>
            <p className="mt-3 text-xs leading-relaxed text-muted-foreground">Already on? Quit and reopen {appName} if macOS hasn’t applied the change. Make sure you enabled this copy of the app.</p>
            <Button variant="outline" size="sm" className="mt-3 w-full" onClick={() => { if (preview) setHelp(false); else void api.revealPermissionApp().catch(fail) }}><FolderOpen />Show this app in Finder</Button>
            <Button variant="ghost" size="sm" className="mt-1 w-full" onClick={() => void api.restartPermissionApp().catch(fail)}>Restart {appName}</Button>
          </div>}
          <p className="mt-3 text-center text-[10px] text-faint">Your typing is never recorded or sent anywhere.</p>
        </div>
      </section>
      {preview && <PermissionPreview />}
    </main>
  )
}

function PermissionPreview() {
  return <aside className="fixed bottom-2 left-2 z-20 flex gap-1 rounded-lg border border-hairline bg-background p-2 text-[10px] text-muted-foreground"><span className="px-1 py-1">Preview</span>{["missing", "allowed", "paused", "secure", "restart", "revoked"].map((state) => <button key={state} className="rounded px-2 py-1 hover:bg-accent focus-visible:ring-2 focus-visible:ring-ring" onClick={() => window.dispatchEvent(new CustomEvent("kudzu:preview-permission", { detail: state }))}>{state}</button>)}</aside>
}
