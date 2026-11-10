"use client"

import { getIdentifier } from "@tauri-apps/api/app"
import { useCallback, useEffect, useRef, useState } from "react"
import { toast } from "sonner"

import { Badge } from "@/components/ui/badge"
import { Toaster } from "@/components/ui/sonner"
import { TooltipProvider } from "@/components/ui/tooltip"
import { useHotkeys } from "@/hooks/use-hotkeys"
import { useStore } from "@/lib/store"
import { api, Events, isTauri, on } from "@/lib/backend"
import { useClipboard } from "@/lib/clipboard"
import { applyTheme } from "@/lib/theme"
import type { Group, UpdateState } from "@/lib/types"
import { cn } from "@/lib/utils"

import { ClipboardView, type ClipboardViewHandle } from "../clipboard/clipboard-view"
import { EditorEmpty, SnippetEditor } from "../editor/snippet-editor"
import { CommandMenu } from "./command-menu"
import { GroupSettingsDialog } from "./group-settings"
import { Insights } from "./insights"
import { Onboarding } from "./onboarding"
import { SettingsView } from "./settings"
import { ShortcutsDialog } from "./shortcuts"
import { Sidebar } from "./sidebar"
import { SnippetList, type SnippetListHandle } from "./snippet-list"
import { Toolbar } from "./toolbar"

export function MainApp() {
  const init = useStore((s) => s.init)
  const ready = useStore((s) => s.ready)
  const view = useStore((s) => s.view)
  const setView = useStore((s) => s.setView)
  const settings = useStore((s) => s.settings)
  const selectedId = useStore((s) => s.selectedId)
  const snippet = useStore((s) => s.library.snippets.find((x) => x.id === s.selectedId) ?? null)
  const newSnippet = useStore((s) => s.newSnippet)
  const deleteSnippets = useStore((s) => s.deleteSnippets)
  const restoreSnippets = useStore((s) => s.restoreSnippets)
  const duplicate = useStore((s) => s.duplicate)
  const toggleFavorite = useStore((s) => s.toggleFavorite)
  const toggleEnabled = useStore((s) => s.toggleEnabled)
  const createGroup = useStore((s) => s.createGroup)
  const setPaused = useStore((s) => s.setPaused)
  const status = useStore((s) => s.status)
  const setCommandOpen = useStore((s) => s.setCommandOpen)
  const setShortcutsOpen = useStore((s) => s.setShortcutsOpen)
  const listRef = useRef<SnippetListHandle>(null)
  const clipRef = useRef<ClipboardViewHandle>(null)
  const [groupSettings, setGroupSettings] = useState<Group | null>(null)
  const [isDev, setIsDev] = useState(false)
  const [sidebarOpen, setSidebarOpen] = useState(true)

  useEffect(() => {
    if (!isTauri()) return
    let live = true
    getIdentifier().then((identifier) => {
      if (live) setIsDev(identifier === "sh.saeed.kudzu.dev")
    }, () => {})
    return () => { live = false }
  }, [])

  useEffect(() => init(), [init])
  // Clipboard history stays live in the background (sidebar count, instant view switches).
  useEffect(() => useClipboard.getState().init(), [])
  useEffect(() => {
    if (ready) applyTheme(settings.theme)
  }, [ready, settings.theme])

  // An update finished downloading in the background: mention it once per version.
  useEffect(() => {
    let announced: string | null = null
    return on<UpdateState>(Events.update, (s) => {
      if (s.phase !== "ready" || s.version === announced) return
      announced = s.version
      if (useStore.getState().view.kind === "settings") return // the About page already shows it
      toast(`Kudzu ${s.version} is ready`, {
        description: "It installs when you quit, or restart now.",
        action: { label: "Restart", onClick: () => void api.restartToUpdate() },
        cancel: { label: "What's new", onClick: () => setView({ kind: "settings", section: "about" }) },
        duration: 15000,
      })
    })
  }, [setView])

  const requestDelete = useCallback(
    async (ids: string[]) => {
      if (!ids.length) return
      const removed = await deleteSnippets(ids)
      const label = removed.length === 1 ? `Deleted “${removed[0].name || "Untitled"}”` : `Deleted ${removed.length} snippets`
      toast(label, {
        action: { label: "Undo", onClick: () => void restoreSnippets(removed) },
        duration: 6000,
      })
      requestAnimationFrame(() => listRef.current?.focusList())
    },
    [deleteSnippets, restoreSnippets],
  )

  const focusEditor = () => {
    const cm = document.querySelector<HTMLElement>(".kz-editor .cm-content")
    if (cm) cm.focus()
  }

  const inLibrary = view.kind !== "insights" && view.kind !== "settings" && view.kind !== "clipboard"
  const area = inLibrary ? "library" : view.kind

  useHotkeys([
    { combo: "mod+k", allowInInput: true, handler: () => setCommandOpen(true) },
    { combo: "mod+n", allowInInput: true, handler: () => void newSnippet() },
    { combo: "mod+shift+n", allowInInput: true, handler: () => void createGroup("New group", null) },
    {
      combo: "mod+f",
      allowInInput: true,
      handler: () => {
        if (view.kind === "clipboard") return clipRef.current?.focusSearch()
        if (!inLibrary) setView({ kind: "all" })
        requestAnimationFrame(() => listRef.current?.focusFilter())
      },
    },
    { combo: "mod+l", allowInInput: true, handler: () => (view.kind === "clipboard" ? clipRef.current?.focusList() : listRef.current?.focusList()) },
    { combo: "mod+e", allowInInput: true, handler: focusEditor, enabled: inLibrary && !!snippet },
    { combo: "mod+1", allowInInput: true, handler: () => setView({ kind: "all" }) },
    { combo: "mod+2", allowInInput: true, handler: () => setView({ kind: "favorites" }) },
    { combo: "mod+3", allowInInput: true, handler: () => setView({ kind: "recent" }) },
    { combo: "mod+4", allowInInput: true, handler: () => setView({ kind: "insights" }) },
    { combo: "mod+5", allowInInput: true, handler: () => setView({ kind: "clipboard" }) },
    { combo: "mod+,", allowInInput: true, handler: () => setView({ kind: "settings" }) },
    { combo: ["mod+/", "shift+?"], allowInInput: false, handler: () => setShortcutsOpen(true) },
    { combo: "mod+/", allowInInput: true, handler: () => setShortcutsOpen(true) },
    { combo: "mod+shift+p", allowInInput: true, handler: () => void setPaused(!status.paused) },
    { combo: "mod+d", allowInInput: true, enabled: inLibrary && !!selectedId, handler: () => selectedId && void duplicate(selectedId) },
    { combo: "mod+shift+f", allowInInput: true, enabled: inLibrary && !!selectedId, handler: () => selectedId && void toggleFavorite(selectedId) },
    { combo: "mod+shift+e", allowInInput: true, enabled: inLibrary && !!selectedId, handler: () => selectedId && void toggleEnabled(selectedId) },
    {
      combo: "mod+backspace",
      enabled: inLibrary,
      handler: () => void requestDelete(listRef.current?.selection() ?? []),
    },
  ])

  // Escape from the editor returns to the list (keyboard round trip).
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== "Escape" || e.defaultPrevented) return
      const t = e.target as HTMLElement
      if (t.closest("[data-editor-scroll]") && !document.querySelector("[data-radix-popper-content-wrapper]")) {
        listRef.current?.focusList()
      }
    }
    window.addEventListener("keydown", onKey)
    return () => window.removeEventListener("keydown", onKey)
  }, [])

  if (!ready) return <div className="h-full bg-background" />

  return (
    <TooltipProvider delayDuration={500} skipDelayDuration={200}>
      {!settings.onboardingCompleted ? (
        <>
          <Onboarding />
          {isDev && <Badge className="fixed top-4 right-4">Dev</Badge>}
        </>
      ) : (
        <div className="flex h-full min-h-0 flex-col bg-sidebar">
          <Toolbar
            sidebarOpen={sidebarOpen}
            onToggleSidebar={() => setSidebarOpen((open) => !open)}
            onNavigate={(destination) => {
              setView(destination)
              requestAnimationFrame(() => listRef.current?.focusList())
            }}
          />
          <div className="flex min-h-0 flex-1">
            {/* Stays mounted and slides off the window's edge, so the main card follows instead of snapping.
                Closed, it is inert and invisible: out of the tab order and the accessibility tree. The
                negative margin leaves the card the same 8px gutter it has on its other sides. */}
            <div
              id="main-sidebar"
              inert={!sidebarOpen}
              aria-hidden={!sidebarOpen || undefined}
              className={cn(
                "w-[224px] shrink-0 transition-[margin-left,opacity,visibility] duration-(--motion-slow) ease-spring",
                !sidebarOpen && "invisible -ml-[216px] opacity-0",
              )}
            >
              <Sidebar onOpenGroupSettings={setGroupSettings} isDev={isDev} />
            </div>
            <main className="my-2 mr-2 flex min-w-0 flex-1 overflow-hidden rounded-2xl bg-background shadow-card">
              {/* Keyed by area, so a view fades in when you arrive from another one, but not when the
                  group, filter or settings section changes inside it. Opacity only: the views animate
                  their own content, and a second translate here would stack with theirs. */}
              <div key={area} className="flex min-w-0 flex-1 animate-in fade-in-0 duration-(--motion-moderate) ease-out">
                {inLibrary ? (
                  <>
                    <div className="w-[clamp(260px,27vw,320px)] shrink-0 border-r border-hairline">
                      <SnippetList ref={listRef} onRequestDelete={requestDelete} onOpen={focusEditor} />
                    </div>
                    <div className="min-w-0 flex-1">
                      {snippet ? <SnippetEditor key={snippet.id} snippet={snippet} onRequestDelete={requestDelete} /> : <EditorEmpty onNew={() => void newSnippet()} />}
                    </div>
                  </>
                ) : view.kind === "insights" ? (
                  <Insights />
                ) : view.kind === "clipboard" ? (
                  <ClipboardView ref={clipRef} filter={view.filter} />
                ) : (
                  <SettingsView section={view.kind === "settings" ? view.section : undefined} />
                )}
              </div>
            </main>
          </div>
        </div>
      )}
      <CommandMenu onOpenGroupSettings={setGroupSettings} onDelete={requestDelete} />
      <ShortcutsDialog />
      <GroupSettingsDialog group={groupSettings} onOpenChange={(o) => !o && setGroupSettings(null)} />
      <Toaster position="bottom-center" />
    </TooltipProvider>
  )
}
