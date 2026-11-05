import { Kbd, KbdGroup } from "@/components/ui/kbd"
import { isMac, keyLabels } from "@/lib/keys"
import { cn } from "@/lib/utils"

const TONES = {
  /** Keycap on a regular surface. Tooltips and filled buttons restyle it automatically (see Kbd). */
  default: "",
  /** Hint that should recede, e.g. revealed on hover. */
  ghost: "bg-transparent text-faint shadow-none",
  /** On an accent fill that isn't a shadcn Button (custom iris buttons). */
  onAccent: "bg-black/15 text-current shadow-[inset_0_1px_1px_oklch(0_0_0/0.15)]",
  /** Highlighted, e.g. the global quick-search shortcut. */
  accent: "bg-iris-soft text-iris-foreground shadow-none",
} as const

/**
 * A keyboard shortcut drawn with shadcn's Kbd. Pass a combo ("mod+shift+k") or pre-made labels
 * (`keys`, e.g. from a recorded accelerator). Hidden from screen readers: controls announce
 * their shortcut through aria-keyshortcuts instead.
 */
export function Shortcut({
  combo,
  keys,
  className,
  tone = "default",
  size = "sm",
  spoken = false,
}: {
  combo?: string
  keys?: string[]
  className?: string
  tone?: keyof typeof TONES
  size?: "sm" | "md"
  /** Set when the shortcut sits inside a sentence, so screen readers still hear it ("Command Return"). */
  spoken?: boolean
}) {
  const labels = keys ?? (combo ? keyLabels(combo) : [])
  const caps = (
    <KbdGroup aria-hidden className={cn("gap-0.5", className)}>
      {labels.map((k, i) => (
        <Kbd key={i} className={cn(size === "sm" && "h-[18px] min-w-[18px] text-[10.5px]", TONES[tone])}>
          {k}
        </Kbd>
      ))}
    </KbdGroup>
  )
  if (!spoken || !combo) return caps
  return (
    <>
      {caps}
      <span className="sr-only">{spokenShortcut(combo)}</span>
    </>
  )
}

/** Words for a combo, e.g. "mod+enter" → "Command Return". */
export function spokenShortcut(combo: string): string {
  const words: Record<string, string> = {
    mod: isMac ? "Command" : "Control",
    ctrl: "Control",
    shift: "Shift",
    alt: isMac ? "Option" : "Alt",
    enter: isMac ? "Return" : "Enter",
    escape: "Escape",
    esc: "Escape",
    tab: "Tab",
    space: "Space",
    backspace: "Delete",
    up: "Up Arrow",
    down: "Down Arrow",
    left: "Left Arrow",
    right: "Right Arrow",
  }
  return combo
    .split("+")
    .map((k) => words[k.toLowerCase()] ?? k.toUpperCase())
    .join(" ")
}

/** Accessible description for a shortcut, e.g. aria-keyshortcuts="Meta+N". */
export function ariaShortcut(combo: string): string {
  return combo
    .split("+")
    .map((k) => ({ mod: "Meta", shift: "Shift", alt: "Alt", ctrl: "Control", enter: "Enter", backspace: "Backspace" })[k] ?? k.toUpperCase())
    .join("+")
}
