"use client"

import { Settings2 } from "lucide-react"

import { appName, useInstalledApps } from "@/lib/apps"
import type { AppRule, AppRuleMode, Group, InsertionMethod, Snippet } from "@/lib/types"
import { cn } from "@/lib/utils"

import { AppPicker } from "../app-picker"
import { Segmented } from "../segmented"
import { useBump } from "./motion"

/** Nearest ancestor group setting `key`, with the group name. */
export function inherited<K extends "rule" | "insertion">(groups: Group[], groupId: string | undefined, key: K): { value: Group[K]; from: Group } | null {
  const byId = new Map(groups.map((g) => [g.id, g]))
  const seen = new Set<string>()
  let g = groupId ? byId.get(groupId) : undefined
  while (g && !seen.has(g.id)) {
    seen.add(g.id)
    if (g[key] != null) return { value: g[key], from: g }
    g = g.parentID ? byId.get(g.parentID) : undefined
  }
  return null
}

const INSERTION_LABEL: Record<InsertionMethod, string> = { automatic: "Automatic", type: "Typing", paste: "Pasting" }

/** `swap` names what the description depends on; when it changes the new text rises in, not on mount. */
function Row({ title, description, swap, children }: { title: string; description: React.ReactNode; swap: unknown; children: React.ReactNode }) {
  const n = useBump(swap)
  return (
    <div className="grid grid-cols-[1fr_auto] items-center gap-x-6 gap-y-1 py-3">
      <div className="min-w-0">
        <div className="text-[13px] font-medium">{title}</div>
        <div key={n} className={cn("text-xs text-muted-foreground", n > 0 && "animate-rise")}>
          {description}
        </div>
      </div>
      <div className="justify-self-end">{children}</div>
    </div>
  )
}

type RuleChoice = "inherit" | AppRuleMode

export function BehaviorSection({ snippet, groups, onChange }: { snippet: Snippet; groups: Group[]; onChange: (patch: Partial<Snippet>) => void }) {
  const apps = useInstalledApps()
  const inheritedInsertion = inherited(groups, snippet.groupID, "insertion")
  const inheritedRule = inherited(groups, snippet.groupID, "rule")
  const ruleChoice: RuleChoice = snippet.rule?.mode ?? "inherit"

  const describeRule = (rule: AppRule | undefined | null) => {
    if (!rule) return "Works in every app"
    if (!rule.bundleIDs.length) return rule.mode === "allow" ? "Disabled everywhere" : "Works in every app"
    const names = rule.bundleIDs.slice(0, 3).map((b) => appName(apps, b)).join(", ") + (rule.bundleIDs.length > 3 ? ` +${rule.bundleIDs.length - 3}` : "")
    return rule.mode === "allow" ? `Only in ${names}` : `Everywhere except ${names}`
  }

  return (
    <section aria-labelledby="behavior-title" className="rounded-xl bg-surface px-4 shadow-card">
      <h3 id="behavior-title" className="sr-only">
        Behavior
      </h3>
      <div className="divide-y divide-hairline">
        <Row
          title="Expand"
          swap={snippet.fireMode}
          description={snippet.fireMode === "immediate" ? "As soon as the last trigger character is typed." : "After you finish the word — space, return or punctuation."}
        >
          <Segmented
            label="When to expand"
            value={snippet.fireMode}
            onChange={(fireMode) => onChange({ fireMode })}
            options={[
              { value: "immediate", label: "Instantly" },
              { value: "wholeWord", label: "After the word" },
            ]}
          />
        </Row>
        <Row
          title="Capitalization"
          swap={snippet.caseMode}
          description={
            snippet.caseMode === "propagate" ? (
              <>
                Follows how you type the trigger: <span className="font-mono">;sig</span> → text, <span className="font-mono">;SIG</span> → TEXT.
              </>
            ) : (
              "The trigger must match exactly; text is never changed."
            )
          }
        >
          <Segmented
            label="Capitalization"
            value={snippet.caseMode}
            onChange={(caseMode) => onChange({ caseMode })}
            options={[
              { value: "propagate", label: "Adapt" },
              { value: "exact", label: "Exact" },
            ]}
          />
        </Row>
        <Row
          title="Insert by"
          swap={snippet.insertion}
          description={
            snippet.formatting?.source === snippet.content
              ? (snippet.insertion ?? inheritedInsertion?.value) === "type"
                ? "Types plain text. Images use their text alternatives."
                : "Pastes formatting and images. Apps that need plain text use the text alternative."
              : snippet.insertion
              ? snippet.insertion === "type"
                ? "Types the text key by key. Most compatible."
                : snippet.insertion === "paste"
                  ? "Pastes through the clipboard, then restores it. Best for long text."
                  : "Types short text and pastes long text."
              : inheritedInsertion
                ? `Inherited from “${inheritedInsertion.from.name}”: ${INSERTION_LABEL[inheritedInsertion.value!]}`
                : "Default — typing for short text, pasting for long or in terminals."
          }
        >
          <Segmented
            label="Insertion method"
            value={(snippet.insertion ?? "default") as InsertionMethod | "default"}
            onChange={(v) => onChange({ insertion: v === "default" ? undefined : (v as InsertionMethod) })}
            options={[
              { value: "default", label: "Default" },
              { value: "type", label: "Type" },
              { value: "paste", label: "Paste" },
            ]}
          />
        </Row>
        <div className="py-3">
          <Row
            title="Apps"
            swap={ruleChoice}
            description={
              ruleChoice === "inherit"
                ? inheritedRule
                  ? `Inherited from “${inheritedRule.from.name}”: ${describeRule(inheritedRule.value)}`
                  : "Works in every app (except excluded apps in Settings)."
                : describeRule(snippet.rule)
            }
          >
            <Segmented
              label="App rule"
              value={ruleChoice}
              onChange={(v) =>
                onChange({
                  rule: v === "inherit" ? undefined : { mode: v, bundleIDs: snippet.rule?.bundleIDs ?? [], insertion: snippet.rule?.insertion },
                })
              }
              options={[
                { value: "inherit", label: inheritedRule ? "Inherit" : "All apps" },
                { value: "allow", label: "Only in…" },
                { value: "deny", label: "Except…" },
              ]}
            />
          </Row>
          {snippet.rule && (
            <div className="flex flex-col gap-3 pb-1 animate-rise">
              <AppPicker
                value={snippet.rule.bundleIDs}
                onChange={(bundleIDs) => onChange({ rule: { ...snippet.rule!, bundleIDs } })}
                addLabel={snippet.rule.mode === "allow" ? "Allow an app" : "Block an app"}
              />
              {snippet.rule.mode === "allow" && snippet.rule.bundleIDs.length > 0 && (
                <div className="flex items-center gap-2 text-xs text-muted-foreground animate-rise">
                  <Settings2 className="size-3.5" />
                  In these apps, insert by
                  <Segmented
                    size="sm"
                    label="Insertion in these apps"
                    value={(snippet.rule.insertion ?? "default") as InsertionMethod | "default"}
                    onChange={(v) => onChange({ rule: { ...snippet.rule!, insertion: v === "default" ? undefined : (v as InsertionMethod) } })}
                    options={[
                      { value: "default", label: "Default" },
                      { value: "type", label: "Type" },
                      { value: "paste", label: "Paste" },
                    ]}
                  />
                </div>
              )}
            </div>
          )}
        </div>
      </div>
    </section>
  )
}
