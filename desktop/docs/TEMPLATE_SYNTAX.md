# Kudzu template syntax

Snippet content is stored as plain text with embedded **tags**. The editor shows tags as chips; this document defines the stored form. The parser and renderer live in `src-tauri/crates/kudzu-core/src/template/`.

## Tags

```
{{name}}
{{name key="value" key2=value2}}
```

- `name`: lowercase identifier `[a-z][a-z0-9_]*`.
- Attribute values are either double-quoted strings or bare tokens `[A-Za-z0-9_.+\-]+`.
  - Inside quotes, `\"` is a literal quote and `\\` a literal backslash.
  - Bare `true` / `false` are booleans. A bare **number** is an optional sign, digits and an optional fraction (`5`, `-3`, `+2.50`); there is no exponent, hex or leading/trailing-dot form. Every other bare token is a string.
  - A bare token given to an attribute that takes a **string** (`default`, `name`, `format`, `options`, `offset`, `locale`, `id`) is that attribute's text exactly as written. `{{field name=qty default=5}}` has the default `"5"`, and `default=007` is `"007"`. The same goes for bare `true` / `false`. Bare tokens are still rejected where a boolean is required (`trim=1` is malformed) or a UUID is required.
  - Attributes this version doesn't know keep their type, so `n=5` stays a number and `n=1e5` is the string `"1e5"`.
- Whitespace around `=` and between attributes is allowed.
- Any text outside a tag is literal. `\{{` produces a literal `{{`, and `\\` a literal backslash.
- A tag cannot start with `{`, so in `{{{` the first brace is literal text and the tag starts at the second. `{{{date}}` is `{` followed by a date tag; `{{{{date}}` is `{{` followed by one. There is no `\{` escape: `\{` followed by anything but `{` is still a backslash and a brace, so text such as LaTeX `\{x\}` keeps its meaning.
- Unknown tag names and malformed tags are **not** errors at render time. They render verbatim, and the parser reports a diagnostic with a source range so the editor can underline them.
  - A closed `{{…}}` span that isn't a valid tag is kept as written: the parser returns a tag node with `verbatim` set to its exact source, and the serializer writes that source back unchanged. Editing a snippet therefore never rewrites a tag a newer version might understand. The chip editor shows it as a red chip labelled with its source.
  - To write the literal text `{{DATE}}`, escape it as `\{{DATE}}`. That stays literal text on every save.
  - An unterminated `{{` (no closing `}}`, or an unclosed quote) is literal text, not a tag. Saving writes it as `\{{`.

## v1 tags

| Tag | Attributes | Renders |
|---|---|---|
| `date` | `format` (ICU pattern, default `yyyy-MM-dd`), `offset` (e.g. `+3d`, `-1w`, `+2m`, `+1y`, `-4h`, `+30min`; may combine: `+1w2d`), `locale` (e.g. `en_US`; default current) | Formatted current date ± offset |
| `time` | same as `date`, default format `HH:mm` | Alias of `date` with a time default |
| `clipboard` | `trim` (bool, default `false`): trims whitespace and newlines at both ends | Plain-text clipboard contents, or empty |
| `cursor` | — | Nothing; marks where the caret lands after insertion. Only the first `cursor` counts; later ones are diagnostics. |
| `snippet` | `id` (UUID, required) | The referenced snippet's template source, rendered recursively. Cycles are diagnostics and render empty. Max depth 8. |
| `field` | `name` (required), `default`, `multiline` (bool) | A form text field. Each `name` is one field; repeating a name reuses its value. |
| `choice` | `name` (required), `options` (pipe-separated, `\|` escapes a pipe), `default` | A form dropdown |

Any `field` or `choice` makes the snippet a **form snippet**: the engine shows the form panel before inserting.

## Rendering contract

```rust
kudzu_core::template::render(source: &str, context: &RenderContext) -> RenderResult
// RenderResult { text: String, cursor_offset: Option<usize> /* UTF-16 units from the start of `text` */, diagnostics }
```

`RenderContext` supplies `now` (with its UTC offset), `locale`, `clipboard: Option<String>`, `form_values`, `resolve_snippet: &dyn Fn(Uuid) -> Option<String>`, and optionally `root_snippet_id: Option<Uuid>`. When the root ID is set, it starts on the active reference path, so a snippet that includes itself is a cycle on the first reference instead of expanding once. The renderer is pure and deterministic given a context.

## Espanso mapping (import)

| Espanso | Kudzu |
|---|---|
| `{{mydate}}` with a `type: date` var, `params.format` (strftime) | `{{date format="<converted ICU>"}}`; strftime is converted to ICU |
| `type: clipboard` | `{{clipboard}}` |
| `$|$` | `{{cursor}}` |
| `type: match` (nested, by trigger) | `{{snippet id="<resolved id>"}}` |
| `type: form` / `[[field]]` in `form:` | `{{field name="field"}}` or `{{choice …}}` for `list`/`choice` fields |
| `type: shell` / `script` / `random` / `echo` and others | Imported as literal text with an import warning; v2 adds support |
| `word: true` | `fireMode: wholeWord` |
| `propagate_case: true` | `caseMode: propagate` (otherwise `exact`) |
| `replace` / `markdown` / `html` | Imported as plain text; rich formats get a warning (v2) |

## Parsing, serialization, and recursion details

- `resolveSnippet` returns **template source**, not rendered text. The renderer expands it with the same context. The root is depth 0; eight nested snippet levels are allowed. A reference at depth 8 is rejected. Missing references, cycles on the active reference path, and excess depth render empty with a diagnostic. Repeated references in separate branches are allowed.
- Nested `cursor` tags render nothing and never set the caret. Only the first cursor in the root template sets the UTF-16 cursor offset.
- Node and diagnostic ranges are half-open UTF-16 offsets in the containing source, including tag delimiters. Nested diagnostics refer to the nested source, not the outer insertion text.
- Text nodes carry both their decoded string and source range (`.text(String, range:)`); tag nodes carry their range on `Tag`.
- Canonical serialization preserves semantic node content, attribute types and order. Ranges are recomputed on parsing because canonical whitespace, quoting and escaping can change source lengths. Canonical inputs serialize byte-identically. All string attributes are quoted; booleans and numbers are bare. Numbers are written as plain decimals (`42`, never `42.0` or `1e+20`). Unrecognized tags are written verbatim, so they are not canonicalized.
- Outside tags, `\\` represents a literal backslash, allowing literal text ending in a backslash immediately before a tag to round-trip. Other backslashes are literal except for `\{{`. The serializer merges adjacent text, then escapes each backslash as `\\` and each `{{` pair as `\{{` (left to right, so `{{{` becomes `\{{{`). A single `{` needs no escape, even directly before a tag, because `{{{` reads as a literal brace and a tag.
- Round trip: for any source, parsing, serializing and parsing again gives the same nodes, and the serialized source renders the same text as the original. Property tests cover brace and backslash runs next to tags.
- Inside quotes, `\|` is also accepted and retained as an escaped pipe for `options` splitting. Empty interior/trailing options are preserved; absent or empty options produce an empty list.
- Duplicate attributes retain their last occurrence (and its position) and produce a warning. Unknown attributes are retained for forward compatibility. Known attributes with incorrect types make the tag unrecognized (kept verbatim) with a diagnostic, except invalid offsets, which are ignored with a warning. Required names must be strings (an empty quoted name is accepted); snippet IDs must be valid UUID strings.
- Invalid date offsets produce a warning and are ignored. One leading sign applies to every component; components are applied in written order using the supplied calendar and time zone. Month and year arithmetic clamps to valid calendar dates.
- Static analysis examines only the supplied source. Form names and referenced snippet IDs are deduplicated in first-appearance order. The first definition determines a form control's kind, options and default; rendering still uses each occurrence's own default when no form value is supplied.
