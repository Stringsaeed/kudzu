// Browser-preview stand-in for the backend's activity log (Settings ▸ Activity).
// The real explanations come from kudzu-core `activity::explain`; this copy of the wording
// exists only so `pnpm dev` shows a realistic mix of outcomes.

import type { ActivityEntry, ActivityExplanation, ActivityOutcome, ActivitySource, Library } from "./types"

type App = NonNullable<ActivityEntry["app"]>

const app = (bundleId: string, name: string): App => ({ bundleId, name, path: `/Applications/${name}.app` })

const APPS = {
  mail: app("com.apple.mail", "Mail"),
  notes: app("com.apple.Notes", "Notes"),
  slack: app("com.tinyspeck.slackmacgap", "Slack"),
  safari: app("com.apple.Safari", "Safari"),
  code: app("com.microsoft.VSCode", "Visual Studio Code"),
  terminal: app("com.apple.Terminal", "Terminal"),
  onePassword: app("com.1password.1password", "1Password"),
  linear: app("com.linear", "Linear"),
  chrome: app("com.google.Chrome", "Google Chrome"),
}

function explain(outcome: ActivityOutcome, appName: string): ActivityExplanation {
  switch (outcome.kind) {
    case "expanded":
      return { label: "Expanded", tone: "ok" }
    case "undone":
      return { label: "Undone", tone: "info", sentence: "You pressed Backspace right after it expanded, so Kudzu put the trigger back." }
    case "paused":
      return { label: "Paused", tone: "skipped", sentence: "Kudzu was paused, so nothing expanded.", fix: { kind: "resume" } }
    case "snippetOff":
      return { label: "Snippet off", tone: "skipped", sentence: "This snippet is turned off.", fix: { kind: "turnOnSnippet" } }
    case "groupOff":
      return { label: "Group off", tone: "skipped", sentence: "The snippet's group is turned off, which turns off everything inside it.", fix: { kind: "openGroup", groupId: outcome.groupId } }
    case "appRule":
      if (outcome.level === "snippet") return { label: "App rule", tone: "skipped", sentence: `This snippet's app rule doesn't allow it in ${appName}.`, fix: { kind: "openSnippet" } }
      if (outcome.level === "group")
        return {
          label: "App rule",
          tone: "skipped",
          sentence: `The snippet inherits an app rule from its group, and that rule doesn't allow ${appName}.`,
          fix: outcome.groupId ? { kind: "openGroup", groupId: outcome.groupId } : { kind: "openSnippet" },
        }
      return {
        label: "App rule",
        tone: "skipped",
        sentence: outcome.level === "appSetting" ? `${appName} is on your list of apps where nothing expands.` : `${appName} is excluded by default, like other password managers and the login window.`,
        fix: { kind: "openAppSettings" },
      }
    case "secureInput":
      return {
        label: "Secure Input",
        tone: "blocked",
        sentence: `${outcome.owner || "A password field or app"} has Secure Input on, so macOS blocks every text expander until it is released.`,
        fix: { kind: "explainSecureInput" },
      }
    case "missingPermission":
      return {
        label: "Needs permission",
        tone: "blocked",
        sentence:
          outcome.permission === "accessibility"
            ? "Kudzu doesn't have Accessibility access, so macOS won't let it type the expansion."
            : "Kudzu doesn't have Input Monitoring access, so it can't see triggers being typed.",
        fix: { kind: "openPermissions", permission: outcome.permission },
      }
    case "formCancelled":
      return { label: "Form cancelled", tone: "info", sentence: "You closed the fill-in form, so nothing was inserted." }
    case "insertionFailed":
      return {
        label: "Failed",
        tone: "failed",
        sentence: "macOS refused Kudzu's keystrokes: Accessibility access was removed or Secure Input came on mid-insertion.",
        fix: { kind: "openPermissions", permission: "accessibility" },
      }
    case "interrupted":
      return { label: "Interrupted", tone: "info", sentence: "You kept typing, clicked or switched apps before Kudzu could finish, so it left your text alone." }
    case "tapDisabled":
      return {
        label: "Listener reset",
        tone: "info",
        sentence: "macOS switched off Kudzu's keyboard listener because it answered too slowly. Kudzu turned it back on; a trigger typed in that moment was missed.",
      }
    case "listenerLost":
      return { label: "Not listening", tone: "blocked", sentence: "Kudzu lost permission to watch the keyboard, so no trigger can expand.", fix: { kind: "openPermissions", permission: "accessibility" } }
    case "engineStopped":
      return { label: "Stopped", tone: "blocked", sentence: "The expansion engine was stopped, so no trigger can expand." }
  }
}

/** A session's worth of attempts against the mock library, newest first. */
export function seedActivity(library: Library): ActivityEntry[] {
  const group = (name: string) => library.groups.find((g) => g.name === name)?.id ?? ""
  const rows: [minutesAgo: number, trigger: string | null, source: ActivitySource | null, app: App | null, outcome: ActivityOutcome][] = [
    [1, ";sig", "typed", APPS.mail, { kind: "expanded" }],
    [3, ";lorem", "typed", APPS.notes, { kind: "snippetOff" }],
    [6, ";ph", "typed", APPS.onePassword, { kind: "appRule", level: "default" }],
    [9, ";cl", "typed", APPS.slack, { kind: "appRule", level: "snippet" }],
    [14, ";bug", "typed", APPS.terminal, { kind: "secureInput", owner: "Terminal" }],
    [21, ";fu", "form", APPS.mail, { kind: "formCancelled" }],
    [25, ";ty", "typed", APPS.safari, { kind: "undone" }],
    [26, ";ty", "typed", APPS.safari, { kind: "expanded" }],
    [34, ";cal", "typed", APPS.slack, { kind: "appRule", level: "group", groupId: group("Work") }],
    [41, ";addr", "quickSearch", APPS.notes, { kind: "expanded" }],
    [48, ";date", "typed", APPS.code, { kind: "paused" }],
    [55, null, null, null, { kind: "tapDisabled", cause: "timeout", reenabled: true }],
    [63, ";now", "typed", APPS.chrome, { kind: "insertionFailed", error: "notAllowed" }],
    [70, ";shrug", "typed", APPS.chrome, { kind: "interrupted" }],
    [82, ";ooo", "typed", APPS.linear, { kind: "appRule", level: "appSetting" }],
    [95, ";coa", "typed", APPS.code, { kind: "groupOff", groupId: group("Developer") }],
    [120, ";quote", "typed", APPS.mail, { kind: "missingPermission", permission: "accessibility" }],
  ]
  return rows.map(([minutesAgo, trigger, source, target, outcome], i) => {
    const snippet = trigger ? library.snippets.find((s) => s.triggers.includes(trigger)) : undefined
    return {
      id: rows.length - i,
      at: new Date(Date.now() - minutesAgo * 60_000).toISOString(),
      source,
      snippetId: snippet?.id ?? null,
      // Quick Search inserts have no trigger: nothing was typed.
      trigger: source === "quickSearch" ? null : trigger,
      app: target,
      outcome,
      explanation: explain(outcome, target?.name ?? "this app"),
    }
  })
}
