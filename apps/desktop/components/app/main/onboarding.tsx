"use client"

import { ArrowLeft, ArrowRight, Check, Loader2, Sparkles } from "lucide-react"
import { useEffect, useRef, useState } from "react"

import brand from "@/design/brand.json"
import { Button } from "@/components/ui/button"
import { api, isTauri } from "@/lib/backend"
import { acceleratorLabels } from "@/lib/keys"
import { useStore } from "@/lib/store"
import type { EspansoDetection } from "@/lib/types"
import { cn } from "@/lib/utils"

import { Shortcut } from "../shortcut"
import { Wordmark } from "../wordmark"
import { PermissionSetup, permissionsReady } from "../permissions/permission-setup"

type Step = "welcome" | "permissions" | "import" | "practice"

type ImportState = { kind: "idle" } | { kind: "working" } | { kind: "done"; count: number } | { kind: "failed"; message: string }
const stepLabels: Record<Step, string> = { welcome: "Welcome", permissions: "Access", import: "Import", practice: "Try it" }

const SAMPLE_TRIGGER = ";kudzu"
const SAMPLE_TEXT = "Hello from Kudzu!"

export function Onboarding() {
  const status = useStore((s) => s.status)
  const platform = useStore((s) => s.platform)
  const settings = useStore((s) => s.settings)
  const [step, setStep] = useState<Step>("welcome")
  const [espanso, setEspanso] = useState<EspansoDetection[]>([])
  const [importState, setImportState] = useState<ImportState>({ kind: "idle" })
  const completedImports = useRef(new Map<string, number>())
  const [finishing, setFinishing] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const stepContent = useRef<HTMLDivElement>(null)

  useEffect(() => {
    api.detectEspanso().then(setEspanso, () => setError("We couldn’t check for Espanso snippets. You can import them later in Settings."))
  }, [])

  const needsPermissions = platform === "macos" || platform === "web"
  const steps: Step[] = ["welcome"]
  if (needsPermissions) steps.push("permissions")
  if (espanso.length) steps.push("import")
  steps.push("practice")
  const index = steps.indexOf(step)
  const busy = finishing || importState.kind === "working"
  const finish = async () => {
    setFinishing(true)
    setError(null)
    try {
      const saved = await api.updateSettings({ onboardingCompleted: true })
      useStore.setState({ settings: saved })
    }
    catch { setError("We couldn’t finish setup. Your library is safe; please try again.") }
    finally { setFinishing(false) }
  }
  const next = () => {
    if (index < steps.length - 1) setStep(steps[index + 1])
    else void finish()
  }
  const importSnippets = async () => {
    setImportState({ kind: "working" })
    try {
      for (const source of espanso) {
        if (completedImports.current.has(source.path)) continue
        const result = await api.importEspanso(source.path)
        completedImports.current.set(source.path, result.importedSnippets)
      }
      setImportState({ kind: "done", count: [...completedImports.current.values()].reduce((sum, count) => sum + count, 0) })
    } catch (e) {
      setImportState({ kind: "failed", message: `Couldn’t finish importing. Completed sources won’t be repeated when you retry. ${String(e)}` })
    }
  }
  useEffect(() => {
    if (step !== "practice") stepContent.current?.focus()
  }, [step])

  return (
    <div className="relative flex h-full flex-col items-center overflow-x-hidden overflow-y-auto bg-background">
      <div className="drag absolute inset-x-0 top-0 h-12" data-tauri-drag-region />
      <div aria-hidden className="pointer-events-none absolute inset-0 onboarding-atmosphere" />
      <header className="absolute inset-x-0 top-14 z-10 flex items-center justify-between px-8">
        <Wordmark variant="small" className={cn("w-20 text-muted-foreground", step === "welcome" && "invisible")} />
        {index > 0 && <Button variant="ghost" size="sm" disabled={busy} onClick={() => setStep(steps[index - 1])}><ArrowLeft className="size-3.5" />Back</Button>}
      </header>

      <div ref={stepContent} tabIndex={-1} role="region" aria-label={`Setup: ${stepLabels[step]}`} className="relative my-auto flex w-full max-w-[480px] shrink-0 flex-col gap-8 px-6 py-24 outline-none" key={step}>
        {step === "welcome" && (
          <div className="flex flex-col items-center gap-6 text-center animate-rise">
            <Wordmark className="mb-2 w-56 text-foreground" />
            <div className="flex flex-col gap-2">
              <h1 className="text-[28px] font-semibold tracking-[-0.025em] text-balance">{brand.tagline}</h1>
              <p className="text-sm leading-relaxed text-muted-foreground text-balance">
                {brand.description}
              </p>
            </div>
            <div className="flex w-full items-center gap-3 rounded-xl border border-hairline bg-surface/70 px-4 py-3 text-left shadow-card">
              <span className="rounded-md bg-iris-soft px-2 py-1 font-mono text-xs text-iris-foreground">;sig</span>
              <ArrowRight className="size-3.5 shrink-0 text-faint" />
              <span className="text-xs leading-relaxed text-muted-foreground">Best regards,<br /><span className="text-foreground">Your Name</span></span>
            </div>
            <Button size="lg" className="h-10 px-5" onClick={next}>
              Get started <ArrowRight />
            </Button>
            <p className="-mt-3 text-[11px] text-faint">A few small steps, then back to your words.</p>
          </div>
        )}

        {step === "permissions" && (
          <div className="flex flex-col gap-6 animate-rise">
            <div className="flex flex-col gap-2">
              <h1 className="text-xl font-semibold tracking-tight">Let Kudzu type for you</h1>
              <p className="text-sm text-muted-foreground">Allow access once in macOS. We’ll guide you through it and detect when you’re ready.</p>
            </div>
            <PermissionSetup status={status} />
            <div className="flex justify-between">
              <Button variant="ghost" onClick={next}>
                Skip for now
              </Button>
              <Button onClick={next} disabled={!permissionsReady(status)}>
                Continue <ArrowRight />
              </Button>
            </div>
          </div>
        )}

        {step === "import" && (
          <div className="flex flex-col gap-6 animate-rise">
            <div className="flex flex-col gap-2">
              <h1 className="text-xl font-semibold tracking-tight">Bring your Espanso snippets?</h1>
              <p className="text-sm text-muted-foreground">
                We found {espanso.reduce((n, d) => n + d.snippetCount, 0)} snippets. Forms, dates and clipboard variables come along; anything Kudzu can&apos;t run yet is kept as text.
              </p>
            </div>
            {importState.kind === "done" ? (
              <div className="flex items-center justify-between rounded-xl bg-success/10 px-4 py-3 text-sm text-success" role="status">
                <span className="flex items-center gap-2"><Check className="size-4 animate-check" />Imported {importState.count} snippets</span>
                <Button size="sm" onClick={next} autoFocus>Continue<ArrowRight /></Button>
              </div>
            ) : (
              <>
                {importState.kind === "failed" && <p role="alert" className="rounded-lg bg-destructive/10 p-3 text-xs text-destructive">{importState.message}</p>}
                <div className="flex justify-between">
                  <Button variant="ghost" onClick={next} disabled={busy}>Not now</Button>
                  <Button disabled={busy} onClick={() => void importSnippets()}>
                    {importState.kind === "working" ? <><Loader2 className="animate-spin motion-reduce:animate-none" />Importing…</> : importState.kind === "failed" ? "Retry import" : "Import snippets"}
                  </Button>
                </div>
              </>
            )}
          </div>
        )}

        {step === "practice" && <Practice onDone={next} hotkey={settings.paletteHotkey} finishing={finishing} />}

        {error && <p role="alert" className="rounded-lg bg-destructive/10 p-3 text-xs text-destructive">{error}</p>}
        <ol className="flex justify-center gap-5" aria-label="Setup progress">
          {steps.map((item, i) => (
            <li key={item} aria-current={item === step ? "step" : undefined} className={cn("flex items-center gap-1.5 text-[10px] transition-colors", i === index ? "text-iris-foreground" : "text-faint")}>
              {i < index ? <Check className="size-3 text-iris animate-check" /> : <span className={cn("size-1.5 rounded-full", i === index ? "bg-iris" : "bg-foreground/15")} />}
              {stepLabels[item]}
            </li>
          ))}
        </ol>
      </div>
    </div>
  )
}

function Practice({ onDone, hotkey, finishing }: { onDone: () => void; hotkey: string; finishing: boolean }) {
  const [value, setValue] = useState("")
  const ref = useRef<HTMLTextAreaElement>(null)
  const success = value.includes(SAMPLE_TEXT)
  useEffect(() => ref.current?.focus(), [])

  return (
    <div className="flex flex-col gap-6 animate-rise">
      <div className="flex flex-col gap-2">
        <h1 className="text-xl font-semibold tracking-tight">Try a preview</h1>
        <p className="text-sm text-muted-foreground">
          Type <span className="rounded bg-iris-soft px-1 font-mono text-iris-foreground">{SAMPLE_TRIGGER}</span> below to see how expansion feels.
        </p>
      </div>
      <div className={cn("rounded-xl bg-surface shadow-card transition-shadow", success && "shadow-[0_0_0_1px_var(--success),0_0_24px_-6px_var(--success)]")}>
        <textarea
          ref={ref}
          value={value}
          aria-label="Practice field"
          onChange={(e) => {
            let v = e.target.value
            // Kudzu never expands inside its own windows (so editing a snippet can't trigger
            // one), so this field shows the expansion itself.
            if (v.toLowerCase().endsWith(SAMPLE_TRIGGER)) v = v.slice(0, -SAMPLE_TRIGGER.length) + SAMPLE_TEXT
            setValue(v)
          }}
          disabled={finishing}
          placeholder={`Type ${SAMPLE_TRIGGER} here…`}
          className="block h-28 w-full resize-none bg-transparent p-4 text-[15px] outline-none placeholder:text-faint"
        />
      </div>
      {success ? (
        <div className="flex flex-col gap-4 animate-rise">
          <p className="flex items-center gap-2 text-sm font-medium text-success">
            <Sparkles className="size-4 animate-star" /> That&apos;s how a snippet expands.
          </p>
          <p className="text-xs text-muted-foreground">
            {isTauri() ? "When expansion is enabled, your library’s triggers work in Notes, Mail, your browser, and other apps." : "In the desktop app this works in every app."}
          </p>
          <p className="flex flex-wrap items-center gap-1.5 text-xs text-muted-foreground">
            Forgot a trigger? Press
            <Shortcut keys={acceleratorLabels(hotkey)} size="md" />
            anywhere to search your snippets.
          </p>
          <Button onClick={onDone} disabled={finishing} className="self-end">
            {finishing ? "Opening…" : "Open library"} <ArrowRight />
          </Button>
        </div>
      ) : (
        <div className="flex justify-end">
          <Button variant="ghost" onClick={onDone} disabled={finishing}>
            Skip
          </Button>
        </div>
      )}
    </div>
  )
}
