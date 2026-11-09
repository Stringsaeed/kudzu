# Kudzu template syntax

Snippet content is stored as plain text with embedded **tags**. The editor shows tags as chips; this document defines the stored form. The parser and renderer live in `apps/macos/Packages/KudzuKit/Sources/KudzuCore/Template/`.

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

### Formatting metadata

`content` always remains the plain-text template. A snippet can additionally store
`formatting: { source, spans }`. `source` is an exact copy of `content`; a mismatch disables
the formatting so an older client editing only `content` cannot apply stale ranges.
Omitting `formatting` means plain text; an empty `spans` array enables formatted output and
formatting inherited from nested snippets.

Each span contains UTF-16 `start` and `end` offsets and a tagged `style`:

```json
{ "start": 0, "end": 5, "style": { "kind": "bold" } }
```

Supported kinds are `bold`, `italic`, `underline`, `link` (with `url`), and `image`
(with a PNG/JPEG `dataUrl` and integer `width` from 1 to 512). Images cover their textual
alternative, such as `[Logo]`, in the source. The editor embeds compact PNG data; remote URLs
and SVG images are not accepted. Links accept only HTTP, HTTPS, and mailto URLs.

Formatting endpoints must be valid source boundaries; a variable chip is formatted as a whole.
Rendering maps those ranges onto the expanded values, including escaped literals, UTF-16,
and nested snippets. All text and attribute values are escaped when generating HTML.
The HTML clipboard flavor contains embedded images; the plain-text flavor retains alternatives.
`RenderResult.formatting` optionally carries spans in the **rendered text's** coordinate space.

### Text rendering

```rust
kudzu_core::template::render(source: &str, context: &RenderContext) -> RenderResult
// RenderResult { text: String, formatting: Option<Vec<FormatSpan>>, cursor_offset: Option<usize> /* UTF-16 units */, diagnostics }
```

`RenderContext` supplies `now` (with its UTC offset), `locale`, `clipboard: Option<String>`, `form_values`, `resolve_snippet: &dyn Fn(Uuid) -> Option<String>`, an optional `resolve_formatting` resolver, and optionally `root_snippet_id: Option<Uuid>`. When the root ID is set, it starts on the active reference path, so a snippet that includes itself is a cycle on the first reference instead of expanding once. The renderer is pure and deterministic given a context. `render_formatted` accepts the root's optional formatting metadata; `render` always produces plain text.

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
| `replace` / `markdown` / `html` | Imported as plain text; importing rich formatting emits a warning |

## Raycast mapping (import)

Raycast's **Export Snippets** writes a JSON array of `{ "name", "text", "keyword" }`. Kudzu detects it by the leading `[`. All snippets land in one **Raycast** group with `caseMode: exact` and `fireMode: immediate`.

| Raycast | Kudzu |
|---|---|
| `{clipboard}` | `{{clipboard}}`; `{clipboard \| trim}` adds `trim=true` |
| `{clipboard offset=1}` (history) | Kept as text with a warning |
| `{cursor}` | `{{cursor}}` |
| `{date}`, `{time}`, `{datetime}`, `{day}` | `{{date format="…"}}`. Raycast's defaults follow the system's settings, so Kudzu uses fixed approximations: `d MMM yyyy`, `HH:mm`, `d MMM yyyy 'at' HH:mm`, `EEEE` |
| `{date format="…" offset="…" locale="fr-FR"}` | `format` is already ICU; `locale` becomes `fr_FR`. Offset units: `m` (minutes) → `min`, `M` (months) → `m`; `h`, `d`, `w`, `y` are unchanged, and parts may be separated by spaces (`+2y +5M`). An offset Kudzu can't express, such as mixed signs, is dropped with a warning |
| `{argument name="n" default="d"}` | `{{field name="n" default="d"}}`; unnamed arguments become `argument 1`, `argument 2`, … |
| `{argument name="n" options="a, b"}` | `{{choice name="n" options="a\|b"}}` |
| `{snippet name="Other"}` | `{{snippet id="…"}}` when a snippet with that name is in the export, otherwise kept as text with a warning |
| `{selection}`, `{uuid}`, other modifiers | Kept as text with a warning |
| `\{` before a placeholder | A literal `{`; anywhere else the backslash stays |
| a snippet without a `keyword` | Imported turned off, with a warning |

## TextExpander mapping (import)

Kudzu reads TextExpander's XML property list (`.textexpander`, the `snippetsTE2` array) and CSV exports. Binary property lists and compressed files are rejected with a message to export CSV instead. A `.textexpander` file becomes one group named after its `groupName`; a CSV becomes one group per `Group` column value, or one group named after the file. CSV columns are matched by header (`Abbreviation`, `Snippet` or `Content`, `Label`, `Group`); without a header they are abbreviation, content, label. Line breaks are normalized to `\n`.

| TextExpander | Kudzu |
|---|---|
| `abbreviationMode` `0` (case sensitive) | `caseMode: exact`; anything else `propagate` |
| group expansion mode | Not read; every snippet gets `fireMode: immediate` |
| rich text snippet | Imported as plain text, with a warning |
| AppleScript, JavaScript and shell snippets | Skipped with a warning |
| `%|` | `{{cursor}}` |
| `%clipboard` | `{{clipboard}}` |
| `%Y %y %m %d %e %H %I %M %S %p %A %a %B %b %j %Z %z`, `%1m %1d %1H %1I` | `{{date format="…"}}`; a run such as `%m/%d/%Y` becomes one tag |
| `%filltext:` / `%fillarea:` | `{{field name="…" default="…"}}` (`fillarea` adds `multiline=true`) |
| `%fillpart:…%text%fillpartend%` (optional section) | The text, always included, with a warning |
| `%fillpopup:name=n:a:default=b%` | `{{choice name="n" options="b\|a" default="b"}}` |
| `%snippet:abbreviation%` | `{{snippet id="…"}}` when the abbreviation is in the export, otherwise kept as text with a warning |
| `%@+1D` before date macros | The `offset` of the date tag (`Y`/`M`/`W`/`D`/`h`/`m` → `y`/`m`/`w`/`d`/`h`/`min`) |
| `%key:`, `%calc:`, `%mouse:` | Kept as text with a warning |

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
