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
  - Attributes that take a **whole number** (`count`, `ms`, `history`) must be bare numbers: `count="2"` or `count=two` is malformed. A fraction is dropped and a number outside the attribute's range is moved to the nearest end of it, each with a warning.
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
| `clipboard` | `trim` (bool, default `false`): trims whitespace and newlines at both ends. `history` (whole number 0–99, default `0`): how many copies back to read | Plain-text clipboard contents, or empty. With `history=1` it is the copy before the current one, `history=2` the one before that, and so on, read from Kudzu's clipboard history; empty when there is no such item, when it isn't text, or while clipboard history is turned off |
| `selection` | `trim` (bool, default `false`) | The text selected in the target app when the snippet is inserted from Quick Search. Empty for a typed trigger, because typing the trigger replaces the selection |
| `uuid` | `upper` (bool, default `false`): capital letters | A new random UUID (version 4) on every expansion; each tag gets its own |
| `random` | `options` (pipe-separated, `\|` escapes a pipe, as in `choice`) | One of the options, picked at random on every expansion. Empty options are kept as options; no options renders nothing |
| `key` | `name` (required): `tab`, `return`, `escape`, `space`, `backspace`, `delete` (forward delete), `left`, `right`, `up`, `down`; `enter` and `esc` are accepted spellings of `return` and `escape`. `count` (whole number 1–50, default `1`) | No text. Presses the key `count` times at this point of the insertion. See [Key presses and pauses](#key-presses-and-pauses) |
| `delay` | `ms` (required, whole number 1–5000) | No text. Pauses the insertion for that many milliseconds. One expansion pauses for at most 10 seconds in total; a delay past that is cut short with a diagnostic |
| `cursor` | — | Nothing; marks where the caret lands after insertion. Only the first `cursor` counts; later ones are diagnostics. |
| `snippet` | `id` (UUID, required) | The referenced snippet's template source, rendered recursively. Cycles are diagnostics and render empty. Max depth 8. |
| `field` | `name` (required), `default`, `multiline` (bool) | A form text field. Each `name` is one field; repeating a name reuses its value. |
| `choice` | `name` (required), `options` (pipe-separated, `\|` escapes a pipe), `default` | A form dropdown |

Any `field` or `choice` makes the snippet a **form snippet**: the engine shows the form panel before inserting.

A build that predates a tag renders it verbatim and saves it unchanged (see above), so `selection`, `uuid`, `random`, `key` and `delay` are safe to sync to an older Kudzu: it types the tag's source instead of acting on it. `history` on `clipboard` is an unknown attribute there, so an older build inserts the current clipboard.

## Key presses and pauses

`{{key}}` and `{{delay}}` turn an expansion into an ordered sequence of text, key presses and pauses. The usual use is filling a form with several fields:

```
{{field name="Email"}}{{key name="tab"}}{{field name="Company"}}{{key name="tab" count=2}}{{delay ms=300}}{{key name="return"}}
```

The rules are deliberately simple:

- **Order.** Steps run in template order, one at a time. Tags inside a nested `{{snippet}}` run in place, as if they were written where the reference is.
- **Text between steps** is delivered piece by piece, each piece by the same rule as a whole expansion: short text is typed, long or multi-line text is pasted, and the clipboard a paste replaced is restored before the next step. `{{clipboard}}` is read once, before the first step, so it never sees a pasted piece.
- **Formatted snippets.** Each piece carries the formatting that falls inside it. A style that spans a key press is applied to both sides; an image that a key press would cut in two is left as its text alternative.
- **Cursor.** The caret is placed with arrow keys after the last step, which only works inside the text that follows the last key press. A `{{cursor}}` after the last `{{key}}` works as usual. A `{{key}}` anywhere after the cursor cancels it (a diagnostic says so), and a later `{{cursor}}` does not take its place: only the first one ever counts. A `{{delay}}` does not affect the cursor.
- **Undo.** An expansion that pressed a key can't be undone with Backspace, because its text may be spread over several fields. An expansion with pauses only still can.
- **Whole-word triggers.** The space or punctuation that completed the trigger is not typed again after a sequence that pressed a key. It still is after one with pauses only.
- **Stopping.** The sequence stops, leaving what it already inserted, if you click, if another app comes to the front, or if expansion is paused or blocked by Secure Input. While a sequence runs, what you type is held and replayed after it, as during any expansion.
- **Trigger case.** Case propagation applies to the text pieces only.
- **Where there is nothing to press keys in** (copying a snippet, previews, the MCP `expand_snippet` tool) the text is produced without the key presses and pauses. Previews show them as keycaps; `expand_snippet` lists them in `skippedActions`.

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
// RenderResult { text: String, formatting: Option<Vec<FormatSpan>>, cursor_offset: Option<usize> /* UTF-16 units */, diagnostics, actions: Vec<RenderAction> }
```

`RenderContext` supplies `now` (with its UTC offset), `locale`, `clipboard: Option<String>`, `form_values`, `resolve_snippet: &dyn Fn(Uuid) -> Option<String>`, an optional `resolve_formatting` resolver, and optionally `root_snippet_id: Option<Uuid>`. It also supplies everything that isn't a function of the template: `selection: Option<String>`, `clipboard_history: Option<&dyn Fn(usize) -> Option<String>>` (1 = the copy before the current clipboard), `new_uuid: Option<&dyn Fn() -> Uuid>` and `pick: Option<&dyn Fn(usize) -> usize>` (an index below the given option count; the renderer takes it modulo the count). Each has a deterministic fallback when absent: an empty selection, empty history items, the nil UUID and the first option. The app passes `template::random_uuid` and `template::random_index`; tests pass seeded sources.

`text` never contains key presses or pauses. They are in `actions`, in insertion order, each as `{ offset, kind: "key", key, count }` or `{ offset, kind: "pause", ms }`, where `offset` is the UTF-16 length of the text inserted before it. Several actions can share an offset. The field is omitted from JSON when empty. Splitting `text` at the offsets gives the sequence the engine executes. `cursor_offset` is `None` whenever a key press follows the cursor. When the root ID is set, it starts on the active reference path, so a snippet that includes itself is a cycle on the first reference instead of expanding once. The renderer is pure and deterministic given a context. `render_formatted` accepts the root's optional formatting metadata; `render` always produces plain text.

## Espanso mapping (import)

| Espanso | Kudzu |
|---|---|
| `{{mydate}}` with a `type: date` var, `params.format` (strftime) | `{{date format="<converted ICU>"}}`; strftime is converted to ICU |
| `type: clipboard` | `{{clipboard}}` |
| `$|$` | `{{cursor}}` |
| `type: match` (nested, by trigger) | `{{snippet id="<resolved id>"}}` |
| `type: form` / `[[field]]` in `form:` | `{{field name="field"}}` or `{{choice …}}` for `list`/`choice` fields |
| `type: random` with `params.choices` | `{{random options="a\|b"}}`. Without choices it is kept as text with a warning |
| `type: echo` | The `params.echo` value as literal text |
| `type: shell` / `script` and others | Imported as literal text with an import warning |
| `word: true` | `fireMode: wholeWord` |
| `propagate_case: true` | `caseMode: propagate` (otherwise `exact`) |
| `replace` | The plain-text template |
| `markdown` | A Formatted snippet: `**bold**`, `*italic*`/`_italic_`, `[text](url)`, `<url>`, inline `<b>`/`<i>`/`<u>`/`<br>`, `-`/`*`/`+` items as `• ` and `1.` items as written. Line breaks are kept as written. Headings become bold; block quote markers, code spans and fences, `~~strikethrough~~`, rules, nested-list indentation and tables (cells joined by tabs) keep their text but lose their styling |
| `html` | A Formatted snippet: `b`/`strong`, `i`/`em`, `u`, `a href`, inline `style` weight, italic and underline, `br`, `p` (blank line), block elements (line break), `ul`/`ol` items as `• `/`1. `, table cells joined by tabs. Whitespace collapses as in a browser (not in `pre`); entities are decoded; `script`, `style` and `head` are skipped. Fonts, sizes, colors, highlights, strikethrough, super/subscript, alignment and other elements are dropped |
| `markdown` / `html` images | Embedded when the source is a `data:` URL holding a PNG or JPEG of at most 512 pixels a side, 144 KiB per image and 384 KiB per snippet; the alternative text becomes `[alt]`. Other images keep only `[alt]` |
| `markdown` / `html` links | HTTP, HTTPS and mailto only; other links keep their text |

Rich matches keep their Espanso variables: `**{{today}}**` formats the whole converted tag, and literal `{{` is escaped as usual. A match whose formatting is dropped in part gets one warning naming what was dropped. Markdown or HTML over 1 MiB, or nested more than 256 elements deep, is imported as plain text with a warning.

## Raycast mapping (import)

Raycast's **Export Snippets** writes a JSON array of `{ "name", "text", "keyword" }`. Kudzu detects it by the leading `[`. All snippets land in one **Raycast** group with `caseMode: exact` and `fireMode: immediate`.

| Raycast | Kudzu |
|---|---|
| `{clipboard}` | `{{clipboard}}`; `{clipboard \| trim}` adds `trim=true` |
| `{clipboard offset=1}` (history) | `{{clipboard history=1}}`, for offsets 1 to 99; `\| trim` still adds `trim=true`. Any other offset is kept as text with a warning |
| `{cursor}` | `{{cursor}}` |
| `{date}`, `{time}`, `{datetime}`, `{day}` | `{{date format="…"}}`. Raycast's defaults follow the system's settings, so Kudzu uses fixed approximations: `d MMM yyyy`, `HH:mm`, `d MMM yyyy 'at' HH:mm`, `EEEE` |
| `{date format="…" offset="…" locale="fr-FR"}` | `format` is already ICU; `locale` becomes `fr_FR`. Offset units: `m` (minutes) → `min`, `M` (months) → `m`; `h`, `d`, `w`, `y` are unchanged, and parts may be separated by spaces (`+2y +5M`). An offset Kudzu can't express, such as mixed signs, is dropped with a warning |
| `{argument name="n" default="d"}` | `{{field name="n" default="d"}}`; unnamed arguments become `argument 1`, `argument 2`, … |
| `{argument name="n" options="a, b"}` | `{{choice name="n" options="a\|b"}}` |
| `{snippet name="Other"}` | `{{snippet id="…"}}` when a snippet with that name is in the export, otherwise kept as text with a warning |
| `{selection}` | `{{selection}}`. It has a value when the snippet is inserted from Quick Search, not from a typed keyword |
| `{uuid}` | `{{uuid upper=true}}` |
| other modifiers | Ignored with a warning |
| `\{` before a placeholder | A literal `{`; anywhere else the backslash stays |
| a snippet without a `keyword` | Imported turned off, with a warning |

## TextExpander mapping (import)

Kudzu reads TextExpander's XML property list (`.textexpander`, the `snippetsTE2` array) and CSV exports. Binary property lists and compressed files are rejected with a message to export CSV instead. A `.textexpander` file becomes one group named after its `groupName`; a CSV becomes one group per `Group` column value, or one group named after the file. CSV columns are matched by header (`Abbreviation`, `Snippet` or `Content`, `Label`, `Group`); without a header they are abbreviation, content, label. Line breaks are normalized to `\n`.

| TextExpander | Kudzu |
|---|---|
| `abbreviationMode` `0` (case sensitive) | `caseMode: exact`; anything else `propagate` |
| group expansion mode | Not read; every snippet gets `fireMode: immediate` |
| rich text snippet (`snippetType` `1`) | A Formatted snippet, read from the Cocoa RTF in `extraInfo.RTF`: bold, italic, underline, `HYPERLINK` fields (HTTP, HTTPS and mailto), line breaks, list markers (`• `, `1. `), Unicode and Windows-1252 characters, and `\pict` PNG/JPEG pictures within the image limits. Fonts and sizes are dropped with a warning only when they vary within the snippet (a bold face of the same family doesn't count); colors, highlights, strikethrough, super/subscript, alignment, tables and attachments (`\NeXTGraphic`, which point at files outside the export) are dropped and named in the warning |
| rich text snippet without `extraInfo.RTF` | Imported as plain text from `plainText`, with a warning. The other copy, `richText`, is an `NSArchiver` archive of an attributed string that Kudzu doesn't decode |
| rich text snippet whose RTF is malformed, nested more than 256 groups deep, over 4 MiB, or whose letters and digits differ from `plainText` | Imported as plain text from `plainText`, with a warning |
| AppleScript, JavaScript and shell snippets | Skipped with a warning |
| `%|` | `{{cursor}}` |
| `%clipboard` | `{{clipboard}}` |
| `%Y %y %m %d %e %H %I %M %S %p %A %a %B %b %j %Z %z`, `%1m %1d %1H %1I` | `{{date format="…"}}`; a run such as `%m/%d/%Y` becomes one tag |
| `%filltext:` / `%fillarea:` | `{{field name="…" default="…"}}` (`fillarea` adds `multiline=true`) |
| `%fillpart:…%text%fillpartend%` (optional section) | The text, always included, with a warning |
| `%fillpopup:name=n:a:default=b%` | `{{choice name="n" options="b\|a" default="b"}}` |
| `%snippet:abbreviation%` | `{{snippet id="…"}}` when the abbreviation is in the export, otherwise kept as text with a warning |
| `%@+1D` before date macros | The `offset` of the date tag (`Y`/`M`/`W`/`D`/`h`/`m` → `y`/`m`/`w`/`d`/`h`/`min`) |
| `%key:tab%`, `%key:enter%`, `%key:return%`, `%key:esc%` | `{{key name="tab"}}`, `{{key name="return"}}` (for both Enter and Return), `{{key name="escape"}}` |
| other `%key:…%`, `%calc:`, `%mouse:` | Kept as text with a warning |

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
- `key` without a known `name`, and `delay` without `ms`, are unrecognized tags (kept verbatim) with a diagnostic. A key name is written back as it was typed, so `name="esc"` is not rewritten to `escape`.
- Static analysis reports `usesSelection` and `hasKeyPresses` next to `usesClipboard` and `hasCursor`. Any `clipboard` tag, with or without `history`, counts as using the clipboard.
- Static analysis examines only the supplied source. Form names and referenced snippet IDs are deduplicated in first-appearance order. The first definition determines a form control's kind, options and default; rendering still uses each occurrence's own default when no form value is supplied.
