"use client"

import { ArrowUpRight, Check, Keyboard, ShieldCheck } from "lucide-react"
import { useState } from "react"

import { Button } from "@/components/ui/button"
import { api } from "@/lib/backend"
import type { EngineStatus } from "@/lib/types"

export function permissionsReady(status: EngineStatus) {
  return status.permissions.accessibility && status.canPostEvents && status.tap !== "off"
}

export function PermissionSetup({ status }: { status: EngineStatus }) {
  const [error, setError] = useState<string | null>(null)
  const [opening, setOpening] = useState(false)
  const ready = permissionsReady(status)
  const open = async () => {
    setOpening(true)
    setError(null)
    try { await api.showPermissions() }
    catch (e) { setError(String(e)) }
    finally { setOpening(false) }
  }
  return (
    <div className="rounded-xl bg-surface p-4 shadow-card">
      <div className="flex items-center gap-3">
        <span className="flex size-10 items-center justify-center rounded-xl bg-iris-soft text-iris-foreground"><ShieldCheck className="size-5" /></span>
        <div className="flex-1">
          <p className="text-[13px] font-semibold">{ready ? "Permissions are set" : "Enable typing in other apps"}</p>
          <p className="mt-0.5 text-xs text-muted-foreground">{ready ? status.canExpand ? "Kudzu is ready to expand your snippets." : status.summary : "A guided setup, with every step beside you."}</p>
        </div>
      </div>
      <div className="my-4 flex items-center gap-2 text-xs text-muted-foreground">
        {status.permissions.accessibility ? <Check className="size-3.5 text-success" /> : <ShieldCheck className="size-3.5" />}
        Accessibility
        <span className="mx-1 text-faint">·</span>
        {status.permissions.inputMonitoring ? <Check className="size-3.5 text-success" /> : <Keyboard className="size-3.5" />}
        Input Monitoring
      </div>
      <Button className="w-full" variant={ready ? "outline" : "default"} onClick={() => void open()} disabled={opening}>
        {opening ? "Opening…" : ready ? "Review permissions" : "Set up permissions"}<ArrowUpRight />
      </Button>
      {error && <p role="alert" className="mt-3 text-xs text-destructive">{error}</p>}
    </div>
  )
}
