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
| `snippet` | `id` (UUID, required) | The referenced snippet's rendered content. Cycles are diagnostics and render empty. Max depth 8. |
| `field` | `name` (required), `default`, `multiline` (bool) | A form text field. Each `name` is one field; repeating a name reuses its value. |
| `choice` | `name` (required), `options` (pipe-separated, `\|` escapes a pipe), `default` | A form dropdown |

Any `field` or `choice` makes the snippet a **form snippet**: the engine shows the form panel before inserting.

## Rendering contract

```swift
TemplateRenderer.render(template, context) -> RenderResult
// RenderResult: text: String, cursorOffset: Int? (in UTF-16 units from the start of `text`)
```

`context` supplies `now: Date`, `clipboard: String?`, `formValues: [String: String]`, `resolveSnippet: (UUID) -> String?`, and `locale`/`timeZone`/`calendar`. The renderer is pure and deterministic given a context.

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
