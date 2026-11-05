"use client"

// Snippet content editor: plain-text template source in CodeMirror, with every tag replaced
// by an atomic inline chip. The stored text never changes shape — chips are decorations —
// so what you see round-trips byte-for-byte through the Rust parser.

import { defaultKeymap, history, historyKeymap, indentWithTab } from "@codemirror/commands"
import { EditorState, RangeSetBuilder, StateEffect, StateField, type Extension } from "@codemirror/state"
import {
  Decoration,
  type DecorationSet,
  EditorView,
  keymap,
  placeholder as cmPlaceholder,
  WidgetType,
} from "@codemirror/view"
import { forwardRef, useEffect, useImperativeHandle, useRef } from "react"

import { chipLabel, chipTone, tokenize, type TagToken } from "@/lib/template"

export interface ChipClick {
  tag: TagToken
  rect: DOMRect
}

export interface SlashOpen {
  /** Position of the typed "/". */
  pos: number
  rect: DOMRect
}

export interface TemplateEditorHandle {
  focus: () => void
  /** Inserts text at the selection (replacing it) and returns the inserted range. */
  insert: (text: string) => { from: number; to: number }
  replace: (from: number, to: number, text: string, focus?: boolean) => { from: number; to: number }
  coordsAt: (pos: number) => DOMRect | null
  view: () => EditorView | null
}

interface Props {
  value: string
  onChange: (value: string) => void
  snippetName: (id: string) => string | undefined
  onChipClick: (click: ChipClick) => void
  onSlash: (open: SlashOpen) => void
  placeholder?: string
  ariaLabel?: string
  className?: string
  onEscape?: () => void
}

class ChipWidget extends WidgetType {
  constructor(
    readonly tag: TagToken,
    readonly label: string,
  ) {
    super()
  }
  eq(other: ChipWidget) {
    return other.tag.source === this.tag.source && other.label === this.label
  }
  toDOM() {
    const el = document.createElement("span")
    el.className = `kz-chip kz-chip-${chipTone(this.tag)}`
    el.textContent = this.label
    el.setAttribute("role", "button")
    el.setAttribute("aria-label", `${this.label} — click to edit`)
    el.dataset.from = String(this.tag.from)
    el.title = this.tag.verbatim ? `Not recognised: ${this.tag.source}` : this.tag.source
    return el
  }
  ignoreEvent() {
    return false
  }
}

const setResolver = StateEffect.define<(id: string) => string | undefined>()

function buildDecorations(state: EditorState, resolve: (id: string) => string | undefined): DecorationSet {
  const builder = new RangeSetBuilder<Decoration>()
  for (const t of tokenize(state.doc.toString())) {
    if (t.kind !== "tag") continue
    builder.add(t.from, t.to, Decoration.replace({ widget: new ChipWidget(t, chipLabel(t, resolve)) }))
  }
  return builder.finish()
}

function chipsField(initial: (id: string) => string | undefined): Extension {
  let resolve = initial
  const field = StateField.define<DecorationSet>({
    create: (state) => buildDecorations(state, resolve),
    update(deco, tr) {
      let changed = tr.docChanged
      for (const e of tr.effects)
        if (e.is(setResolver)) {
          resolve = e.value
          changed = true
        }
      return changed ? buildDecorations(tr.state, resolve) : deco
    },
    provide: (f) => [EditorView.decorations.from(f), EditorView.atomicRanges.of((view) => view.state.field(f))],
  })
  return field
}

const theme = EditorView.theme({
  "&": { height: "100%" },
  ".kz-chip": {
    display: "inline-flex",
    alignItems: "center",
    height: "20px",
    padding: "0 7px",
    margin: "0 1px",
    borderRadius: "6px",
    fontFamily: "var(--app-sans)",
    fontSize: "11.5px",
    fontWeight: "500",
    lineHeight: "1",
    verticalAlign: "1px",
    cursor: "pointer",
    whiteSpace: "nowrap",
    transition: "background 120ms, box-shadow 120ms",
  },
})


export const TemplateEditor = forwardRef<TemplateEditorHandle, Props>(function TemplateEditor(
  { value, onChange, snippetName, onChipClick, onSlash, placeholder, ariaLabel, className, onEscape },
  ref,
) {
  const host = useRef<HTMLDivElement>(null)
  const viewRef = useRef<EditorView | null>(null)
  const cb = useRef({ onChange, onChipClick, onSlash, onEscape })
  cb.current = { onChange, onChipClick, onSlash, onEscape }

  useEffect(() => {
    const view = new EditorView({
      parent: host.current!,
      state: EditorState.create({
        doc: value,
        extensions: [
          history(),
          EditorView.lineWrapping,
          cmPlaceholder(placeholder ?? ""),
          chipsField(snippetName),
          theme,
          EditorView.contentAttributes.of({ "aria-label": ariaLabel ?? "Snippet content", "aria-multiline": "true", spellcheck: "true" }),
          keymap.of([
            {
              key: "Escape",
              run: () => {
                cb.current.onEscape?.()
                return !!cb.current.onEscape
              },
            },
            ...defaultKeymap,
            ...historyKeymap,
            indentWithTab,
          ]),
          EditorView.updateListener.of((u) => {
            if (!u.docChanged) return
            cb.current.onChange(u.state.doc.toString())
            // Slash menu: a "/" typed at the start of a word.
            if (!u.transactions.some((t) => t.isUserEvent("input.type"))) return
            u.changes.iterChanges((_fa, _ta, fromB, toB, inserted) => {
              if (inserted.toString() !== "/" || toB - fromB !== 1) return
              const before = fromB > 0 ? u.state.doc.sliceString(fromB - 1, fromB) : ""
              if (before && !/\s|[(["']/.test(before)) return
              requestAnimationFrame(() => {
                const c = u.view.coordsAtPos(fromB)
                if (c) cb.current.onSlash({ pos: fromB, rect: new DOMRect(c.left, c.top, c.right - c.left, c.bottom - c.top) })
              })
            })
          }),
          EditorView.domEventHandlers({
            mousedown: (e, view) => {
              const el = (e.target as HTMLElement).closest(".kz-chip") as HTMLElement | null
              if (!el) return false
              e.preventDefault()
              const pos = view.posAtDOM(el)
              const tag = tokenize(view.state.doc.toString()).find((t) => t.kind === "tag" && t.from === pos) as TagToken | undefined
              if (tag) cb.current.onChipClick({ tag, rect: el.getBoundingClientRect() })
              return true
            },
          }),
        ],
      }),
    })
    viewRef.current = view
    return () => {
      view.destroy()
      viewRef.current = null
    }
    // The editor is created once per mount; `value` changes are synced below.
    // oxlint-disable-next-line react/exhaustive-deps
  }, [])

  // External value changes (switching snippets, chip edits from popovers).
  useEffect(() => {
    const view = viewRef.current
    if (!view) return
    const cur = view.state.doc.toString()
    if (cur !== value) view.dispatch({ changes: { from: 0, to: cur.length, insert: value } })
  }, [value])

  useEffect(() => {
    viewRef.current?.dispatch({ effects: setResolver.of(snippetName) })
  }, [snippetName])

  useImperativeHandle(ref, () => ({
    focus: () => viewRef.current?.focus(),
    insert: (text) => {
      const view = viewRef.current!
      const { from, to } = view.state.selection.main
      view.dispatch({ changes: { from, to, insert: text }, selection: { anchor: from + text.length }, userEvent: "input" })
      view.focus()
      return { from, to: from + text.length }
    },
    replace: (from, to, text, focus = true) => {
      const view = viewRef.current!
      view.dispatch({ changes: { from, to, insert: text }, selection: { anchor: from + text.length }, userEvent: "input" })
      if (focus) view.focus()
      return { from, to: from + text.length }
    },
    coordsAt: (pos) => {
      const c = viewRef.current?.coordsAtPos(pos)
      return c ? new DOMRect(c.left, c.top, c.right - c.left, c.bottom - c.top) : null
    },
    view: () => viewRef.current,
  }))

  return <div ref={host} className={`kz-editor ${className ?? ""}`} />
})
