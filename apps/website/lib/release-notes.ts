export type NoteSection = { title: string; items: string[] };

/** Reduces one release-please bullet to plain text, without its scope or commit link. */
function cleanItem(item: string) {
  const text = item
    .replace(/\s*\(\[[0-9a-f]{7,40}\]\([^)]*\)\)/g, "")
    .replace(/^\*\*[^*]+:\*\*\s*/, "")
    .replace(/\[([^\]]+)\]\([^)]*\)/g, "$1")
    .replace(/[*`]/g, "")
    .trim();
  return text.charAt(0).toUpperCase() + text.slice(1);
}

/** Parses release-please notes. Everything renders as text, so no release markup reaches the page. */
export function parseReleaseNotes(body: string): NoteSection[] {
  const sections: NoteSection[] = [];
  for (const raw of body.split(/\r?\n/)) {
    const line = raw.trim();
    const heading = line.match(/^#{2,4}\s+(.+)$/);
    if (heading) {
      if (!/^\[?v?\d+\.\d+/.test(heading[1])) sections.push({ title: heading[1], items: [] });
      continue;
    }
    const bullet = line.match(/^[*-]\s+(.+)$/);
    if (!bullet) continue;
    if (sections.length === 0) sections.push({ title: "Changes", items: [] });
    sections[sections.length - 1].items.push(cleanItem(bullet[1]));
  }
  return sections.filter((section) => section.items.length > 0);
}
