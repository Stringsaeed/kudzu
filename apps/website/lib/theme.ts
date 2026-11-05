export type Theme = "system" | "light" | "dark";

export const themeStorageKey = "kudzu-site-theme";

export function isTheme(value: unknown): value is Theme {
  return value === "system" || value === "light" || value === "dark";
}

// Runs before the page paints; the CSS handles live system-theme changes.
export const themeInitialization = `(() => {
  let theme = 'system';
  try {
    const saved = localStorage.getItem('${themeStorageKey}');
    if (saved === 'light' || saved === 'dark') theme = saved;
  } catch {}
  document.documentElement.dataset.theme = theme;
})();`;
