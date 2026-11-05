"use client"

// Fill-in form shown near the caret when a snippet contains fields or dropdowns.
// Tab moves between fields, Return inserts (⌘Return inside multi-line fields), Esc cancels and
// leaves the trigger exactly as typed.

import { ChevronDown, TextCursorInput } from "lucide-react"
import { useCallback, useEffect, useLayoutEffect, useRef, useState } from "react"

import { api, Events, on, renderPreview } from "@/lib/backend"
import { applyTheme } from "@/lib/theme"
import type { FormRequest, Library, RenderResult } from "@/lib/types"
import { cn } from "@/lib/utils"

import { Shortcut } from "../shortcut"

export function FormPanel() {
  const [request, setRequest] = useState<FormRequest | null>(null)
  const [values, setValues] = useState<Record<string, string>>({})
  const [preview, setPreview] = useState<RenderResult | null>(null)
  const [library, setLibrary] = useState<Library | null>(null)
  const root = useRef<HTMLDivElement>(null)
  const first = useRef<HTMLElement | null>(null)

  const load = useCallback((r: FormRequest | null) => {
    setRequest(r)
    setValues(r?.initialValues ?? {})
    setPreview(null)
    requestAnimationFrame(() => first.current?.focus())
  }, [])

  useEffect(() => {
    api.getState().then((s) => {
      setLibrary(s.library)
      applyTheme(s.settings.theme)
    })
    api.pendingForm().then(load)
    const offs = [on<FormRequest>(Events.formRequest, load), on<Library>(Events.library, setLibrary)]
    return () => offs.forEach((f) => f())
  }, [load])

  // Preview
  useEffect(() => {
    if (!request) return
    let live = true
    const t = setTimeout(() => {
      renderPreview(request.content, request.snippetId, values, (id) => library?.snippets.find((s) => s.id === id)?.content).then((r) => live && setPreview(r))
    }, 50)
    return () => {
      live = false
      clearTimeout(t)
    }
  }, [request, values, library])

  // Size the native window to the content.
  useLayoutEffect(() => {
    if (!root.current) return
    const ro = new ResizeObserver(() => {
      const pad = document.documentElement.classList.contains("vibrant") ? 0 : 24
      const h = Math.ceil(root.current!.getBoundingClientRect().height) + pad
      void api.resizeForm(Math.min(h, 640))
    })
    ro.observe(root.current)
    return () => ro.disconnect()
  }, [])

  const submit = () => request && void api.submitForm(request.token, values)
  const cancel = () => request && void api.cancelForm(request.token)

  if (!request) return null

  return (
    <div className="flex w-full justify-center p-3 vibrant:p-0">
      <div
        ref={root}
        role="dialog"
        aria-label={`Fill in ${request.snippetName}`}
        onKeyDown={(e) => {
          const mod = e.metaKey || e.ctrlKey
          if (e.key === "Escape") {
            e.preventDefault()
            cancel()
          } else if (e.key === "Enter" && (mod || !(e.target instanceof HTMLTextAreaElement))) {
            e.preventDefault()
            submit()
          }
        }}
        className="flex w-full flex-col overflow-hidden rounded-2xl bg-popover text-popover-foreground shadow-panel backdrop-blur-2xl animate-pop vibrant:animate-none vibrant:rounded-none vibrant:bg-popover/55 vibrant:shadow-none vibrant:backdrop-blur-none"
      >
        <header className="flex items-center gap-2.5 border-b border-hairline px-4 py-3" data-tauri-drag-region>
          <span className="flex size-6 items-center justify-center rounded-md bg-iris-soft text-iris-foreground">
            <TextCursorInput className="size-3.5" />
          </span>
          <span className="min-w-0 flex-1 truncate text-[13px] font-semibold" data-tauri-drag-region>
            {request.snippetName || "Fill in snippet"}
          </span>
          {request.targetApp && <span className="truncate text-[11px] text-faint">into {request.targetApp.name}</span>}
        </header>

        <div className="flex flex-col gap-3 px-4 py-3.5">
          {request.fields.map((f, i) => {
            const id = `field-${i}`
            const ref = (el: HTMLElement | null) => {
              if (i === 0) first.current = el
            }
            const set = (v: string) => setValues((cur) => ({ ...cur, [f.name]: v }))
            const common = "w-full rounded-lg bg-foreground/[0.03] px-3 text-[13.5px] shadow-well outline-none transition-shadow focus:bg-transparent focus:shadow-none focus:ring-2 focus:ring-ring"
            return (
              <div key={f.name} className="flex flex-col gap-1.5">
                <label htmlFor={id} className="text-xs font-medium text-muted-foreground">
                  {f.name}
                </label>
                {f.kind === "choice" ? (
                  <div className="relative">
                    <select id={id} ref={ref} value={values[f.name] ?? ""} onChange={(e) => set(e.target.value)} className={cn(common, "h-9 appearance-none pr-8")}>
                      {f.options.map((o) => (
                        <option key={o} value={o}>
                          {o}
                        </option>
                      ))}
                    </select>
                    <ChevronDown className="pointer-events-none absolute top-2.5 right-2.5 size-4 text-muted-foreground" />
                  </div>
                ) : f.kind === "multiline" ? (
                  <textarea id={id} ref={ref} value={values[f.name] ?? ""} onChange={(e) => set(e.target.value)} rows={3} className={cn(common, "resize-none py-2 leading-relaxed")} />
                ) : (
                  <input id={id} ref={ref} value={values[f.name] ?? ""} onChange={(e) => set(e.target.value)} className={cn(common, "h-9")} autoComplete="off" spellCheck={false} />
                )}
              </div>
            )
          })}
        </div>

        <div className="mx-4 mb-3.5 max-h-40 overflow-y-auto rounded-lg bg-foreground/[0.03] px-3 py-2.5 shadow-well">
          <div className="mb-1 text-[10.5px] font-medium text-faint">Preview</div>
          <div className="selectable text-[12.5px] leading-relaxed whitespace-pre-wrap text-foreground/85">{preview?.text ?? "…"}</div>
        </div>

        <footer className="flex items-center gap-4 border-t border-hairline px-4 py-2.5 text-[11.5px] text-muted-foreground">
          <span className="flex items-center gap-1.5">
            <Shortcut combo="tab" /> Next field
          </span>
          <span className="flex items-center gap-1.5">
            <Shortcut combo="escape" /> Cancel
          </span>
          <button onClick={submit} className="ml-auto flex h-7 items-center gap-2 rounded-lg bg-iris px-3 text-xs font-medium text-iris-contrast shadow-accent sheen-strong transition hover:brightness-110 active:translate-y-px active:shadow-pressed active:bg-none">
            Insert <Shortcut combo="enter" tone="onAccent" />
          </button>
        </footer>
      </div>
    </div>
  )
}
