"use client"

import { ArrowLeft, ArrowRight, Check, Download, Loader2, Sparkles } from "lucide-react"
import { useEffect, useRef, useState } from "react"

import brand from "@/design/brand.json"
import { Button } from "@/components/ui/button"
import { api, isTauri } from "@/lib/backend"
import { acceleratorLabels } from "@/lib/keys"
import { useStore } from "@/lib/store"
import type { EspansoDetection, ImportSummary } from "@/lib/types"
import { cn } from "@/lib/utils"

import { IMPORT_HINTS, pickEspansoFolder, pickImportFile } from "./data-actions"
import { Shortcut } from "../shortcut"
import { Wordmark } from "../wordmark"
import { PermissionSetup, permissionsReady } from "../permissions/permission-setup"

type Step = "welcome" | "permissions" | "import" | "practice"

type Source = "espanso" | "raycast" | "textexpander"
type Outcome = { count: number; changed: number; firstChange?: string }
const ACTION_WIDTH = "min-w-28"
const fileSources: { id: Exclude<Source, "espanso">; name: string }[] = [
  { id: "raycast", name: "Raycast" },
  { id: "textexpander", name: "TextExpander" },
]
const stepLabels: Record<Step, string> = { welcome: "Welcome", permissions: "Access", import: "Import", practice: "Try it" }

const SAMPLE_TRIGGER = ";kudzu"
const SAMPLE_TEXT = "Hello from Kudzu!"

export function Onboarding() {
  const status = useStore((s) => s.status)
  const platform = useStore((s) => s.platform)
  const settings = useStore((s) => s.settings)
  const [step, setStep] = useState<Step>("welcome")
  const [espanso, setEspanso] = useState<EspansoDetection[]>([])
  const completedImports = useRef(new Set<string>())
  const [working, setWorking] = useState<Source | null>(null)
  const [picking, setPicking] = useState(false)
  const continueButton = useRef<HTMLButtonElement>(null)
  const focusAfterImport = useRef<Element | null>(null)
  const [outcomes, setOutcomes] = useState<Partial<Record<Source, Outcome>>>({})
  const [failures, setFailures] = useState<Partial<Record<Source, string>>>({})
  const [detection, setDetection] = useState<"looking" | "done" | "failed">("looking")
  const [finishing, setFinishing] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const stepContent = useRef<HTMLDivElement>(null)

  useEffect(() => {
    api.detectEspanso().then(
      (found) => {
        setEspanso(found)
        setDetection("done")
      },
      () => setDetection("failed"),
    )
  }, [])

  const needsPermissions = platform === "macos" || platform === "web"
  const steps: Step[] = ["welcome"]
  if (needsPermissions) steps.push("permissions")
  steps.push("import")
  steps.push("practice")
  const index = steps.indexOf(step)
  const busy = finishing || picking || working !== null
  // Rows show each source's latest result; this running total decides Continue vs. Skip.
  const [imported, setImported] = useState(0)
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
  const espansoCount = espanso.reduce((n, d) => n + d.snippetCount, 0)
  // Each source shows its latest result: how many snippets were new, not how many were in the file.
  const record = (source: Source, result: ImportSummary, accumulate = false) => {
    setImported((total) => total + newSnippets(result))
    setOutcomes((all) => {
      const added = newSnippets(result)
      const before = accumulate ? all[source] : undefined
      return {
        ...all,
        [source]: {
          count: (before?.count ?? 0) + added,
          changed: (before?.changed ?? 0) + result.warnings.length,
          firstChange: before?.firstChange ?? result.warnings[0]?.message,
        },
      }
    })
  }
  const fail = (source: Source, e: unknown, prefix = "") =>
    setFailures((all) => ({ ...all, [source]: `${prefix}${String(e).replace(/^Error:\s*/, "")}` }))
  // Runs one source's import; `run` throws to report a failure on that source's row.
  const runSource = async (source: Source, run: () => Promise<void>) => {
    setWorking(source)
    setFailures((all) => ({ ...all, [source]: undefined }))
    try {
      await run()
    } finally {
      setWorking(null)
    }
  }
  const importDetectedEspanso = async () => {
    focusAfterImport.current = document.activeElement
    await runSource("espanso", async () => {
      for (const source of espanso) {
        if (completedImports.current.has(source.path)) continue
        try {
          record("espanso", await api.importEspanso(source.path), true)
          completedImports.current.add(source.path)
        } catch (e) {
          const done = completedImports.current.size
          fail("espanso", e, done ? `Imported ${done} of ${espanso.length} folders; retry brings in the rest. ` : "")
          return
        }
      }
    })
  }
  const chooseEspansoFolder = async () => {
    focusAfterImport.current = document.activeElement
    setPicking(true)
    try {
      // The browser preview has no file dialog, so it imports a sample instead.
      const path = isTauri() ? await pickEspansoFolder() : "preview://espanso/work"
      setPicking(false)
      if (path) {
        await runSource("espanso", async () => {
          try {
            record("espanso", await api.importEspanso(path))
          } catch (e) {
            fail("espanso", e)
          }
        })
      }
    } catch (e) {
      setPicking(false)
      fail("espanso", e)
    }
  }
  const importFromFile = async (source: Exclude<Source, "espanso">) => {
    focusAfterImport.current = document.activeElement
    setPicking(true)
    try {
      const path = isTauri() ? await pickImportFile(source) : `preview://${source}`
      setPicking(false)
      if (path) {
        await runSource(source, async () => {
          try {
            record(source, await api.importFile(path))
          } catch (e) {
            fail(source, e)
          }
        })
      }
    } catch (e) {
      setPicking(false)
      fail(source, e)
    }
  }
  // The pressed button is disabled while a dialog or import is busy, which drops keyboard focus.
  // Give it back afterwards, or move it to Continue when that button has nothing left to do.
  useEffect(() => {
    const previous = focusAfterImport.current
    if (!previous || working !== null || picking) return
    focusAfterImport.current = null
    // Chrome leaves a disabled button as the active element instead of blurring it.
    const active = document.activeElement
    const lost = !active || active === document.body || (active instanceof HTMLButtonElement && active.disabled)
    if (!lost) return
    const back = previous instanceof HTMLButtonElement && previous.isConnected && !previous.disabled ? previous : continueButton.current
    back?.focus()
  }, [working, picking])
  useEffect(() => {
    if (step !== "practice") stepContent.current?.focus()
  }, [step])

  return (
    <div className="relative flex h-full flex-col items-center overflow-x-hidden overflow-y-auto bg-background [scrollbar-gutter:stable]">
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
              <h1 className="text-xl font-semibold tracking-tight">Bring your snippets?</h1>
              <p className="text-sm text-muted-foreground">
                Switching from another expander? Forms, dates and clipboard variables come along; anything Kudzu can&apos;t run yet is kept as text. A backup is saved first.
              </p>
            </div>
            <p role="status" className="sr-only">{imported > 0 ? `Imported ${imported} snippet${imported === 1 ? "" : "s"}` : ""}</p>
            <ul className="flex flex-col gap-2" aria-label="Import sources">
              <SourceRow
                name="Espanso"
                detail={
                  detection === "looking" ? "Looking on this Mac…"
                  : espanso.length ? `Found ${espansoCount} snippets on this Mac.`
                  : detection === "failed" ? "Couldn’t check this Mac. Choose your Espanso folder instead."
                  : outcomes.espanso ? "From the folder you chose."
                  : "Not found on this Mac. If you keep it elsewhere, choose its folder."
                }
                outcome={outcomes.espanso}
                error={failures.espanso}
              >
                {espanso.length > 0 ? (
                  <Button size="sm" className={ACTION_WIDTH} aria-label={outcomes.espanso && !failures.espanso ? "Espanso imported" : `${failures.espanso ? "Retry" : "Import"} Espanso snippets`} disabled={busy || (!!outcomes.espanso && !failures.espanso)} onClick={() => void importDetectedEspanso()}>
                    {working === "espanso" ? <Loader2 className="animate-spin motion-reduce:animate-none" /> : <Download />}
                    {outcomes.espanso && !failures.espanso ? "Imported" : failures.espanso ? "Retry" : "Import"}
                  </Button>
                ) : (
                  <Button size="sm" variant="secondary" className={ACTION_WIDTH} aria-label="Choose folder… for Espanso" disabled={busy || detection === "looking"} onClick={() => void chooseEspansoFolder()}>
                    {working === "espanso" && <Loader2 className="animate-spin motion-reduce:animate-none" />}Choose folder…
                  </Button>
                )}
              </SourceRow>
              {fileSources.map((source) => (
                <SourceRow key={source.id} name={source.name} detail={IMPORT_HINTS[source.id]} outcome={outcomes[source.id]} error={failures[source.id]}>
                  <Button size="sm" variant="secondary" className={ACTION_WIDTH} aria-label={`${failures[source.id] ? "Try again…" : "Choose file…"} for ${source.name}`} disabled={busy} onClick={() => void importFromFile(source.id)}>
                    {working === source.id && <Loader2 className="animate-spin motion-reduce:animate-none" />}
                    {failures[source.id] ? "Try again…" : "Choose file…"}
                  </Button>
                </SourceRow>
              ))}
            </ul>
            <div className="flex justify-end">
              {imported > 0 ? (
                <Button key="continue" ref={continueButton} onClick={next} disabled={busy}>Continue<ArrowRight /></Button>
              ) : (
                <Button key="skip" variant="ghost" onClick={next} disabled={busy}>Skip for now</Button>
              )}
            </div>
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

/** Snippets a result really added: the source's count minus what deduplication merged away. */
function newSnippets(result: ImportSummary) {
  return Math.max(0, result.importedSnippets - (result.dedupeReport?.mergedSnippets ?? 0))
}

function SourceRow({ name, detail, outcome, error, children }: { name: string; detail: string; outcome?: Outcome; error?: string; children?: React.ReactNode }) {
  return (
    <li className="flex flex-col gap-2 rounded-xl border border-hairline bg-surface/70 px-4 py-3 shadow-card">
      <div className="flex items-center justify-between gap-4">
        <div className="flex min-w-0 flex-col gap-0.5">
          <span className="text-sm font-medium">{name}</span>
          <span className="text-xs text-muted-foreground">{detail}</span>
        </div>
        <div className="shrink-0">{children}</div>
      </div>
      {outcome && (
        <p className={cn("flex items-center gap-1.5 text-xs", outcome.count ? "text-success" : "text-muted-foreground")}>
          {outcome.count > 0 && <Check className="size-3.5 animate-check" />}
          {outcome.count ? `Imported ${outcome.count} snippet${outcome.count === 1 ? "" : "s"}` : "No new snippets to add"}
        </p>
      )}
      {outcome && outcome.changed > 0 && (
        <p className="text-xs text-muted-foreground">
          {outcome.changed} item{outcome.changed === 1 ? "" : "s"} need{outcome.changed === 1 ? "s" : ""} a look — e.g. {outcome.firstChange}
        </p>
      )}
      {error && <p role="alert" className="text-xs text-destructive">{error}</p>}
    </li>
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
