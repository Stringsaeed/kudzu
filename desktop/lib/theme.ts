import type { ThemePreference } from "./types"

let preference: ThemePreference = "dark"
let media: MediaQueryList | null = null

function paint() {
  const dark = preference === "dark" || (preference === "system" && !!media?.matches)
  const root = document.documentElement
  if (root.classList.contains("dark") === dark && root.classList.contains("light") === !dark) return
  // Flip instantly: without this, every colour transition animates and text washes out mid-switch.
  root.classList.add("theme-switching")
  root.classList.toggle("dark", dark)
  root.classList.toggle("light", !dark)
  root.style.colorScheme = dark ? "dark" : "light"
  void root.offsetHeight
  requestAnimationFrame(() => requestAnimationFrame(() => root.classList.remove("theme-switching")))
}

/**
 * Applies a theme preference to this window and remembers it for the pre-paint script.
 * "system" keeps following the OS appearance as it changes (e.g. auto dark mode at sunset).
 */
export function applyTheme(pref: ThemePreference) {
  preference = pref
  try {
    localStorage.setItem("kudzu-theme", pref)
  } catch {}
  if (!media && typeof matchMedia !== "undefined") {
    media = matchMedia("(prefers-color-scheme: dark)")
    media.addEventListener("change", () => {
      if (preference === "system") paint()
    })
  }
  paint()
}
