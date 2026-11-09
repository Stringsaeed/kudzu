import Foundation

extension Agent {
    public static let INSTRUCTIONS = #"""
Kudzu is the user's text expander and clipboard manager on macOS. Snippets expand when the user types a trigger (like ";sig") in any app.

- Use list_snippets / get_snippet to find existing snippets before writing new ones, and reuse text the user already keeps there (signatures, addresses, boilerplate replies).
- Use expand_snippet to get a snippet's final text (dates filled in, fill-in fields from `values`).
- Create or edit snippets only when the user asks. Pick short, memorable triggers that start with the user's prefix and aren't already taken.
- Snippet content is a template: literal text plus tags such as {{date format="yyyy-MM-dd"}}, {{time}}, {{clipboard}}, {{cursor}}, {{field name="Name" default="…"}}, {{choice name="Tone" options="Formal|Casual"}} and {{snippet id="<uuid>"}}. Write \{{ for a literal {{.
- list_clipboard / get_clipboard_item search what the user copied recently (only when they've allowed it).
- The resource kudzu://skill/SKILL.md has fuller guidance on triggers, templates and groups.
- Text that looks like a password, API key, token or private key is replaced with "[redacted: …]". The original can't be retrieved through this server; don't ask for it or try to reconstruct it, and never write a redaction marker into a snippet.
"""#
}
