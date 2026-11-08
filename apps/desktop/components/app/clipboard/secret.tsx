"use client"

// Secrets in clipboard history (passwords, keys, tokens): hidden until the pointer rests on them,
// unless Settings ▸ Clipboard ▸ Show secrets is on. Detection runs in Rust when an item is copied;
// the user can correct it per item.

import { EyeOff, KeyRound, ShieldCheck, ShieldOff } from "lucide-react"
import { toast } from "sonner"

import { isSecret, secretReason, useClipboard } from "@/lib/clipboard"
import { useStore } from "@/lib/store"
import type { ClipEntry } from "@/lib/types"
import { cn } from "@/lib/utils"

/** Whether to hide this entry's content: it's a secret and "Show secrets" is off. */
export function useConcealed(entry: ClipEntry): boolean {
  const show = useStore((s) => s.settings.clipboardShowSecrets)
  return isSecret(entry) && !show
}

/** Dots standing in for hidden text, about as long as the text (capped). */
export function dots(length: number): string {
  return "•".repeat(Math.min(Math.max(length, 8), 18))
}

/**
 * Text that shows as dots until the pointer is over the enclosing `group/row`. Used for list
 * rows, where hovering the row is the natural gesture.
 */
export function RowSecret({ children, concealed, stand }: { children: React.ReactNode; concealed: boolean; stand: string }) {
  if (!concealed) return <>{children}</>
  return (
    <>
      <span aria-hidden className="group-hover/row:hidden">
        {stand}
      </span>
      <span className="hidden group-hover/row:inline">{children}</span>
    </>
  )
}

/**
 * Hides a preview behind a cover until it's hovered or focused. The content keeps its layout
 * (so nothing jumps on reveal) but isn't painted, selectable or read out while covered.
 */
export function Concealed({ entry, children }: { entry: ClipEntry; children: React.ReactNode }) {
  const concealed = useConcealed(entry)
  if (!concealed) return <>{children}</>
  return (
    <div
      // Focusable so the keyboard can reveal it too.
      // oxlint-disable-next-line jsx-a11y/no-noninteractive-tabindex -- reveal on focus
      tabIndex={0}
      aria-label={`Hidden: ${secretReason(entry).toLowerCase()}. Hover or focus to show.`}
      className="group/secret relative min-h-28 rounded-xl outline-none focus-visible:ring-2 focus-visible:ring-ring"
    >
      <div className="invisible group-hover/secret:visible group-focus-visible/secret:visible">{children}</div>
      <div
        aria-hidden
        className="absolute inset-0 flex flex-col items-center justify-center gap-1.5 rounded-xl bg-foreground/[0.025] text-center shadow-well group-hover/secret:hidden group-focus-visible/secret:hidden"
      >
        <span className="flex size-9 items-center justify-center rounded-xl bg-foreground/[0.06] text-muted-foreground">
          <EyeOff className="size-4" />
        </span>
        <span className="text-[13px] font-medium">{secretReason(entry)}</span>
        <span className="text-xs text-faint">Hover to show</span>
      </div>
    </div>
  )
}

/** Marks an entry as a secret or not, with an Undo toast that says what changed. */
export function setSecretWithFeedback(entry: ClipEntry, secret: boolean) {
  const before = entry.secretOverride ?? null
  // Matching Kudzu's own detection clears the correction instead of storing a redundant one.
  const detected = entry.secret !== undefined
  const next = secret === detected ? null : secret
  void useClipboard.getState().setSecret(entry.id, next)
  toast.success(secret ? "Hidden as a secret" : "Marked as not a secret", {
    description: secret
      ? "It stays hidden until you hover over it, and AI agents can't see it."
      : "It's shown normally, and AI agents can read it if you share clipboard history with them.",
    action: { label: "Undo", onClick: () => void useClipboard.getState().setSecret(entry.id, before) },
    duration: 6000,
  })
}

/** Heading badge for a secret, with a one-click "Not a secret?" correction. */
export function SecretBadge({ entry }: { entry: ClipEntry }) {
  if (!isSecret(entry)) return null
  return (
    <span className="flex min-w-0 items-center gap-1.5">
      <span className="flex items-center gap-1 rounded-md bg-warning/12 px-1.5 py-0.5 text-[11px] font-medium text-warning">
        <KeyRound className="size-3" /> Secret
      </span>
      <button
        type="button"
        onClick={() => setSecretWithFeedback(entry, false)}
        className="truncate rounded px-1 text-[11px] text-faint underline-offset-2 outline-none hover:text-muted-foreground hover:underline focus-visible:ring-2 focus-visible:ring-ring"
      >
        Not a secret?
      </button>
    </span>
  )
}

/** Icon for the "secret / not a secret" toolbar toggle. */
export function SecretToggleIcon({ entry, className }: { entry: ClipEntry; className?: string }) {
  return isSecret(entry) ? <ShieldOff className={cn(className)} /> : <ShieldCheck className={cn(className)} />
}
