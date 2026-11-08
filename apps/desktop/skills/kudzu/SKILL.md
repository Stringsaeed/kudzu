---
name: kudzu
description: Use the user's Kudzu text-expander snippets and clipboard history through the Kudzu MCP tools. Use when the user mentions snippets, text expansion, triggers like ";sig", Kudzu, reusable replies or boilerplate, or something they copied earlier; when they ask to save text as a snippet; or when a task needs text they keep in Kudzu (signature, address, standard replies).
---

# Kudzu

Kudzu is the user's text expander and clipboard manager on macOS. A snippet expands when the user types its trigger (for example `;sig`) in any app. The Kudzu MCP server (`list_snippets`, `get_snippet`, `expand_snippet`, `list_groups`, `create_snippet`, `update_snippet`, `list_clipboard`, `get_clipboard_item`) reads and writes the user's library on this Mac.

If the tools answer that Kudzu isn't running or access is off, tell the user to open Kudzu and turn on **Settings ▸ AI agents ▸ Allow AI agents**. Don't work around it by reading Kudzu's files.

## Using snippets

1. Look before you write. `list_snippets` with a `query` searches names, triggers and content. Prefer the user's own wording (signature, address, standard replies) over writing new text.
2. Use `expand_snippet` for final text. It fills in dates, nested snippets and the clipboard as if the user typed the trigger now. Pass fill-in fields in `values`; `fields` in the result lists them, with options for choices.
3. Use `get_snippet` to see or edit the raw template.

## Writing snippets

Create or change snippets only when the user asks. Confirm first before writing more than a couple at a time.

- **Triggers**: short, memorable, and start with the prefix from `list_snippets` (`triggerPrefix`, usually `;`), such as `;addr` or `;thx`. Kudzu rejects a trigger that's already in use. If a warning says a shorter trigger fires first, choose another trigger or use `fireMode: "wholeWord"`.
- **Name**: a few words that say what the text is for.
- **Group**: an existing group from `list_groups`, or a new path like `Work/Support`. Missing groups are created.
- **Editing**: `update_snippet` changes only the fields you pass. Formatted snippets (bold, links or images) and snippets that hold a secret can only have their content edited in Kudzu.

### Template syntax

Content is literal text plus tags:

- `{{date}}`, `{{date format="EEEE, MMMM d"}}`, `{{date offset="+1w"}}`: today's date (ICU pattern, default `yyyy-MM-dd`), optionally shifted by `d`, `w`, `m`, `y`, `h` or `min`.
- `{{time}}`, `{{time format="h:mm a"}}`: the current time (default `HH:mm`).
- `{{clipboard}}`, `{{clipboard trim=true}}`: what's on the clipboard.
- `{{cursor}}`: where the caret goes after expanding.
- `{{field name="Name" default="there"}}`: a fill-in field the user completes when expanding. Add `multiline=true` for long text.
- `{{choice name="Tone" options="Formal|Casual" default="Casual"}}`: a drop-down. Options are separated by `|`.
- `{{snippet id="<uuid>"}}`: another snippet's content, by id.

Write `\{{` for a literal `{{`. A repeated field name reuses the same value. Unknown tags are kept as typed and shown as errors in Kudzu, so check the `warnings` in a write result.

## Clipboard history

`list_clipboard` lists recent copies, newest first; filter with `query`, `kind` (`text`, `code`, `link`, `color`, `image`, `file`) or `pinnedOnly`. Rows are shortened, so use `get_clipboard_item` for the full text. These tools exist only when the user shares clipboard history.

## Secrets

Kudzu replaces anything that looks like a password, API key, token or private key with `[redacted: …]`, and withholds items that are entirely secret. This happens on the user's Mac, before the text reaches you.

- The original can't be retrieved through these tools. Don't ask the user to paste it, and don't try to reconstruct it.
- Never write a `[redacted: …]` marker into a snippet; Kudzu rejects it.
- If the user asks you to save a secret as a snippet, suggest a password manager instead.
