import { Fragment } from "react"

import { formattedParts } from "@/lib/formatting"
import { formatDelay, keyInfo } from "@/lib/template"
import type { RenderAction, RenderResult } from "@/lib/types"

import { Shortcut } from "./shortcut"

/** A key press or pause shown where it happens in a preview: a keycap, never part of the text. */
function ActionMark({ action }: { action: RenderAction }) {
  if (action.kind === "pause") {
    return (
      <span className="mx-0.5 inline-flex h-[18px] items-center rounded-[5px] px-1.5 align-[1px] text-[10.5px] font-medium text-faint shadow-well select-none" title={`Waits ${formatDelay(action.ms)}`}>
        {formatDelay(action.ms)}
      </span>
    )
  }
  const key = keyInfo(action.key)
  const label = key?.label ?? action.key
  return (
    <span className="mx-0.5 inline-flex items-center gap-0.5 align-[1px] select-none" title={`Presses ${label}${action.count > 1 ? ` ${action.count} times` : ""}`}>
      <Shortcut keys={[key?.cap ?? action.key]} />
      {action.count > 1 && <span className="text-[10.5px] font-medium text-faint tabular">×{action.count}</span>}
      <span className="sr-only">{`press ${label}${action.count > 1 ? ` ${action.count} times` : ""}`}</span>
    </span>
  )
}

export function FormattedText({ result }: { result: RenderResult }) {
  const cursor = <span aria-label="cursor" className="mx-px inline-block h-[1.1em] w-[2px] translate-y-[3px] rounded-full bg-iris" />
  const actions = result.actions ?? []
  const marks = (at: number) => actions.map((action, i) => action.offset === at && <ActionMark key={`a${i}`} action={action} />)
  return <>
    {formattedParts(result.text, result.formatting ?? [], result.cursorOffset, actions.map((a) => a.offset)).map((part) => {
      const link = part.styles.find((s) => s.kind === "link")
      return <Fragment key={part.start}>
        {marks(part.start)}
        {result.cursorOffset === part.start && cursor}
        {part.image?.style.kind === "image" ? (part.start === part.image.start &&
          // oxlint-disable-next-line nextjs/no-img-element -- embedded raster data, no server optimizer
          <img src={part.image.style.dataUrl} width={part.image.style.width} alt={result.text.slice(part.image.start, part.image.end)} className="inline-block max-w-full align-middle" />
        ) : <span title={link?.kind === "link" ? link.url : undefined} style={{
          fontWeight: part.styles.some((s) => s.kind === "bold") ? 700 : undefined,
          fontStyle: part.styles.some((s) => s.kind === "italic") ? "italic" : undefined,
          textDecoration: part.styles.some((s) => s.kind === "underline" || s.kind === "link") ? "underline" : undefined,
          color: link ? "var(--iris)" : undefined,
        }}>{part.text}</span>}
      </Fragment>
    })}
    {marks(result.text.length)}
    {result.cursorOffset === result.text.length && cursor}
  </>
}
