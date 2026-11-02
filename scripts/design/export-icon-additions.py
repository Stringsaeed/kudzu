#!/usr/bin/env python3
"""Export the icon glyphs Kudzu added to the design system as SVG files.

Reads the "Additions" cases from Packages/KudzuUI/Sources/KudzuUI/Icons/KzIconName.swift and
writes design/kudzu/additions/icons/<name>.svg in the design system's icon format (16-unit
viewBox, 1.5 stroke, round caps and joins, currentColor), so they can be added to `ICONS`.

Usage (from the repo root): python3 scripts/design/export-icon-additions.py
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
SWIFT = REPO / "Packages/KudzuUI/Sources/KudzuUI/Icons/KzIconName.swift"
OUT = REPO / "design/kudzu/additions/icons"


def kebab(swift_case: str, raw_values: dict[str, str]) -> str:
    return raw_values.get(swift_case, swift_case)


def main() -> int:
    source = SWIFT.read_text(encoding="utf-8")
    raw_values = dict(re.findall(r'case (\w+) = "([a-z-]+)"', source))
    marker = source.index("// Additions, same grid and weight.")
    block = source[marker:]
    rows = re.findall(r"case \.(\w+): \[(.*)\]\n", block)
    if not rows:
        print("no additions found", file=sys.stderr)
        return 1
    OUT.mkdir(parents=True, exist_ok=True)
    js_lines = []
    for case, body in rows:
        name = kebab(case, raw_values)
        elements, js_parts = [], []
        for kind, args in re.findall(r"\.(stroke|fill|circle)\(([^)]*)\)", body):
            if kind == "circle":
                x, y, r = (re.search(rf"{k}: ([\d.]+)", args).group(1) for k in ("x", "y", "r"))
                elements.append(f'  <circle cx="{x}" cy="{y}" r="{r}"/>')
                js_parts.append(f"'c:{x}:{y}:{r}'")
            else:
                d = re.fullmatch(r'"(.*)"', args.strip()).group(1)
                attrs = ' fill="currentColor" stroke="none"' if kind == "fill" else ""
                elements.append(f'  <path d="{d}"{attrs}/>')
                js_parts.append(f"'{'F:' if kind == 'fill' else ''}{d}'")
        svg = (
            '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16" width="16" height="16" '
            'fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">\n'
            + "\n".join(elements)
            + "\n</svg>\n"
        )
        (OUT / f"{name}.svg").write_text(svg, encoding="utf-8")
        key = f"'{name}'" if "-" in name else name
        js_lines.append(f"    {key}: [{', '.join(js_parts)}],")
    (OUT / "ICONS-additions.js.txt").write_text(
        "// Paste into the ICONS map in components/bundle.js.\n" + "\n".join(js_lines) + "\n", encoding="utf-8"
    )
    print(f"wrote {len(rows)} glyphs to {OUT.relative_to(REPO)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
