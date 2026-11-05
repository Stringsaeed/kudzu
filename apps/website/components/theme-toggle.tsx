"use client";

import { useEffect, useRef, useState, useSyncExternalStore, type KeyboardEvent } from "react";
import { isTheme, themeStorageKey, type Theme } from "@/lib/theme";
import { Icon } from "./icons";

const options: { value: Theme; label: string; icon: string }[] = [
  { value: "system", label: "System", icon: "monitor" },
  { value: "light", label: "Light", icon: "sun" },
  { value: "dark", label: "Dark", icon: "moon" },
];

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
  const theme = useSyncExternalStore<Theme>(subscribe, currentTheme, () => "system");
  const [open, setOpen] = useState(false);
  const root = useRef<HTMLDivElement>(null);
  const button = useRef<HTMLButtonElement>(null);
  const items = useRef<(HTMLButtonElement | null)[]>([]);
  const active = options.find((option) => option.value === theme) ?? options[0];
  const activeIndex = options.indexOf(active);

  useEffect(() => {
    if (!open) return;
    items.current[activeIndex]?.focus();
    const closeOutside = (event: Event) => {
      if (event.target instanceof Node && !root.current?.contains(event.target)) setOpen(false);
    };
    const closeOnEscape = (event: globalThis.KeyboardEvent) => {
      if (event.key !== "Escape") return;
      setOpen(false);
      button.current?.focus();
    };
    document.addEventListener("pointerdown", closeOutside);
    document.addEventListener("focusin", closeOutside);
    document.addEventListener("keydown", closeOnEscape);
    return () => {
      document.removeEventListener("pointerdown", closeOutside);
      document.removeEventListener("focusin", closeOutside);
      document.removeEventListener("keydown", closeOnEscape);
    };
  }, [open, activeIndex]);

  function choose(next: Theme) {
    document.documentElement.dataset.theme = next;
    try {
      localStorage.setItem(themeStorageKey, next);
    } catch {
      /* The choice still applies when storage is unavailable. */
    }
    window.dispatchEvent(new Event("kudzu-theme-change"));
    setOpen(false);
    button.current?.focus();
  }

  function navigate(event: KeyboardEvent<HTMLDivElement>) {
    if (!["ArrowDown", "ArrowUp", "Home", "End"].includes(event.key)) return;
    event.preventDefault();
    const current = items.current.findIndex((item) => item === document.activeElement);
    const next =
      event.key === "Home"
        ? 0
        : event.key === "End"
          ? options.length - 1
          : (current + (event.key === "ArrowDown" ? 1 : -1) + options.length) % options.length;
    items.current[next]?.focus();
  }

  return (
    <div className="theme-control" ref={root}>
      <button
        ref={button}
        type="button"
        className="theme-button"
        aria-haspopup="menu"
        aria-expanded={open}
        aria-label={`Color theme: ${active.label}`}
        onClick={() => setOpen((value) => !value)}
      >
        <Icon name={active.icon} />
      </button>
      {open && (
        <div
          className="theme-menu"
          role="menu"
          aria-label="Color theme"
          tabIndex={-1}
          onKeyDown={navigate}
        >
          {options.map((option, index) => (
            <button
              key={option.value}
              ref={(node) => {
                items.current[index] = node;
              }}
              type="button"
              role="menuitemradio"
              aria-checked={option.value === theme}
              tabIndex={option.value === theme ? 0 : -1}
              onClick={() => choose(option.value)}
            >
              <Icon name={option.icon} />
              {option.label}
              {option.value === theme && <Icon name="check" className="theme-check" />}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
