// Footer key hints that react to their key (the `key-hint` utility in globals.css). Driven straight
// through the DOM so holding or pressing a key never re-renders the window.

/** Which hint a key press belongs to: matches the `data-hint` on the footer spans. */
export function hintFor(e: React.KeyboardEvent, mod: boolean): string | null {
  const k = e.key
  if (k === "Enter") return mod ? "mod+enter" : e.shiftKey ? "shift+enter" : "enter"
  if (k === "Tab") return "tab"
  if (k === "Escape") return "escape"
  if (mod && (k === "Backspace" || k === "Delete")) return "mod+backspace"
  if (mod && k.length === 1) return `mod+${k.toLowerCase()}`
  return null
}

export function pressHint(root: HTMLElement | null, hint: string | null) {
  if (!root || !hint) return
  for (const el of root.querySelectorAll<HTMLElement>(`[data-hint="${hint}"]`)) el.dataset.pressed = ""
}

export function releaseHints(root: HTMLElement | null) {
  if (!root) return
  for (const el of root.querySelectorAll<HTMLElement>("[data-pressed]")) delete el.dataset.pressed
}
