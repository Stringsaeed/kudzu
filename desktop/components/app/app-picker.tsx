"use client"

import { Check, Plus, X } from "lucide-react"
import { useState } from "react"

import { Button } from "@/components/ui/button"
import { Command, CommandEmpty, CommandGroup, CommandInput, CommandItem, CommandList } from "@/components/ui/command"
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover"
import { appName, useInstalledApps } from "@/lib/apps"
import { cn } from "@/lib/utils"

import { AppIcon } from "./app-icon"

/** Shows chosen apps as removable chips plus an "Add app" searchable picker. */
export function AppPicker({
  value,
  onChange,
  addLabel = "Add app",
  className,
}: {
  value: string[]
  onChange: (bundleIds: string[]) => void
  addLabel?: string
  className?: string
}) {
  const apps = useInstalledApps()
  const [open, setOpen] = useState(false)
  const [query, setQuery] = useState("")
  const chosen = new Set(value)
  const toggle = (id: string) => onChange(chosen.has(id) ? value.filter((v) => v !== id) : [...value, id])

  return (
    <div className={cn("flex flex-wrap items-center gap-1.5", className)}>
      {value.map((id) => {
        const app = apps.find((a) => a.bundleId === id)
        return (
          <span key={id} className="inline-flex h-7 items-center gap-1.5 rounded-md bg-surface-raised pr-1 pl-1.5 text-xs shadow-raised animate-pop">
            <AppIcon path={app?.path} name={appName(apps, id)} size={16} />
            <span className="max-w-40 truncate">{appName(apps, id)}</span>
            <button className="flex size-5 items-center justify-center rounded text-muted-foreground hover:bg-foreground/10 hover:text-foreground" onClick={() => toggle(id)} aria-label={`Remove ${appName(apps, id)}`}>
              <X className="size-3" />
            </button>
          </span>
        )
      })}
      <Popover open={open} onOpenChange={setOpen}>
        <PopoverTrigger asChild>
          <Button variant="ghost" size="sm" className="h-7 text-muted-foreground ring-1 ring-dashed ring-border">
            <Plus /> {addLabel}
          </Button>
        </PopoverTrigger>
        <PopoverContent align="start" className="w-72 p-0 shadow-float ring-0">
          <Command loop className="bg-transparent">
            <CommandInput placeholder="Search apps or type a bundle ID…" value={query} onValueChange={setQuery} />
            <CommandList className="max-h-72">
              <CommandEmpty>
                {/^[\w-]+(\.[\w-]+)+$/.test(query) ? (
                  <button className="text-foreground underline-offset-2 hover:underline" onClick={() => (toggle(query), setQuery(""))}>
                    Add “{query}”
                  </button>
                ) : (
                  "No apps found."
                )}
              </CommandEmpty>
              <CommandGroup>
                {apps.map((a) => (
                  <CommandItem key={a.bundleId} value={`${a.name} ${a.bundleId}`} onSelect={() => toggle(a.bundleId)} className="gap-2">
                    <AppIcon path={a.path} name={a.name} size={18} />
                    <span className="truncate">{a.name}</span>
                    {chosen.has(a.bundleId) && <Check className="ml-auto size-3.5 text-iris" />}
                  </CommandItem>
                ))}
              </CommandGroup>
            </CommandList>
          </Command>
        </PopoverContent>
      </Popover>
    </div>
  )
}
