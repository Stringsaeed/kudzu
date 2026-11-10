"use client"

import { useState } from "react"

import { Button } from "@/components/ui/button"
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogTitle } from "@/components/ui/dialog"
import { Input } from "@/components/ui/input"
import { Switch } from "@/components/ui/switch"
import { useStore } from "@/lib/store"
import type { AppRuleMode, Group, InsertionMethod } from "@/lib/types"

import { AppPicker } from "../app-picker"
import { Segmented } from "../segmented"

/** Name, on/off, insertion method and app rule for a group; snippets inside inherit them. */
export function GroupSettingsDialog({ group, onOpenChange }: { group: Group | null; onOpenChange: (open: boolean) => void }) {
  return (
    <Dialog open={!!group} onOpenChange={onOpenChange}>
      {group && <GroupSettingsForm key={group.id} group={group} onOpenChange={onOpenChange} />}
    </Dialog>
  )
}

function GroupSettingsForm({ group, onOpenChange }: { group: Group; onOpenChange: (open: boolean) => void }) {
  const saveGroup = useStore((s) => s.saveGroup)
  const [draft, setDraft] = useState<Group>(group)
  const ruleChoice: "none" | AppRuleMode = draft.rule?.mode ?? "none"

  return (
    <>
      <DialogContent className="max-w-[520px] gap-5 rounded-2xl shadow-float ring-0 sm:max-w-[520px]">
        <div className="flex flex-col gap-1">
          <DialogTitle>Group settings</DialogTitle>
          <DialogDescription>Snippets in this group (and its subgroups) inherit these unless they set their own.</DialogDescription>
        </div>
        <form
          id="group-settings"
          className="flex flex-col gap-5"
          onSubmit={(e) => {
            e.preventDefault()
            void saveGroup({ ...draft, name: draft.name.trim() || group.name })
            onOpenChange(false)
          }}
        >
          <div className="flex items-center gap-3">
            <Input aria-label="Group name" value={draft.name} onChange={(e) => setDraft({ ...draft, name: e.target.value })} className="h-9 flex-1" autoFocus />
            <label className="flex items-center gap-2 text-xs text-muted-foreground">
              <Switch checked={draft.isEnabled} onCheckedChange={(isEnabled) => setDraft({ ...draft, isEnabled })} />
              {draft.isEnabled ? "On" : "Off"}
            </label>
          </div>
          <div className="flex items-center justify-between gap-4">
            <div>
              <div className="text-[13px] font-medium">Insert by</div>
              <div className="text-xs text-muted-foreground">How text is delivered for snippets in this group.</div>
            </div>
            <Segmented
              label="Insertion method"
              value={(draft.insertion ?? "default") as InsertionMethod | "default"}
              onChange={(v) => setDraft({ ...draft, insertion: v === "default" ? undefined : (v as InsertionMethod) })}
              options={[
                { value: "default", label: "Default" },
                { value: "type", label: "Type" },
                { value: "paste", label: "Paste" },
              ]}
            />
          </div>
          <div className="flex flex-col gap-3">
            <div className="flex items-center justify-between gap-4">
              <div>
                <div className="text-[13px] font-medium">Apps</div>
                <div className="text-xs text-muted-foreground">Where snippets in this group can expand.</div>
              </div>
              <Segmented
                label="App rule"
                value={ruleChoice}
                onChange={(v) => setDraft({ ...draft, rule: v === "none" ? undefined : { mode: v, bundleIDs: draft.rule?.bundleIDs ?? [] } })}
                options={[
                  { value: "none", label: "All apps" },
                  { value: "allow", label: "Only in…" },
                  { value: "deny", label: "Except…" },
                ]}
              />
            </div>
            {draft.rule && (
              // Rises in when a rule is chosen. A group that already has one shows it from the start.
              <div className={group.rule ? undefined : "animate-rise"}>
                <AppPicker value={draft.rule.bundleIDs} onChange={(bundleIDs) => setDraft({ ...draft, rule: { ...draft.rule!, bundleIDs } })} />
              </div>
            )}
          </div>
        </form>
        <DialogFooter>
          <Button variant="ghost" onClick={() => onOpenChange(false)}>
            Cancel
          </Button>
          <Button type="submit" form="group-settings">
            Save
          </Button>
        </DialogFooter>
      </DialogContent>
    </>
  )
}
