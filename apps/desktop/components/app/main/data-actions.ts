"use client"

import { toast } from "sonner"

import { api, isTauri } from "@/lib/backend"
import type { ImportSummary } from "@/lib/types"

function reportImport(r: ImportSummary) {
  const what = `${r.importedSnippets} snippet${r.importedSnippets === 1 ? "" : "s"}${r.importedGroups ? ` in ${r.importedGroups} group${r.importedGroups === 1 ? "" : "s"}` : ""}`
  toast.success(`Imported ${what}`, {
    description: r.warnings.length ? `${r.warnings.length} item${r.warnings.length === 1 ? "" : "s"} imported with changes — e.g. ${r.warnings[0].message}` : "A backup was saved first, just in case.",
  })
}

type ImportKind = "file" | "raycast" | "textexpander" | "espanso-folder"

const IMPORT_DIALOGS: Record<Exclude<ImportKind, "espanso-folder">, { title: string; filters: { name: string; extensions: string[] }[] }> = {
  file: { title: "Import snippets", filters: [{ name: "Snippet files", extensions: ["json", "yml", "yaml", "textexpander", "csv"] }] },
  raycast: { title: "Choose your Raycast snippets export", filters: [{ name: "Raycast snippets", extensions: ["json"] }] },
  textexpander: { title: "Choose your TextExpander export", filters: [{ name: "TextExpander", extensions: ["textexpander", "csv"] }] },
}

/** Short export instructions per source, shared by Settings and onboarding. */
export const IMPORT_HINTS = {
  raycast: "In Raycast, run Export Snippets, then choose the .json file.",
  textexpander: "In TextExpander, export a group as .textexpander or CSV.",
} as const

/** Shows the folder dialog for an Espanso config folder; null if cancelled. */
export async function pickEspansoFolder(): Promise<string | null> {
  const { open } = await import("@tauri-apps/plugin-dialog")
  const dir = await open({ directory: true, title: "Choose your Espanso config folder" })
  return typeof dir === "string" ? dir : null
}

/** Shows the file dialog for `kind` and returns the chosen path, or null if cancelled. */
export async function pickImportFile(kind: Exclude<ImportKind, "espanso-folder">): Promise<string | null> {
  const { open } = await import("@tauri-apps/plugin-dialog")
  const file = await open(IMPORT_DIALOGS[kind])
  return typeof file === "string" ? file : null
}

/**
 * Asks for a snippet file and imports it. The desktop app works out whether it came from
 * Kudzu, Raycast (.json), Espanso (.yml) or TextExpander (.textexpander, .csv).
 */
export async function runImport(kind: ImportKind = "file") {
  if (!isTauri()) {
    toast("Import works in the desktop app", { description: "This preview runs in a browser without file access." })
    return
  }
  try {
    if (kind === "espanso-folder") {
      const dir = await pickEspansoFolder()
      if (dir) reportImport(await api.importEspanso(dir))
      return
    }
    const file = await pickImportFile(kind)
    if (file) reportImport(await api.importFile(file))
  } catch (e) {
    toast.error("Import failed", { description: String(e) })
  }
}

export async function runExport(kind: "json" | "espanso") {
  if (!isTauri()) {
    toast("Export works in the desktop app")
    return
  }
  const dialog = await import("@tauri-apps/plugin-dialog")
  try {
    if (kind === "json") {
      const stamp = new Date().toISOString().slice(0, 10)
      const path = await dialog.save({ title: "Export library", defaultPath: `kudzu-${stamp}.json`, filters: [{ name: "Kudzu library", extensions: ["json"] }] })
      if (!path) return
      await api.exportJson(path)
      toast.success("Library exported", { description: path })
    } else {
      const dir = await dialog.open({ directory: true, title: "Choose a folder for the Espanso match files" })
      if (typeof dir !== "string") return
      const n = await api.exportEspanso(dir)
      toast.success(`Exported ${n} Espanso file${n === 1 ? "" : "s"}`, { description: `${dir} · Formatting and images export as plain text. Use Kudzu JSON to keep them.` })
    }
  } catch (e) {
    toast.error("Export failed", { description: String(e) })
  }
}
