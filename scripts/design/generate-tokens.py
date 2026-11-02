#!/usr/bin/env python3
"""Generate KudzuUI's Swift design tokens from the design system's tokens.json.

Usage (from the repo root):

    python3 scripts/design/generate-tokens.py           # write the Swift file
    python3 scripts/design/generate-tokens.py --check   # exit 1 if the file is stale

Standard library only. Output is deterministic: the same tokens.json and the same
version of this script always produce byte-identical Swift.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
SOURCE = REPO / "design/kudzu/project/tokens.json"
OUTPUT = REPO / "Packages/KudzuUI/Sources/KudzuUI/Tokens/Tokens.generated.swift"
SCRIPT = Path(__file__).resolve()

# Increase Contrast is not specified by the design system. These cheap substitutions
# strengthen hairlines and faint text when macOS asks for more contrast. Each entry
# means "under Increase Contrast, use this other token's value instead".
HIGH_CONTRAST_SUBSTITUTES = {
    "line": "line-strong",
    "line-strong": "edge",
    "ink-faint": "ink-muted",
}
# Under Increase Contrast these translucent tokens become fully opaque.
HIGH_CONTRAST_OPAQUE = {"glass", "glass-strong"}

SWIFT_KEYWORDS = {"default", "case", "class", "struct", "enum", "func", "let", "var", "in", "is", "as"}


def camel(name: str) -> str:
    """kebab-case → lowerCamelCase (`surface-raised` → `surfaceRaised`)."""
    head, *rest = name.split("-")
    out = head + "".join(p[:1].upper() + p[1:] for p in rest)
    return f"`{out}`" if out in SWIFT_KEYWORDS else out


def swift_string(text: str) -> str:
    return '"' + text.replace("\\", "\\\\").replace('"', '\\"') + '"'


def doc(text: str, indent: str) -> str:
    """One `///` doc line from a usage note (kept on one line, as the repo convention asks)."""
    flat = " ".join(text.split())
    return f"{indent}/// {flat}\n"


def num(value: float) -> str:
    """Stable, short number formatting for Swift literals."""
    if float(value).is_integer():
        return str(int(value))
    return repr(round(float(value), 6))


def px(value: str) -> float:
    m = re.fullmatch(r"\s*(-?[\d.]+)px\s*", value)
    if not m:
        raise ValueError(f"expected a px value, got {value!r}")
    return float(m.group(1))


def em(value: str | None) -> float:
    if value is None:
        return 0.0
    m = re.fullmatch(r"\s*(-?[\d.]+)em\s*", value)
    if not m:
        raise ValueError(f"expected an em value, got {value!r}")
    return float(m.group(1))


# ---------- colours ----------

def parse_hex(value: str) -> tuple[float, float, float, float]:
    m = re.fullmatch(r"#([0-9a-fA-F]{6})([0-9a-fA-F]{2})?", value.strip())
    if not m:
        raise ValueError(f"unsupported colour {value!r} (expected #rrggbb or #rrggbbaa)")
    rgb = m.group(1)
    r, g, b = (int(rgb[i:i + 2], 16) for i in (0, 2, 4))
    a = int(m.group(2), 16) if m.group(2) else 255
    return (r / 255, g / 255, b / 255, a / 255)


def resolve_colors(tokens: list[dict]) -> dict[str, dict[str, tuple]]:
    raw = {t["name"]: t["value"] for t in tokens}
    resolved: dict[str, dict[str, tuple]] = {}

    def resolve(name: str, theme: str, seen: tuple[str, ...]) -> tuple:
        if name in seen:
            raise ValueError(f"alias cycle: {' → '.join(seen + (name,))}")
        if name not in raw:
            raise ValueError(f"unknown colour alias {{{name}}}")
        value = raw[name][theme]
        alias = re.fullmatch(r"\{([a-z0-9-]+)\}", value.strip())
        if alias:
            return resolve(alias.group(1), theme, seen + (name,))
        return parse_hex(value)

    for name in raw:
        resolved[name] = {theme: resolve(name, theme, ()) for theme in ("light", "dark")}
    return resolved


def rgba_literal(c: tuple) -> str:
    r, g, b, a = (round(x * 255) for x in c)
    return f"KzRGBA(0x{r:02X}, 0x{g:02X}, 0x{b:02X}, alpha: 0x{a:02X})"


def alias_of(value: dict) -> str | None:
    m = re.fullmatch(r"\{([a-z0-9-]+)\}", value["light"].strip())
    return m.group(1) if m and value["light"] == value["dark"] else None


def gen_colors(data: dict) -> str:
    tokens = data["color"]["tokens"]
    resolved = resolve_colors(tokens)
    out = []
    out.append("// MARK: - Colour\n\n")
    out.append("/// Every colour token in the design system, with its Light and Dark values and usage note.\n")
    out.append("public enum KzColorToken: String, CaseIterable, Sendable {\n")
    for t in tokens:
        out.append(doc(t["usage"], "    "))
        out.append(f"    case {camel(t['name'])} = {swift_string(t['name'])}\n")
    out.append("\n    /// The Light theme value, with aliases resolved.\n")
    out.append("    public var light: KzRGBA {\n        switch self {\n")
    for t in tokens:
        out.append(f"        case .{camel(t['name'])}: {rgba_literal(resolved[t['name']]['light'])}\n")
    out.append("        }\n    }\n")
    out.append("\n    /// The Dark theme value, with aliases resolved.\n")
    out.append("    public var dark: KzRGBA {\n        switch self {\n")
    for t in tokens:
        out.append(f"        case .{camel(t['name'])}: {rgba_literal(resolved[t['name']]['dark'])}\n")
    out.append("        }\n    }\n")
    out.append("\n    /// The token this one is an alias of in tokens.json (`focus` → `vineInk`), if any.\n")
    out.append("    public var aliasOf: KzColorToken? {\n        switch self {\n")
    aliased = [(t["name"], alias_of(t["value"])) for t in tokens]
    for name, target in aliased:
        if target:
            out.append(f"        case .{camel(name)}: .{camel(target)}\n")
    out.append("        default: nil\n        }\n    }\n")
    out.append("\n    /// The token whose value replaces this one under Increase Contrast (not in tokens.json; see the generator).\n")
    out.append("    public var highContrastSubstitute: KzColorToken? {\n        switch self {\n")
    for src, dst in sorted(HIGH_CONTRAST_SUBSTITUTES.items()):
        out.append(f"        case .{camel(src)}: .{camel(dst)}\n")
    out.append("        default: nil\n        }\n    }\n")
    out.append("\n    /// Whether this token becomes fully opaque under Increase Contrast.\n")
    out.append("    public var isOpaqueInHighContrast: Bool {\n        switch self {\n")
    out.append("        case " + ", ".join(f".{camel(n)}" for n in sorted(HIGH_CONTRAST_OPAQUE)) + ": true\n")
    out.append("        default: false\n        }\n    }\n")
    out.append("\n    /// How the design system says this colour should be used.\n")
    out.append("    public var usage: String {\n        switch self {\n")
    for t in tokens:
        out.append(f"        case .{camel(t['name'])}: {swift_string(' '.join(t['usage'].split()))}\n")
    out.append("        }\n    }\n}\n\n")

    out.append("/// Kudzu's colours as dynamic SwiftUI colours that follow Light, Dark and Increase Contrast.\n")
    out.append("public enum KzColor {\n")
    for t in tokens:
        out.append(doc(t["usage"], "    "))
        out.append(f"    public static let {camel(t['name'])}: Color = KzColorToken.{camel(t['name'])}.color\n")
    out.append("}\n\n")
    return "".join(out)


# ---------- type ----------

def gen_type(data: dict) -> str:
    families = data["type"]["families"]
    styles = []
    for group in data["type"]["groups"]:
        for s in group["styles"]:
            family = s.get("family", group["family"])
            if family not in families:
                raise ValueError(f"unknown family {family!r}")
            styles.append((group["name"], family, s))
    out = ["// MARK: - Type\n\n"]
    out.append("/// Every type style in the design system. Apply one with `.kzTextStyle(_:)`.\n")
    out.append("public enum KzTextStyle: String, CaseIterable, Sendable {\n")
    for _, _, s in styles:
        out.append(doc(s["usage"], "    "))
        out.append(f"    case {camel(s['name'])} = {swift_string(s['name'])}\n")
    out.append("\n    /// Size, line height, weight, tracking and family, straight from tokens.json.\n")
    out.append("    public var spec: KzTextSpec {\n        switch self {\n")
    for group, family, s in styles:
        tabular = "true" if "tabular" in s["usage"].lower() else "false"
        out.append(
            f"        case .{camel(s['name'])}: KzTextSpec("
            f"family: .{family}, size: {num(px(s['fontSize']))}, lineHeight: {num(px(s['lineHeight']))}, "
            f"weight: {int(s['fontWeight'])}, trackingEm: {num(em(s.get('letterSpacing')))}, "
            f"tabularFigures: {tabular}, group: {swift_string(group)}, sample: {swift_string(s['sample'])})\n"
        )
    out.append("        }\n    }\n")
    out.append("\n    /// How the design system says this style should be used.\n")
    out.append("    public var usage: String {\n        switch self {\n")
    for _, _, s in styles:
        out.append(f"        case .{camel(s['name'])}: {swift_string(' '.join(s['usage'].split()))}\n")
    out.append("        }\n    }\n}\n\n")
    return "".join(out)


# ---------- metrics (spacing, radius, size, blur) ----------

def gen_metric(enum: str, title: str, tokens: list[dict], prefix: str, rename=None) -> str:
    out = [f"// MARK: - {title}\n\n"]
    out.append(f"/// {title} tokens from the design system, in points.\n")
    out.append(f"public enum {enum} {{\n")
    names = []
    for t in tokens:
        name = t["name"]
        if not name.startswith(prefix):
            raise ValueError(f"{name!r} lacks prefix {prefix!r}")
        short = rename(name[len(prefix):]) if rename else camel(name[len(prefix):])
        names.append((short, t))
        out.append(doc(t["usage"], "    "))
        out.append(f"    public static let {short}: CGFloat = {num(px(t['value']))}\n")
    out.append("\n    /// Every token in this group, in design-system order, for galleries and docs.\n")
    out.append("    public static let all: [KzMetric] = [\n")
    for short, t in names:
        out.append(
            f"        KzMetric(token: {swift_string(t['name'])}, name: {swift_string(short)}, "
            f"value: {num(px(t['value']))}, usage: {swift_string(' '.join(t['usage'].split()))}),\n"
        )
    out.append("    ]\n}\n\n")
    return "".join(out)


# ---------- shadows ----------

RGBA_RE = re.compile(r"rgba?\(\s*([\d.]+)\s*,\s*([\d.]+)\s*,\s*([\d.]+)\s*(?:,\s*([\d.]+)\s*)?\)")


def split_shadow_list(value: str) -> list[str]:
    parts, depth, cur = [], 0, ""
    for ch in value:
        if ch == "(":
            depth += 1
        elif ch == ")":
            depth -= 1
        if ch == "," and depth == 0:
            parts.append(cur.strip())
            cur = ""
        else:
            cur += ch
    if cur.strip():
        parts.append(cur.strip())
    return parts


def parse_shadow(layer: str) -> dict:
    m = RGBA_RE.search(layer)
    if not m:
        raise ValueError(f"shadow layer without rgba(): {layer!r}")
    color = (float(m.group(1)) / 255, float(m.group(2)) / 255, float(m.group(3)) / 255,
             float(m.group(4)) if m.group(4) is not None else 1.0)
    rest = (layer[:m.start()] + layer[m.end():]).split()
    inset = "inset" in rest
    lengths = [r for r in rest if r != "inset"]
    nums = []
    for v in lengths:
        mm = re.fullmatch(r"(-?[\d.]+)(px)?", v)
        if not mm:
            raise ValueError(f"bad shadow length {v!r} in {layer!r}")
        nums.append(float(mm.group(1)))
    while len(nums) < 4:
        nums.append(0.0)
    x, y, blur, spread = nums[:4]
    return {"inset": inset, "x": x, "y": y, "blur": blur, "spread": spread, "color": color}


def shadow_rgba_literal(c: tuple) -> str:
    r, g, b, a = c
    return f"KzRGBA({num(round(r * 255))} / 255, {num(round(g * 255))} / 255, {num(round(b * 255))} / 255, {num(a)})"


def gen_shadows(data: dict) -> str:
    tokens = data["shadow"]["tokens"]
    out = ["// MARK: - Shadow\n\n"]
    out.append("/// Every shadow token in the design system. Apply one with `.kzShadow(_:in:)`.\n")
    out.append("public enum KzShadow: String, CaseIterable, Sendable {\n")
    for t in tokens:
        out.append(doc(t["usage"], "    "))
        out.append(f"    case {camel(t['name'][len('shadow-'):])} = {swift_string(t['name'])}\n")
    for theme in ("light", "dark"):
        out.append(f"\n    /// The CSS shadow list for the {theme.capitalize()} theme, parsed into layers (first layer on top).\n")
        out.append(f"    public var {theme}Layers: [KzShadowLayer] {{\n        switch self {{\n")
        for t in tokens:
            layers = [parse_shadow(p) for p in split_shadow_list(t["value"][theme])]
            out.append(f"        case .{camel(t['name'][len('shadow-'):])}: [\n")
            for l in layers:
                out.append(
                    f"            KzShadowLayer(inset: {'true' if l['inset'] else 'false'}, x: {num(l['x'])}, y: {num(l['y'])}, "
                    f"blur: {num(l['blur'])}, spread: {num(l['spread'])}, color: {shadow_rgba_literal(l['color'])}),\n"
                )
            out.append("        ]\n")
        out.append("        }\n    }\n")
    out.append("\n    /// How the design system says this shadow should be used.\n")
    out.append("    public var usage: String {\n        switch self {\n")
    for t in tokens:
        out.append(f"        case .{camel(t['name'][len('shadow-'):])}: {swift_string(' '.join(t['usage'].split()))}\n")
    out.append("        }\n    }\n}\n\n")
    return "".join(out)


# ---------- driver ----------

def fingerprint() -> str:
    h = hashlib.sha256()
    h.update(SOURCE.read_bytes())
    h.update(b"\0")
    h.update(SCRIPT.read_bytes())
    return h.hexdigest()


def generate() -> str:
    data = json.loads(SOURCE.read_text(encoding="utf-8"))
    out = []
    out.append("// GENERATED FILE — do not edit.\n")
    out.append("// Source: design/kudzu/project/tokens.json\n")
    out.append("// Regenerate: python3 scripts/design/generate-tokens.py\n")
    out.append("// swiftlint:disable all\n\n")
    out.append("import CoreGraphics\nimport SwiftUI\n\n")
    out.append("/// Metadata about the generated tokens.\n")
    out.append("public enum KzTokens {\n")
    out.append("    /// SHA-256 of tokens.json plus the generator script; a test compares it to catch a stale file.\n")
    out.append(f"    public static let sourceFingerprint = {swift_string(fingerprint())}\n")
    out.append(f"    /// The design system's name and version from tokens.json.\n")
    out.append(f"    public static let designSystem = {swift_string(data['name'] + ' v' + str(data['version']))}\n")
    out.append("}\n\n")
    out.append(gen_colors(data))
    out.append(gen_type(data))
    out.append(gen_metric("KzSpace", "Spacing", data["spacing"]["tokens"], "space-", rename=lambda s: "s" + s))
    out.append(gen_metric("KzRadius", "Radius", data["radius"]["tokens"], "radius-"))
    out.append(gen_shadows(data))
    out.append(gen_metric("KzSize", "Size", data["size"]["tokens"], "size-"))
    out.append(gen_metric("KzBlur", "Blur", data["blur"]["tokens"], "blur-"))
    return "".join(out).rstrip("\n") + "\n"


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--check", action="store_true", help="exit 1 if the generated file is out of date")
    args = parser.parse_args()
    swift = generate()
    if args.check:
        current = OUTPUT.read_text(encoding="utf-8") if OUTPUT.exists() else ""
        if current != swift:
            print(f"{OUTPUT.relative_to(REPO)} is stale; run python3 scripts/design/generate-tokens.py", file=sys.stderr)
            return 1
        print("tokens up to date")
        return 0
    OUTPUT.parent.mkdir(parents=True, exist_ok=True)
    OUTPUT.write_text(swift, encoding="utf-8")
    print(f"wrote {OUTPUT.relative_to(REPO)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
