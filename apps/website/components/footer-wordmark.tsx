import { readFile } from "node:fs/promises";
import { join } from "node:path";
import type { CSSProperties } from "react";

/**
 * The Echo wordmark, inlined so its layers can move on scroll. The asset is the single source:
 * a main outline plus four echoes, each 7 units further down and right and a little fainter.
 * Each echo gets its distance from the main outline as --step (1 nearest, 4 farthest), which the
 * footer's scroll animation uses to fan them out from under the letters (globals.css).
 */
export async function FooterWordmark() {
  const svg = await readFile(join(process.cwd(), "public/assets/kudzu-wordmark.svg"), "utf8");
  const viewBox = svg.match(/viewBox="([^"]+)"/)?.[1] ?? "0 0 735 201";
  const layers = [...svg.matchAll(/<path\b([^>]*)\/>/g)].map(([, attributes]) => ({
    d: attributes.match(/\bd="([^"]+)"/)?.[1] ?? "",
    opacity: Number(attributes.match(/fill-opacity="([^"]+)"/)?.[1] ?? 1),
  }));
  // Fainter layers sit further back, so sorting by opacity puts the main outline last at step 0.
  const ordered = [...layers].sort((a, b) => a.opacity - b.opacity);
  return (
    <svg
      className="footer-wordmark wrap"
      viewBox={viewBox}
      fill="currentColor"
      aria-hidden="true"
      focusable="false"
    >
      {ordered.map((layer, index) => (
        <path
          key={layer.d.slice(0, 24)}
          className={layer.opacity < 1 ? "wordmark-echo" : undefined}
          fillRule="evenodd"
          fillOpacity={layer.opacity < 1 ? layer.opacity : undefined}
          d={layer.d}
          style={{ "--step": ordered.length - 1 - index } as CSSProperties}
        />
      ))}
    </svg>
  );
}
