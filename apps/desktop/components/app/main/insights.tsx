"use client"

import { Lock } from "lucide-react"
import { useEffect, useMemo, useState } from "react"

import { Item } from "@/components/ui/item"
import { useStore } from "@/lib/store"
import { CHARS_PER_WORD, formatCount, formatDuration, relativeTime, totals } from "@/lib/stats"

import { TriggerChip } from "../trigger-chip"

const COUNT_UP_MS = 560

/**
 * Counts a formatted stat ("471", "27k", "2.3 h") up from zero, once, when the page arrives. Only the
 * leading number moves, in the final value's own unit and precision, so the text keeps its shape;
 * the last frame is the formatted string itself. Later changes (the typing-speed slider) show at once.
 */
function useCountUp(value: string) {
  // Insights only mounts in the browser (the shell waits for the store), so reading the media query here is safe.
  const [progress, setProgress] = useState(() => (window.matchMedia("(prefers-reduced-motion: reduce)").matches ? 1 : 0))
  useEffect(() => {
    if (progress >= 1) return
    let frame = 0
    let start = 0
    const tick = (now: number) => {
      start ||= now
      const t = Math.min(1, (now - start) / COUNT_UP_MS)
      setProgress(t)
      if (t < 1) frame = requestAnimationFrame(tick)
    }
    frame = requestAnimationFrame(tick)
    return () => cancelAnimationFrame(frame)
    // Once, on arrival: `progress` is only read to skip the run under reduced motion.
    // oxlint-disable-next-line react/exhaustive-deps
  }, [])

  if (progress >= 1) return value
  const match = /^(\d+(?:\.(\d+))?)(.*)$/.exec(value)
  if (!match) return value
  const eased = 1 - (1 - progress) ** 4
  return (Number(match[1]) * eased).toFixed(match[2]?.length ?? 0) + match[3]
}

function Tile({ label, value, sub }: { label: string; value: string; sub?: string }) {
  const shown = useCountUp(value)
  return (
    <div className="group/tile flex flex-col gap-1 rounded-xl bg-surface p-4 shadow-card transition-[translate] duration-(--motion-moderate) ease-spring hover:-translate-y-px">
      <span className="text-xs text-muted-foreground">{label}</span>
      <span className="text-[26px] leading-none font-semibold tracking-[-0.02em] tabular">
        <span aria-hidden>{shown}</span>
        <span className="sr-only">{value}</span>
      </span>
      {sub && <span className="text-[11px] text-faint transition-colors duration-(--motion-moderate) group-hover/tile:text-muted-foreground">{sub}</span>}
    </div>
  )
}

export function Insights() {
  const library = useStore((s) => s.library)
  const usage = useStore((s) => s.usage)
  const settings = useStore((s) => s.settings)
  const updateSettings = useStore((s) => s.updateSettings)
  const select = useStore((s) => s.select)
  const setView = useStore((s) => s.setView)
  const t = useMemo(() => totals(library, usage, settings.typingSpeedWPM), [library, usage, settings.typingSpeedWPM])

  const ranked = useMemo(
    () =>
      library.snippets
        .map((s) => ({ s, u: usage.get(s.id) }))
        .filter((x) => x.u && x.u.count > 0)
        .sort((a, b) => b.u!.count - a.u!.count),
    [library.snippets, usage],
  )
  const top = ranked.slice(0, 10)
  const max = top[0]?.u?.count ?? 1
  const unused = library.snippets.filter((s) => s.isEnabled && !usage.get(s.id)?.count && s.triggers.length)
  const recent = [...ranked].sort((a, b) => b.u!.last - a.u!.last).slice(0, 6)

  const open = (id: string) => {
    setView({ kind: "all" })
    select(id)
  }

  return (
    <div className="flex h-full min-h-0 flex-1 flex-col">
      <header className="drag flex h-12 shrink-0 items-center border-b border-hairline px-6" data-tauri-drag-region>
        <h1 className="text-[13px] font-semibold">Insights</h1>
      </header>
      <div className="min-h-0 flex-1 overflow-y-auto">
        <div className="mx-auto flex max-w-[880px] flex-col gap-8 px-8 py-8 stagger">
          <div className="grid grid-cols-4 gap-3">
            <Tile label="Expansions" value={formatCount(t.expansions)} sub={`${t.usedSnippets} snippets used`} />
            <Tile label="Characters saved" value={formatCount(t.chars)} sub={`≈ ${formatCount(Math.round(t.chars / CHARS_PER_WORD))} words`} />
            <Tile label="Time saved" value={formatDuration(t.minutesSaved)} sub={`at ${settings.typingSpeedWPM} words per minute`} />
            <Tile label="Active snippets" value={String(t.activeSnippets)} sub={`${library.snippets.length - t.activeSnippets} turned off`} />
          </div>

          <div className="grid grid-cols-[1.4fr_1fr] gap-6">
            <section aria-labelledby="top-title" className="flex flex-col gap-3">
              <h2 id="top-title" className="text-[13px] font-semibold">Most used</h2>
              {top.length ? (
                <ol className="flex flex-col gap-0.5">
                  {top.map(({ s, u }, i) => (
                    <li key={s.id}>
                      <Item asChild className="group grid grid-cols-[18px_minmax(0,1fr)_auto] gap-3 px-2 py-1.5 text-left press [--press-scale:0.99] hover:bg-foreground/[0.04]">
                        <button onClick={() => open(s.id)}>
                          <span className="text-[11px] text-faint tabular transition-colors duration-(--motion-fast) group-hover:text-muted-foreground">{i + 1}</span>
                          <span className="flex min-w-0 flex-col gap-1">
                            <span className="flex min-w-0 items-center gap-2">
                              <span className="truncate text-[13px]">{s.name || "Untitled"}</span>
                              {s.triggers[0] && <TriggerChip trigger={s.triggers[0]} muted />}
                            </span>
                            <span className="h-1 overflow-hidden rounded-full bg-foreground/[0.05]">
                              <span className="block h-full origin-left rounded-full bg-iris/80 [transition:width_var(--motion-slow)_var(--spring-soft),background-color_var(--motion-fast)] animate-bar group-hover:bg-iris" style={{ width: `${Math.max(3, (u!.count / max) * 100)}%`, animationDelay: `${60 + i * 40}ms` }} />
                            </span>
                          </span>
                          <span className="text-xs text-muted-foreground tabular transition-colors duration-(--motion-fast) group-hover:text-foreground">{formatCount(u!.count)}</span>
                        </button>
                      </Item>
                    </li>
                  ))}
                </ol>
              ) : (
                <div className="flex flex-col items-center gap-4 rounded-xl bg-foreground/[0.03] p-6">
                  {/* A sketch of the chart to come: three bars growing in, the way real ones do. */}
                  <div aria-hidden className="flex w-40 flex-col gap-2">
                    {[100, 64, 36].map((width, i) => (
                      <span key={width} className="block h-1 origin-left rounded-full bg-foreground/[0.08] animate-bar" style={{ width: `${width}%`, animationDelay: `${60 + i * 40}ms` }} />
                    ))}
                  </div>
                  <p className="text-center text-sm text-muted-foreground">Expand a snippet and it&apos;ll show up here.</p>
                </div>
              )}
            </section>

            <div className="flex flex-col gap-6">
              <section aria-labelledby="recent-title" className="flex flex-col gap-3">
                <h2 id="recent-title" className="text-[13px] font-semibold">Recently used</h2>
                <ul className="flex flex-col gap-0.5">
                  {recent.map(({ s, u }) => (
                    <li key={s.id}>
                      <Item asChild className="group flex-nowrap gap-2 px-2 py-1.5 text-left press [--press-scale:0.99] hover:bg-foreground/[0.04]">
                        <button onClick={() => open(s.id)}>
                          <span className="min-w-0 flex-1 truncate text-[13px]">{s.name || "Untitled"}</span>
                          <span className="text-[11px] text-faint transition-colors duration-(--motion-fast) group-hover:text-muted-foreground">{relativeTime(u!.last)}</span>
                        </button>
                      </Item>
                    </li>
                  ))}
                  {!recent.length && <li className="px-2 text-xs text-muted-foreground">Nothing yet.</li>}
                </ul>
              </section>
              {unused.length > 0 && (
                <section aria-labelledby="unused-title" className="flex flex-col gap-3">
                  <h2 id="unused-title" className="flex items-baseline gap-2 text-[13px] font-semibold">
                    Never used <span className="text-xs font-normal text-faint">{unused.length}</span>
                  </h2>
                  <div className="flex flex-wrap gap-1.5">
                    {unused.slice(0, 18).map((s) => (
                      <button key={s.id} onClick={() => open(s.id)} className="rounded-md bg-surface-raised px-2 py-1 text-xs text-muted-foreground shadow-raised press hover:text-foreground hover:shadow-raised-hover active:shadow-pressed">
                        {s.name || s.triggers[0]}
                      </button>
                    ))}
                  </div>
                  <p className="text-[11px] text-faint">Forgotten triggers? Try the quick-search tray, or tidy these up.</p>
                </section>
              )}
            </div>
          </div>

          <section className="flex items-center justify-between gap-6 rounded-xl bg-surface px-4 py-3 shadow-card">
            <div>
              <div className="text-[13px] font-medium">Your typing speed</div>
              <div className="text-xs text-muted-foreground">Used to estimate time saved.</div>
            </div>
            <div className="flex items-center gap-3">
              <input
                type="range"
                min={10}
                max={150}
                step={5}
                value={settings.typingSpeedWPM}
                onChange={(e) => void updateSettings({ typingSpeedWPM: Number(e.target.value) })}
                aria-label="Typing speed in words per minute"
                className="w-48 accent-[var(--iris)]"
              />
              <span className="w-16 text-right text-sm tabular">{settings.typingSpeedWPM} wpm</span>
            </div>
          </section>

          <p className="flex items-center justify-center gap-1.5 text-[11px] text-faint">
            <Lock className="size-3" /> Kudzu counts expansions only. What you type is never recorded or sent anywhere.
          </p>
        </div>
      </div>
    </div>
  )
}
