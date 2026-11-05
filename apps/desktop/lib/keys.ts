// Keyboard helpers: one shortcut grammar ("mod+shift+k") shared by handlers and hint labels.

export const isMac = typeof navigator !== "undefined" ? /Mac|iPhone|iPad/.test(navigator.platform || navigator.userAgent) : true

const SYMBOLS: Record<string, string> = isMac
  ? { mod: "⌘", cmd: "⌘", ctrl: "⌃", control: "⌃", alt: "⌥", option: "⌥", shift: "⇧", enter: "↵", backspace: "⌫", delete: "⌫", escape: "esc", up: "↑", down: "↓", left: "←", right: "→", tab: "⇥", space: "Space" }
  : { mod: "Ctrl", cmd: "Ctrl", ctrl: "Ctrl", control: "Ctrl", alt: "Alt", option: "Alt", shift: "Shift", enter: "Enter", backspace: "Backspace", delete: "Del", escape: "Esc", up: "↑", down: "↓", left: "←", right: "→", tab: "Tab", space: "Space" }

/** Splits a shortcut into display keys: "mod+shift+k" → ["⌘", "⇧", "K"]. */
export function keyLabels(combo: string): string[] {
  return combo.split("+").map((k) => SYMBOLS[k.toLowerCase()] ?? (k.length === 1 ? k.toUpperCase() : k))
}

/** Whether a keyboard event matches a combo such as "mod+k", "shift+?", "alt+up". */
export function matches(e: KeyboardEvent | React.KeyboardEvent, combo: string): boolean {
  const parts = combo.toLowerCase().split("+")
  const key = parts[parts.length - 1]
  const want = new Set(parts.slice(0, -1))
  const mod = isMac ? e.metaKey : e.ctrlKey
  if (want.has("mod") !== mod) return false
  if (!isMac && want.has("mod") && e.metaKey) return false
  if (isMac && !want.has("ctrl") && e.ctrlKey) return false
  if (want.has("ctrl") && !e.ctrlKey) return false
  if (want.has("alt") !== e.altKey) return false
  // Shift is implied for symbols like "?"; only enforce it when asked for, or for letters.
  if (want.has("shift") && !e.shiftKey) return false
  if (!want.has("shift") && e.shiftKey && /^[a-z0-9]$|^(enter|backspace|delete|up|down|tab)$/.test(key)) return false
  const k = e.key.toLowerCase()
  const named: Record<string, string> = {
    up: "arrowup",
    down: "arrowdown",
    left: "arrowleft",
    right: "arrowright",
    esc: "escape",
    space: " ",
    del: "delete",
  }
  if (named[key]) return k === named[key]
  if (key.length === 1 && /[a-z0-9]/.test(key)) return k === key || e.code.toLowerCase() === (/[0-9]/.test(key) ? `digit${key}` : `key${key}`)
  return k === key
}

/** True when the event target is a text-editing surface (typing shouldn't trigger shortcuts). */
export function isEditable(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false
  if (target.isContentEditable) return true
  const tag = target.tagName
  if (tag === "TEXTAREA") return true
  if (tag === "INPUT") {
    const type = (target as HTMLInputElement).type
    return !["checkbox", "radio", "button", "submit", "range"].includes(type)
  }
  return !!target.closest(".cm-editor")
}

/** Accelerator string ("Control+Alt+Space") → display keys. */
export function acceleratorLabels(accel: string): string[] {
  return accel.split("+").map((p) => {
    const k = p.toLowerCase()
    if (k === "commandorcontrol" || k === "cmdorctrl") return isMac ? "⌘" : "Ctrl"
    if (k === "super" || k === "command" || k === "cmd" || k === "meta") return isMac ? "⌘" : "Win"
    return SYMBOLS[k] ?? (p.length === 1 ? p.toUpperCase() : p)
  })
}

/** Builds a Tauri accelerator from a keydown, or null if it's only modifiers / has none. */
export function acceleratorFromEvent(e: KeyboardEvent): string | null {
  const mods: string[] = []
  if (e.ctrlKey) mods.push("Control")
  if (e.altKey) mods.push("Alt")
  if (e.shiftKey) mods.push("Shift")
  if (e.metaKey) mods.push(isMac ? "Command" : "Super")
  const code = e.code
  let key: string | null = null
  if (/^Key[A-Z]$/.test(code)) key = code.slice(3)
  else if (/^Digit\d$/.test(code)) key = code.slice(5)
  else if (code === "Space") key = "Space"
  else if (/^F\d{1,2}$/.test(code)) key = code
  else if (["Semicolon", "Quote", "Comma", "Period", "Slash", "Backslash", "BracketLeft", "BracketRight", "Backquote", "Minus", "Equal", "Enter", "Tab"].includes(code)) key = code
  if (!key) return null
  const fkey = /^F\d{1,2}$/.test(key)
  if (!mods.length && !fkey) return null
  if (mods.length === 1 && mods[0] === "Shift" && !fkey) return null
  return [...mods, key].join("+")
}
