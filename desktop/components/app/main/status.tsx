"use client"

import { Pause, Play, ShieldAlert } from "lucide-react"

import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { acceleratorLabels } from "@/lib/keys"
import { useStore } from "@/lib/store"
import type { EngineStatus } from "@/lib/types"
import { cn } from "@/lib/utils"

import { Shortcut } from "../shortcut"

export type Health = "ready" | "paused" | "limited" | "blocked" | "secure"

export function health(status: EngineStatus, platform: string): Health {
  if (status.paused) return "paused"
  if (platform === "web") return "ready"
  if (status.secureInput) return "secure"
  if (status.tap === "off" || !status.permissions.inputMonitoring && !status.permissions.accessibility) return "blocked"
  if (!status.permissions.accessibility || status.tap === "listenOnly") return "limited"
  return "ready"
}

const COPY: Record<Health, { label: string; detail: string; dot: string }> = {
  ready: { label: "Expanding", detail: "Kudzu is listening for your triggers in every app.", dot: "bg-success shadow-[0_0_8px_var(--success)]" },
  paused: { label: "Paused", detail: "Nothing expands until you resume.", dot: "bg-warning" },
  limited: { label: "Limited", detail: "Grant Accessibility so Kudzu can type expansions reliably.", dot: "bg-warning" },
  blocked: { label: "Needs permission", detail: "Kudzu can't see your typing yet. Open Settings → Permissions.", dot: "bg-destructive" },
  secure: { label: "Secure Input on", detail: "A password field or app has Secure Input on, so macOS blocks expansion.", dot: "bg-warning" },
}

export function StatusIndicator() {
  const status = useStore((s) => s.status)
  const platform = useStore((s) => s.platform)
  const settings = useStore((s) => s.settings)
  const setPaused = useStore((s) => s.setPaused)
  const setView = useStore((s) => s.setView)
  const h = health(status, platform)
  const copy = COPY[h]
  const detail = h === "secure" && status.secureInputOwner ? `${status.secureInputOwner} has Secure Input on, so macOS blocks expansion.` : copy.detail

  return (
    <div className="flex items-center gap-1 border-t border-hairline px-2.5 py-2">
      <Tooltip>
        <TooltipTrigger asChild>
          <button
            onClick={() => (h === "blocked" || h === "limited" ? setView({ kind: "settings", section: "general" }) : undefined)}
            className="flex min-w-0 flex-1 items-center gap-2 rounded-md px-2 py-1 text-left text-xs hover:bg-sidebar-accent"
            aria-label={`Status: ${copy.label}. ${detail}`}
          >
            <span className={cn("size-1.5 shrink-0 rounded-full", copy.dot)} aria-hidden />
            <span className="truncate font-medium text-sidebar-foreground/80">{copy.label}</span>
            {(h === "blocked" || h === "secure") && <ShieldAlert className="size-3.5 text-warning" />}
          </button>
        </TooltipTrigger>
        <TooltipContent side="top" className="max-w-64">
          <p>{detail}</p>
          <p className="mt-1 flex items-center gap-1 opacity-70">
            Quick search anywhere:{" "}
            <Shortcut keys={acceleratorLabels(settings.paletteHotkey)} />
          </p>
        </TooltipContent>
      </Tooltip>
      <Tooltip>
        <TooltipTrigger asChild>
          <button
            onClick={() => setPaused(!status.paused)}
            aria-label={status.paused ? "Resume expansion" : "Pause expansion"}
            aria-pressed={status.paused}
            className="flex size-7 items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-sidebar-accent hover:text-foreground"
          >
            {status.paused ? <Play className="size-3.5" /> : <Pause className="size-3.5" />}
          </button>
        </TooltipTrigger>
        <TooltipContent side="top">
          {status.paused ? "Resume" : "Pause"} <Shortcut combo="mod+shift+p" />
        </TooltipContent>
      </Tooltip>
    </div>
  )
}
