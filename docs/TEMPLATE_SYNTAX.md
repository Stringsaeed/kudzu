# Kudzu template syntax

Snippet content is stored as plain text with embedded **tags**. The editor shows tags as chips; this document defines the stored form. The parser lives in `KudzuCore/Templates`.

## Tags

```
{{name}}
{{name key="value" key2=value2}}
```

- `name`: lowercase identifier `[a-z][a-z0-9_]*`.
- Attribute values are either double-quoted strings or bare tokens `[A-Za-z0-9_.+\-]+`.
  - Inside quotes, `\"` is a literal quote and `\\` a literal backslash.
  - Bare `true` / `false` are booleans and bare numbers are numbers; everything else is a string.
- Whitespace around `=` and between attributes is allowed.
- Any text outside a tag is literal. `\{{` produces a literal `{{`.
- Unknown tag names and malformed tags are **not** errors at render time. They render verbatim, and the parser reports a diagnostic with a source range so the editor can underline them.

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

```swift
TemplateRenderer.render(template, context) -> RenderResult
// RenderResult: text: String, cursorOffset: Int? (in UTF-16 units from the start of `text`)
```

`context` supplies `now: Date`, `clipboard: String?`, `formValues: [String: String]`, `resolveSnippet: @Sendable (UUID) -> String?`, and `locale`/`timeZone`/`calendar`. The renderer is pure and deterministic given a context.

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
- Canonical serialization preserves semantic node content, attribute types and order. Ranges are recomputed on parsing because canonical whitespace, quoting and escaping can change source lengths. Canonical inputs serialize byte-identically. All string attributes are quoted; booleans and numbers are bare.
- Outside tags, `\\` represents a literal backslash, allowing literal text ending in a backslash immediately before a tag to round-trip. Other backslashes are literal except for `\{{`. The serializer escapes both backslashes and literal opening delimiters.
- Inside quotes, `\|` is also accepted and retained as an escaped pipe for `options` splitting. Empty interior/trailing options are preserved; absent or empty options produce an empty list.
- Duplicate attributes retain their last occurrence (and its position) and produce a warning. Unknown attributes are retained for forward compatibility. Known attributes with incorrect types make the tag literal with a diagnostic, except invalid offsets, which are ignored with a warning. Required names must be strings (an empty quoted name is accepted); snippet IDs must be valid UUID strings.
- Invalid date offsets produce a warning and are ignored. One leading sign applies to every component; components are applied in written order using the supplied calendar and time zone. Month and year arithmetic clamps to valid calendar dates.
- Static analysis examines only the supplied source. Form names and referenced snippet IDs are deduplicated in first-appearance order. The first definition determines a form control's kind, options and default; rendering still uses each occurrence's own default when no form value is supplied.
