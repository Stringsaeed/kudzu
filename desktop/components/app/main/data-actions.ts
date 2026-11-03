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

/** Asks for a Kudzu JSON file, an Espanso YAML file, or an Espanso folder, then imports it. */
export async function runImport(kind?: "espanso-folder" | "file") {
  if (!isTauri()) {
    toast("Import works in the desktop app", { description: "This preview runs in a browser without file access." })
    return
  }
  const { open } = await import("@tauri-apps/plugin-dialog")
  try {
    if (kind === "espanso-folder") {
      const dir = await open({ directory: true, title: "Choose your Espanso config folder" })
      if (typeof dir === "string") reportImport(await api.importEspanso(dir))
      return
    }
    const file = await open({
      title: "Import snippets",
      filters: [{ name: "Kudzu or Espanso", extensions: ["json", "yml", "yaml"] }],
    })
    if (typeof file !== "string") return
    reportImport(file.endsWith(".json") ? await api.importJson(file) : await api.importEspanso(file))
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
      toast.success(`Exported ${n} Espanso file${n === 1 ? "" : "s"}`, { description: dir })
    }
  } catch (e) {
    toast.error("Export failed", { description: String(e) })
  }
}
