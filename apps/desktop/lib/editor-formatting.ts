import { invertedEffects, isolateHistory } from "@codemirror/commands"
import { StateEffect, StateField, type ChangeDesc, type EditorState } from "@codemirror/state"
import { Decoration, EditorView, WidgetType } from "@codemirror/view"

import { sameStyle, validSpans } from "./formatting"
import { tokenize } from "./template"
import type { FormatSpan, FormatStyle } from "./types"

function mapSpans(spans: FormatSpan[] | null, changes: ChangeDesc) {
  return spans?.map((s) => ({ ...s, start: changes.mapPos(s.start, s.style.kind === "image" ? 1 : -1), end: changes.mapPos(s.end, s.style.kind === "image" ? -1 : 1) }))
    .filter((s) => s.start < s.end) ?? null
}

class ImageWidget extends WidgetType {
  constructor(readonly span: FormatSpan, readonly alt: string) { super() }
  eq(other: ImageWidget) { return this.alt === other.alt && sameStyle(this.span.style, other.span.style) && this.span.start === other.span.start && this.span.end === other.span.end }
  toDOM(view: EditorView) {
    const image = document.createElement("img")
    const style = this.span.style
    if (style.kind === "image") {
      image.src = style.dataUrl
      image.width = style.width
      image.alt = this.alt
    }
    image.className = "inline-block max-w-full align-middle rounded-sm"
    image.draggable = false
    image.addEventListener("mousedown", (e) => {
      e.preventDefault()
      view.dispatch({ selection: { anchor: this.span.start, head: this.span.end } })
      view.focus()
    })
    return image
  }
  ignoreEvent() { return true }
}

function imageDecorations(spans: FormatSpan[] | null, text: string) {
  return Decoration.set((spans ?? []).filter((s) => s.style.kind === "image")
    .map((s) => Decoration.replace({ widget: new ImageWidget(s, text.slice(s.start, s.end)) }).range(s.start, s.end)), true)
}

export const setFormatting = StateEffect.define<FormatSpan[] | null>({ map: mapSpans })
export const formattingField = StateField.define<FormatSpan[] | null>({
  create: () => null,
  update(value, tr) {
    let next = tr.docChanged ? mapSpans(value, tr.changes) : value
    for (const effect of tr.effects) if (effect.is(setFormatting)) next = effect.value
    if (next === value) return value
    return next === null ? null : validSpans(tr.newDoc.toString(), next)
  },
  provide: (field) => [
    EditorView.decorations.from(field, (spans) => Decoration.set((spans ?? []).filter((s) => s.style.kind !== "image").map((s) =>
      Decoration.mark({ class: `kz-format-${s.style.kind}` }).range(s.start, s.end)), true)),
    EditorView.decorations.compute([field], (state) => imageDecorations(state.field(field), state.doc.toString())),
    EditorView.atomicRanges.of((view) => imageDecorations(view.state.field(field), view.state.doc.toString())),
  ],
})

export const formattingHistory = invertedEffects.of((tr) =>
  tr.docChanged || tr.effects.some((e) => e.is(setFormatting))
    ? [setFormatting.of(tr.startState.field(formattingField))] : [])

export function selectedRange(state: EditorState) {
  let { from, to } = state.selection.main
  if (from === to) return { from, to }
  for (const token of tokenize(state.doc.toString())) {
    if (token.kind === "tag" && token.from < to && token.to > from) {
      from = Math.min(from, token.from)
      to = Math.max(to, token.to)
    }
  }
  for (const span of state.field(formattingField) ?? []) {
    if (span.style.kind === "image" && span.start < to && span.end > from) {
      from = Math.min(from, span.start)
      to = Math.max(to, span.end)
    }
  }
  return { from, to }
}

export function changeFormat(view: EditorView, style: FormatStyle | "clear") {
  const { from, to } = selectedRange(view.state)
  if (from === to || view.state.field(formattingField) === null || view.composing) return false
  const spans = view.state.field(formattingField) ?? []
  const remove = style === "clear" || spans.some((s) => s.start <= from && s.end >= to && sameStyle(s.style, style))
  const next = spans.flatMap((s) => {
    if (s.end <= from || s.start >= to || (style !== "clear" && s.style.kind !== style.kind)) return [s]
    return [
      ...(s.start < from ? [{ ...s, end: from }] : []),
      ...(s.end > to ? [{ ...s, start: to }] : []),
    ]
  })
  if (!remove) next.push({ start: from, end: to, style })
  view.dispatch({ effects: setFormatting.of(next), annotations: isolateHistory.of("full"), userEvent: "input.format" })
  view.focus()
  return true
}

/** Prefixes are real text so lists keep their numbering in plain-text apps and exports. */
export function toggleList(view: EditorView, ordered: boolean) {
  if (view.composing) return
  const { from, to } = view.state.selection.main
  const first = view.state.doc.lineAt(from).number
  const last = view.state.doc.lineAt(to > from ? to - 1 : to).number
  const lines = Array.from({ length: last - first + 1 }, (_, i) => view.state.doc.line(first + i))
  const marker = ordered ? /^\d+\. / : /^• /
  const remove = lines.every((l) => marker.test(l.text))
  const changes = lines.map((line, i) => ({
    from: line.from, to: line.from + (/^(?:• |\d+\. )/.exec(line.text)?.[0].length ?? 0),
    insert: remove ? "" : ordered ? `${i + 1}. ` : "• ",
  }))
  view.dispatch({ changes, annotations: isolateHistory.of("full"), userEvent: "input" })
  view.focus()
}
