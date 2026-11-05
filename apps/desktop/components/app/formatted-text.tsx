import { Fragment } from "react"

import { formattedParts } from "@/lib/formatting"
import type { RenderResult } from "@/lib/types"

export function FormattedText({ result }: { result: RenderResult }) {
  const cursor = <span aria-label="cursor" className="mx-px inline-block h-[1.1em] w-[2px] translate-y-[3px] rounded-full bg-iris" />
  return <>
    {formattedParts(result.text, result.formatting ?? [], result.cursorOffset).map((part) => {
      const link = part.styles.find((s) => s.kind === "link")
      return <Fragment key={part.start}>
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
    {result.cursorOffset === result.text.length && cursor}
  </>
}
