import assert from "node:assert/strict"
import * as module from "node:module"
import * as loader from "./typescript-loader.mjs"
import { fileURLToPath } from "node:url"
import test from "node:test"
import { execFileSync } from "node:child_process"

// Use the real editor state and renderer without adding a test framework.
if (module.registerHooks) module.registerHooks(loader)
else module.register(new URL("./typescript-loader.mjs", import.meta.url))

const { EditorState } = await import("@codemirror/state")
const { history, undo, redo, isolateHistory } = await import("@codemirror/commands")
const { formattingField, formattingHistory, setFormatting, changeFormat, toggleList } = await import("../../lib/editor-formatting.ts")
const { renderLocal } = await import("../../lib/render-local.ts")
const { formattedHtml, safeImage, safeLink } = await import("../../lib/formatting.ts")
const PNG = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAABgAAAAYCAYAAADgdz34AAAAJUlEQVR4nGMICLj0n5aYYdSCUQtGLRi1YNSCUQtGLRi1YGhYAACpwH6M7RqhEQAAAABJRU5ErkJggg=="

function editor(doc) {
  let state = EditorState.create({ doc, extensions: [history(), formattingField.init(() => []), formattingHistory] })
  return {
    get state() { return state }, composing: false, focus() {},
    dispatch(transaction) { state = transaction.state ?? state.update(transaction).state },
  }
}

test("format, edit, undo and redo preserve formatting with the text", () => {
  const target = editor("Hello world")
  target.dispatch({ selection: { anchor: 0, head: 5 } })
  changeFormat(target, { kind: "bold" })
  target.dispatch({ changes: { from: 0, to: 5, insert: "Hi" }, userEvent: "input" })
  assert.equal(formattedHtml(target.state.doc.toString(), target.state.field(formattingField)).includes("<strong>Hi</strong> world"), true)
  undo(target)
  assert.equal(target.state.doc.toString(), "Hello world")
  assert.deepEqual(target.state.field(formattingField), [{ start: 0, end: 5, style: { kind: "bold" } }])
  undo(target)
  assert.deepEqual(target.state.field(formattingField), [])
  redo(target)
  assert.deepEqual(target.state.field(formattingField), [{ start: 0, end: 5, style: { kind: "bold" } }])
})

test("removing formatting and switching to plain text are undoable", () => {
  const target = editor("Hi")
  target.dispatch({ selection: { anchor: 0, head: 2 } })
  changeFormat(target, { kind: "italic" })
  target.dispatch({ effects: setFormatting.of(null), userEvent: "input.format" })
  assert.equal(target.state.field(formattingField), null)
  undo(target)
  assert.deepEqual(target.state.field(formattingField), [{ start: 0, end: 2, style: { kind: "italic" } }])
})

test("template values are escaped and styled after rendering", () => {
  const source = String.raw`😀 \{{literal}} {{field name=who}}{{cursor}}!`
  const start = source.indexOf("{{field")
  const result = renderLocal(source, {
    formValues: { who: "<Sam & سلمى>" },
    formatting: { source, spans: [{ start, end: start + "{{field name=who}}".length, style: { kind: "bold" } }] },
  })
  assert.equal(result.text, "😀 {{literal}} <Sam & سلمى>!")
  assert.equal(result.cursorOffset, 27)
  assert.equal(formattedHtml(result.text, result.formatting).includes("<strong>&lt;Sam &amp; سلمى&gt;</strong>!"), true)
  assert.equal(renderLocal("edited", { formatting: { source, spans: [] } }).formatting, undefined)
})

test("typing beside an image leaves it intact; deleting it can be undone", () => {
  const target = editor("[Logo]")
  const image = { start: 0, end: 6, style: { kind: "image", dataUrl: PNG, width: 120 } }
  target.dispatch({ effects: setFormatting.of([image]) })
  target.dispatch({ changes: { from: 6, insert: "!" }, userEvent: "input" })
  assert.deepEqual(target.state.field(formattingField), [image])
  target.dispatch({ changes: { from: 0, to: 6 }, annotations: isolateHistory.of("full"), userEvent: "delete" })
  assert.equal(target.state.doc.toString(), "!")
  assert.deepEqual(target.state.field(formattingField), [])
  undo(target)
  assert.equal(target.state.doc.toString(), "[Logo]!")
  assert.deepEqual(target.state.field(formattingField), [image])
})

test("lists preserve plain-text content and can be toggled off", () => {
  const target = editor("One\nTwo")
  target.dispatch({ selection: { anchor: 0, head: 7 } })
  toggleList(target, true)
  assert.equal(target.state.doc.toString(), "1. One\n2. Two")
  toggleList(target, true)
  assert.equal(target.state.doc.toString(), "One\nTwo")
})

test("only inert links and embedded raster images enter clipboard HTML", () => {
  assert.equal(safeLink("javascript:alert(1)"), false)
  assert.equal(safeImage("data:image/svg+xml;base64,PHN2Zz4="), false)
  assert.equal(safeImage("https://example.com/a.png"), false)
  assert.equal(safeImage(PNG), true)
  assert.equal(formattedHtml("[Logo]", [{ start: 0, end: 6, style: { kind: "image", dataUrl: PNG, width: 120 } }]).includes('width="120" alt="[Logo]"'), true)
})

test("formatted nested signatures retain images, plain roots use the text alternative", () => {
  const id = "00000000-0000-0000-0000-000000000001"
  const source = `Hello\n{{snippet id="${id}"}}`
  const opts = {
    resolve: () => "[Logo]",
    resolveFormatting: () => ({ source: "[Logo]", spans: [{ start: 0, end: 6, style: { kind: "image", dataUrl: PNG, width: 120 } }] }),
  }
  const rich = renderLocal(source, { ...opts, formatting: { source, spans: [] } })
  assert.equal(rich.text, "Hello\n[Logo]")
  assert.deepEqual(rich.formatting.map((s) => [s.start, s.end]), [[6, 12]])
  assert.equal(renderLocal(source, opts).formatting, undefined)
})

test("macOS imports generated HTML as bold text and a real embedded attachment", { skip: process.platform !== "darwin" }, () => {
  const text = "Hello\n[Logo] world"
  const html = formattedHtml(text, [
    { start: 0, end: 5, style: { kind: "bold" } },
    { start: 6, end: 12, style: { kind: "image", dataUrl: PNG, width: 120 } },
  ])
  const result = execFileSync("swift", [fileURLToPath(new URL("./formatted-paste.swift", import.meta.url))], {
    input: JSON.stringify({ html, text: "Hello\n\ufffc world", attachments: 1 }), encoding: "utf8", timeout: 30000,
  })
  assert.match(result, /verified/)
})
