"use client"

import { ArrowRight, Check, Sparkles } from "lucide-react"
import { useEffect, useRef, useState } from "react"

import { Button } from "@/components/ui/button"
import { api, isTauri } from "@/lib/backend"
import { acceleratorLabels } from "@/lib/keys"
import { useStore } from "@/lib/store"
import type { EspansoDetection } from "@/lib/types"
import { cn } from "@/lib/utils"

import { Shortcut } from "../shortcut"

type Step = "welcome" | "permissions" | "import" | "practice"

const SAMPLE_TRIGGER = ";kudzu"
const SAMPLE_TEXT = "Kudzu is working! 🌱"

export function Onboarding() {
  const status = useStore((s) => s.status)
  const platform = useStore((s) => s.platform)
  const settings = useStore((s) => s.settings)
  const update = useStore((s) => s.updateSettings)
  const [step, setStep] = useState<Step>("welcome")
  const [espanso, setEspanso] = useState<EspansoDetection[]>([])
  const [imported, setImported] = useState<number | null>(null)

  useEffect(() => {
    api.detectEspanso().then(setEspanso, () => {})
  }, [])

  const needsPermissions = platform === "macos"
  const steps: Step[] = ["welcome", ...(needsPermissions ? (["permissions"] as Step[]) : []), ...(espanso.length ? (["import"] as Step[]) : []), "practice"]
  const index = steps.indexOf(step)
  const next = () => (index < steps.length - 1 ? setStep(steps[index + 1]) : void update({ onboardingCompleted: true }))
  const perms = status.permissions

  return (
    <div className="relative flex h-full flex-col items-center justify-center overflow-hidden bg-background">
      <div className="drag absolute inset-x-0 top-0 h-12" data-tauri-drag-region />
      {/* Ambient glow */}
      <div aria-hidden className="pointer-events-none absolute -top-40 left-1/2 h-[480px] w-[720px] -translate-x-1/2 rounded-full bg-iris/[0.07] blur-3xl" />

      <div className="relative flex w-full max-w-[460px] flex-col gap-8 px-6" key={step}>
        {step === "welcome" && (
          <div className="flex flex-col items-center gap-6 text-center animate-rise">
      {/* eslint-disable-next-line @next/next/no-img-element -- static export */}
      <img src="/icon.png" alt="" width={88} height={88} className="-my-2 drop-shadow-[0_12px_24px_oklch(0.5_0.15_282/0.35)]" draggable={false} />
            <div className="flex flex-col gap-2">
              <h1 className="text-[28px] font-semibold tracking-[-0.025em] text-balance">Type less. Say more.</h1>
              <p className="text-sm leading-relaxed text-muted-foreground text-balance">
                Kudzu turns short triggers like <span className="rounded bg-iris-soft px-1 font-mono text-iris-foreground">;sig</span> into the text you write every day — in every app.
              </p>
            </div>
            <Button size="lg" className="h-10 px-5" onClick={next} autoFocus>
              Get started <ArrowRight />
            </Button>
          </div>
        )}

        {step === "permissions" && (
          <div className="flex flex-col gap-6 animate-rise">
            <div className="flex flex-col gap-2">
              <h1 className="text-xl font-semibold tracking-tight">Two quick permissions</h1>
              <p className="text-sm text-muted-foreground">macOS needs your OK before any app can react to typing. Kudzu only looks for your triggers — it never records or sends what you type.</p>
            </div>
            <div className="flex flex-col gap-2">
              <PermissionCard
                n={1}
                title="Input Monitoring"
                body="So Kudzu can notice when you finish a trigger."
                granted={perms.inputMonitoring}
                onGrant={() => void api.requestPermission("inputMonitoring").then(() => api.openSystemSettings("inputMonitoring"))}
              />
              <PermissionCard
                n={2}
                title="Accessibility"
                body="So Kudzu can swap the trigger for your text."
                granted={perms.accessibility}
                onGrant={() => void api.requestPermission("accessibility").then(() => api.openSystemSettings("accessibility"))}
              />
            </div>
            <p className="text-xs text-faint">This page updates by itself once you flip the switches in System Settings.</p>
            <div className="flex justify-between">
              <Button variant="ghost" onClick={next}>
                Skip for now
              </Button>
              <Button onClick={next} disabled={!perms.inputMonitoring && !perms.accessibility}>
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
            {imported === null ? (
              <div className="flex justify-between">
                <Button variant="ghost" onClick={next}>
                  Not now
                </Button>
                <Button
                  autoFocus
                  onClick={async () => {
                    let n = 0
                    for (const d of espanso) n += (await api.importEspanso(d.path)).importedSnippets
                    setImported(n)
                  }}
                >
                  Import snippets
                </Button>
              </div>
            ) : (
              <div className="flex items-center justify-between rounded-xl bg-success/10 px-4 py-3 text-sm text-success">
                <span className="flex items-center gap-2">
                  <Check className="size-4" /> Imported {imported} snippets
                </span>
                <Button size="sm" onClick={next} autoFocus>
                  Continue
                </Button>
              </div>
            )}
          </div>
        )}

        {step === "practice" && <Practice onDone={next} hotkey={settings.paletteHotkey} />}

        <div className="flex justify-center gap-1.5" aria-label={`Step ${index + 1} of ${steps.length}`}>
          {steps.map((s, i) => (
            <span key={s} className={cn("h-1 rounded-full transition-all duration-300", i === index ? "w-5 bg-iris" : "w-1.5 bg-foreground/15")} />
          ))}
        </div>
      </div>
    </div>
  )
}

function PermissionCard({ n, title, body, granted, onGrant }: { n: number; title: string; body: string; granted: boolean; onGrant: () => void }) {
  return (
    <div className={cn("flex items-center gap-3 rounded-xl bg-surface p-3.5 shadow-card transition-colors", granted && "bg-success/[0.06]")}>
      <span className={cn("flex size-7 items-center justify-center rounded-full text-xs font-semibold", granted ? "bg-success text-background" : "bg-foreground/[0.07] text-muted-foreground")}>
        {granted ? <Check className="size-4" /> : n}
      </span>
      <div className="min-w-0 flex-1">
        <div className="text-[13px] font-medium">{title}</div>
        <div className="text-xs text-muted-foreground">{body}</div>
      </div>
      {granted ? (
        <span className="text-xs font-medium text-success">Allowed</span>
      ) : (
        <Button size="sm" onClick={onGrant}>
          Open Settings
        </Button>
      )}
    </div>
  )
}

function Practice({ onDone, hotkey }: { onDone: () => void; hotkey: string }) {
  const [value, setValue] = useState("")
  const ref = useRef<HTMLTextAreaElement>(null)
  const success = value.includes(SAMPLE_TEXT)
  useEffect(() => ref.current?.focus(), [])

  return (
    <div className="flex flex-col gap-6 animate-rise">
      <div className="flex flex-col gap-2">
        <h1 className="text-xl font-semibold tracking-tight">Try it</h1>
        <p className="text-sm text-muted-foreground">
          Type <span className="rounded bg-iris-soft px-1 font-mono text-iris-foreground">{SAMPLE_TRIGGER}</span> below and watch it expand.
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
          placeholder={`Type ${SAMPLE_TRIGGER} here…`}
          className="block h-28 w-full resize-none bg-transparent p-4 text-[15px] outline-none placeholder:text-faint"
        />
      </div>
      {success ? (
        <div className="flex flex-col gap-4 animate-rise">
          <p className="flex items-center gap-2 text-sm font-medium text-success">
            <Sparkles className="size-4" /> That&apos;s it — you&apos;re set.
          </p>
          <p className="text-xs text-muted-foreground">
            {isTauri() ? "Now try it for real in any other app — Notes, Mail, Slack, your browser." : "In the desktop app this works in every app."}
          </p>
          <p className="flex flex-wrap items-center gap-1.5 text-xs text-muted-foreground">
            Forgot a trigger? Press
            <Shortcut keys={acceleratorLabels(hotkey)} size="md" />
            anywhere to search your snippets.
          </p>
          <Button onClick={onDone} autoFocus className="self-end">
            Open Kudzu <Shortcut combo="enter" />
          </Button>
        </div>
      ) : (
        <div className="flex justify-end">
          <Button variant="ghost" onClick={onDone}>
            Skip
          </Button>
        </div>
      )}
    </div>
  )
}
