"use client"

import { Plus, X } from "lucide-react"
import { forwardRef, useImperativeHandle, useRef, useState } from "react"

import { cn } from "@/lib/utils"

export const MAX_TRIGGER_LENGTH = 64

const graphemes = (s: string) => {
  try {
    return [...new Intl.Segmenter().segment(s)].length
  } catch {
    return [...s].length
  }
}

/** Validates a typed trigger; returns the cleaned value or a message. */
export function validateTrigger(raw: string, existing: string[], caseSensitive: boolean): { ok: string } | { error: string } {
  if (!raw) return { error: "Enter a trigger first." }
  const t = raw.trim()
  if (!t) return { error: "A trigger can't be only spaces." }
  const len = graphemes(t)
  if (len > MAX_TRIGGER_LENGTH) return { error: `Triggers can be at most ${MAX_TRIGGER_LENGTH} characters; this one has ${len}.` }
  if (existing.some((e) => (caseSensitive ? e === t : e.toLowerCase() === t.toLowerCase()))) return { error: "This snippet already has that trigger." }
  return { ok: t }
}

export interface TriggerInputHandle {
  focus: () => void
}

interface Props {
  triggers: string[]
  onChange: (triggers: string[]) => void
  caseSensitive: boolean
  prefix: string
  /** Triggers (lowercased) that collide with another snippet. */
  conflicted: Set<string>
  id?: string
}

/** Token input for a snippet's triggers. Space or Enter adds; Backspace on empty edits the last. */
export const TriggerInput = forwardRef<TriggerInputHandle, Props>(function TriggerInput(
  { triggers, onChange, caseSensitive, prefix, conflicted, id },
  ref,
) {
  const input = useRef<HTMLInputElement>(null)
  const [draft, setDraft] = useState("")
  const [error, setError] = useState<string | null>(null)
  useImperativeHandle(ref, () => ({ focus: () => input.current?.focus() }))

  const commit = (raw = draft): boolean => {
    if (!raw.trim()) return false
    const parts = raw.split(/\s+/).filter(Boolean)
    let next = [...triggers]
    for (const p of parts) {
      const r = validateTrigger(p, next, caseSensitive)
      if ("error" in r) {
        setError(r.error)
        return false
      }
      next = [...next, r.ok]
    }
    onChange(next)
    setDraft("")
    setError(null)
    return true
  }

  return (
    <div className="flex flex-col gap-1.5">
      <div
        className={cn(
          "group flex min-h-9 flex-wrap items-center gap-1.5 rounded-lg bg-foreground/[0.025] px-1.5 py-1 shadow-well transition-shadow dark:bg-black/15",
          "focus-within:bg-transparent focus-within:shadow-none focus-within:ring-2 focus-within:ring-ring",
          error && "ring-1 ring-destructive/60 focus-within:ring-destructive/60",
        )}
        onClick={() => input.current?.focus()}
      >
        {triggers.map((t, i) => {
          const bad = conflicted.has(t.toLowerCase())
          return (
            <span
              key={`${t}-${i}`}
              className={cn(
                "group/chip inline-flex h-6 items-center gap-1 rounded-md pr-1 pl-2 font-mono text-[12px] animate-pop",
                bad ? "bg-warning/15 text-warning" : "bg-iris-soft text-iris-foreground",
              )}
              title={bad ? "Conflicts with another snippet" : undefined}
            >
              {t}
              <button
                type="button"
                className="flex size-4 items-center justify-center rounded opacity-50 hover:bg-foreground/10 hover:opacity-100 focus-visible:opacity-100"
                aria-label={`Remove trigger ${t}`}
                onClick={(e) => {
                  e.stopPropagation()
                  onChange(triggers.filter((_, j) => j !== i))
                }}
              >
                <X className="size-3" />
              </button>
            </span>
          )
        })}
        <input
          ref={input}
          id={id}
          value={draft}
          spellCheck={false}
          autoCapitalize="off"
          autoCorrect="off"
          aria-label="Add trigger"
          aria-invalid={!!error}
          aria-describedby={error ? `${id}-error` : undefined}
          placeholder={triggers.length ? "Add another…" : `${prefix}trigger`}
          className="h-6 min-w-24 flex-1 bg-transparent px-1 font-mono text-[12.5px] outline-none placeholder:font-sans placeholder:text-faint"
          onFocus={() => {
            if (!draft && !triggers.length && prefix) setDraft(prefix)
          }}
          onBlur={() => {
            if (draft === prefix) setDraft("")
            else if (draft.trim()) commit()
          }}
          onChange={(e) => {
            setError(null)
            const v = e.target.value
            if (/\s$/.test(v) && v.trim()) commit(v)
            else setDraft(v.replace(/^\s+/, ""))
          }}
          onPaste={(e) => {
            const text = e.clipboardData.getData("text")
            if (/\s/.test(text.trim())) {
              e.preventDefault()
              commit(draft + text)
            }
          }}
          onKeyDown={(e) => {
            if (e.key === "Enter" || (e.key === "Tab" && draft.trim() && draft !== prefix)) {
              if (draft.trim() && draft !== prefix) {
                e.preventDefault()
                commit()
              }
            } else if (e.key === "Backspace" && !draft && triggers.length) {
              e.preventDefault()
              const last = triggers[triggers.length - 1]
              onChange(triggers.slice(0, -1))
              setDraft(last)
            } else if (e.key === "Escape" && draft) {
              e.preventDefault()
              e.stopPropagation()
              setDraft("")
              setError(null)
            }
          }}
        />
        {!draft && triggers.length > 0 && (
          <Plus className="mr-1 size-3.5 text-faint opacity-0 transition-opacity group-hover:opacity-100" aria-hidden />
        )}
      </div>
      {error && (
        <p id={`${id}-error`} role="alert" className="text-xs text-destructive">
          {error}
        </p>
      )}
    </div>
  )
})
