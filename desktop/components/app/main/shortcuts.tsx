"use client"

import { Dialog, DialogContent, DialogDescription, DialogTitle } from "@/components/ui/dialog"
import { acceleratorLabels } from "@/lib/keys"
import { useStore } from "@/lib/store"

import { Shortcut } from "../shortcut"

const SECTIONS: { title: string; items: [string, string][] }[] = [
  {
    title: "Everywhere",
    items: [
      ["mod+k", "Search & commands"],
      ["mod+n", "New snippet"],
      ["mod+shift+n", "New group"],
      ["mod+f", "Filter snippets"],
      ["mod+shift+p", "Pause / resume expanding"],
      ["mod+,", "Settings"],
      ["mod+/", "This cheat sheet"],
    ],
  },
  {
    title: "Navigate",
    items: [
      ["mod+1", "All snippets"],
      ["mod+2", "Favorites"],
      ["mod+3", "Recently used"],
      ["mod+4", "Insights"],
      ["mod+l", "Focus the list"],
      ["mod+e", "Edit content"],
      ["escape", "Back to the list"],
    ],
  },
  {
    title: "Snippet list",
    items: [
      ["up", "Previous (or K)"],
      ["down", "Next (or J)"],
      ["shift+down", "Extend selection"],
      ["mod+a", "Select all"],
      ["enter", "Open in editor"],
      ["/", "Filter"],
      ["mod+backspace", "Delete (undoable)"],
    ],
  },
  {
    title: "Editor",
    items: [
      ["/", "Insert a variable"],
      ["mod+d", "Duplicate snippet"],
      ["mod+shift+f", "Toggle favorite"],
      ["mod+shift+e", "Turn on / off"],
      ["mod+z", "Undo"],
      ["mod+s", "Save now (auto-saves anyway)"],
    ],
  },
]

export function ShortcutsDialog() {
  const open = useStore((s) => s.shortcutsOpen)
  const setOpen = useStore((s) => s.setShortcutsOpen)
  const hotkey = useStore((s) => s.settings.paletteHotkey)
  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogContent className="max-w-[720px] gap-0 rounded-2xl p-0 shadow-float ring-0 sm:max-w-[720px]">
        <div className="flex items-center gap-3 border-b border-hairline px-6 py-4">
          <DialogTitle className="text-[15px] font-semibold">Keyboard shortcuts</DialogTitle>
          <DialogDescription className="sr-only">Every shortcut in Kudzu.</DialogDescription>
          <span className="ml-auto flex items-center gap-2 text-xs text-muted-foreground">
            Quick search from any app
            <Shortcut keys={acceleratorLabels(hotkey)} tone="accent" />
          </span>
        </div>
        <div className="grid grid-cols-2 gap-x-10 gap-y-6 px-6 py-5">
          {SECTIONS.map((s) => (
            <section key={s.title} aria-label={s.title}>
              <h3 className="mb-2 text-[11px] font-medium text-faint">{s.title}</h3>
              <dl className="flex flex-col">
                {s.items.map(([combo, label]) => (
                  <div key={combo + label} className="flex h-7 items-center justify-between gap-4 text-[13px]">
                    <dt className="text-foreground/85">{label}</dt>
                    <dd>
                      <Shortcut combo={combo} />
                    </dd>
                  </div>
                ))}
              </dl>
            </section>
          ))}
        </div>
      </DialogContent>
    </Dialog>
  )
}
