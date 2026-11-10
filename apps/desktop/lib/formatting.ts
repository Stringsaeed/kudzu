import type { FormatSpan, FormatStyle, Formatting } from "./types"

export function safeLink(url: string): boolean {
  // oxlint-disable-next-line no-control-regex -- reject control characters in imported URLs
  return /^(https?:\/\/|mailto:)/i.test(url) && !/[\s\x00-\x1f\x7f"'<>]/.test(url)
}

export function safeImage(dataUrl: string): boolean {
  const match = /^(?:data:image\/png;base64,(iVBORw0KGgo[A-Za-z0-9+/=]*)|data:image\/jpeg;base64,(\/9j\/[A-Za-z0-9+/=]*))$/.exec(dataUrl)
  const data = match?.[1] ?? match?.[2]
  return dataUrl.length <= 196608 && !!data && data.length % 4 === 0
}

export function validSpans(text: string, spans: FormatSpan[]): FormatSpan[] {
  const boundary = (pos: number) => Number.isInteger(pos) && pos >= 0 && pos <= text.length &&
    !(pos > 0 && /[\uD800-\uDBFF]/.test(text[pos - 1]) && /[\uDC00-\uDFFF]/.test(text[pos] ?? ""))
  return spans.filter((s) => s.start < s.end && boundary(s.start) && boundary(s.end) &&
    (s.style.kind !== "link" || safeLink(s.style.url)) &&
    (s.style.kind !== "image" || (safeImage(s.style.dataUrl) && Number.isInteger(s.style.width) && s.style.width >= 1 && s.style.width <= 512)))
}

export function sourceSpans(content: string, formatting?: Formatting): FormatSpan[] | null {
  return formatting?.source === content ? validSpans(content, formatting.spans) : null
}

export function literalPositions(raw: string, start: number, output: number, positions: Map<number, number>) {
  const graphemes = Array.from(new Intl.Segmenter(undefined, { granularity: "grapheme" }).segment(raw), (s) => s.segment)
  let input = start
  let rendered = output
  positions.set(input, rendered)
  for (let i = 0; i < graphemes.length; i++) {
    if (graphemes[i] === "\\" && (graphemes[i + 1] === "\\" || (graphemes[i + 1] === "{" && graphemes[i + 2] === "{"))) {
      input++
      i++
    }
    input += graphemes[i].length
    rendered += graphemes[i].length
    positions.set(input, rendered)
  }
}

export function sameStyle(a: FormatStyle, b: FormatStyle): boolean {
  return a.kind === b.kind && (a.kind !== "link" || (b.kind === "link" && a.url === b.url)) &&
    (a.kind !== "image" || (b.kind === "image" && a.dataUrl === b.dataUrl && a.width === b.width))
}

/** Splits overlapping spans into independently styled pieces, without interpreting HTML. */
export function formattedParts(text: string, spans: FormatSpan[] = [], cursor?: number | null, breaks: number[] = []) {
  const valid = validSpans(text, spans)
  const boundaries = [...new Set([0, text.length, ...valid.flatMap((s) => [s.start, s.end]),
    ...[...breaks, ...(cursor != null ? [cursor] : [])].filter((at) => at >= 0 && at <= text.length)])].sort((a, b) => a - b)
  return boundaries.slice(0, -1).map((start, i) => ({
    start, text: text.slice(start, boundaries[i + 1]),
    image: valid.find((s) => s.style.kind === "image" && s.start <= start && s.end > start),
    styles: valid.filter((s) => s.start <= start && s.end >= boundaries[i + 1]).map((s) => s.style),
  }))
}

function escapeHtml(text: string) {
  return text.replaceAll("&", "&amp;").replaceAll("<", "&lt;").replaceAll(">", "&gt;").replaceAll('"', "&quot;").replaceAll("'", "&#39;")
}

export function formattedHtml(text: string, spans: FormatSpan[]): string {
  const body = formattedParts(text, spans).map(({ start, text: part, styles, image }) => {
    if (image?.style.kind === "image") return image.start === start
      ? `<img src="${image.style.dataUrl}" width="${image.style.width}" alt="${escapeHtml(text.slice(image.start, image.end))}">` : ""
    let html = escapeHtml(part).replaceAll("\n", "<br>")
    if (styles.some((s) => s.kind === "underline")) html = `<u>${html}</u>`
    if (styles.some((s) => s.kind === "italic")) html = `<em>${html}</em>`
    if (styles.some((s) => s.kind === "bold")) html = `<strong>${html}</strong>`
    const link = styles.find((s) => s.kind === "link")
    if (link?.kind === "link") html = `<a href="${escapeHtml(link.url)}">${html}</a>`
    return html
  }).join("")
  return `<html><head><meta charset="utf-8"></head><body><span style="white-space: pre-wrap; font-family: -apple-system, sans-serif;">${body}</span></body></html>`
}

export function sameSpans(a: FormatSpan[] | null, b: FormatSpan[] | null): boolean {
  return a === b || (a !== null && b !== null && a.length === b.length && a.every((span, i) =>
    span.start === b[i].start && span.end === b[i].end && sameStyle(span.style, b[i].style)))
}
