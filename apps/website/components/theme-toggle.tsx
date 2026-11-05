"use client";

import { useSyncExternalStore } from "react";
import { isTheme, themeStorageKey, type Theme } from "@/lib/theme";

function currentTheme(): Theme {
  const theme = document.documentElement.dataset.theme;
  return isTheme(theme) ? theme : "system";
}

function subscribe(onChange: () => void) {
  function onStorage(event: StorageEvent) {
    if (event.key !== themeStorageKey && event.key !== null) return;
    document.documentElement.dataset.theme = isTheme(event.newValue) ? event.newValue : "system";
    onChange();
  }
  window.addEventListener("kudzu-theme-change", onChange);
  window.addEventListener("storage", onStorage);
  return () => {
    window.removeEventListener("kudzu-theme-change", onChange);
    window.removeEventListener("storage", onStorage);
  };
}

export function ThemeToggle() {
  const theme = useSyncExternalStore(subscribe, currentTheme, () => "system");
  return (
    <label className="theme-control" title={`Color theme: ${theme}`}>
      <svg className="icon" viewBox="0 0 24 24" aria-hidden="true">
        {theme === "dark" ? (
          <path d="M20.5 13a8.5 8.5 0 0 1-9.5-9.5A8.5 8.5 0 1 0 20.5 13Z" />
        ) : theme === "light" ? (
          <>
            <circle cx="12" cy="12" r="4" />
            <path d="M12 2v2m0 16v2M2 12h2m16 0h2M5 5l1.5 1.5m11 11L19 19M5 19l1.5-1.5m11-11L19 5" />
          </>
        ) : (
          <>
            <rect x="3" y="4" width="18" height="13" rx="2" />
            <path d="M8 21h8m-4-4v4" />
          </>
        )}
      </svg>
      <span className="visually-hidden">Color theme</span>
      <select
        aria-label="Color theme"
        value={theme}
        onChange={(event) => {
          const next = event.target.value;
          if (!isTheme(next)) return;
          document.documentElement.dataset.theme = next;
          try {
            localStorage.setItem(themeStorageKey, next);
          } catch {
            /* The choice still applies when storage is unavailable. */
          }
          window.dispatchEvent(new Event("kudzu-theme-change"));
        }}
      >
        <option value="system">System</option>
        <option value="light">Light</option>
        <option value="dark">Dark</option>
      </select>
    </label>
  );
}
