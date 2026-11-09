#!/usr/bin/env python3
"""Verify committed design exports without rendering anything (runs in CI and before releases).

design/assets-manifest.json records the SHA-256 of every source and every export as of the last
scripts/design/update-design-assets.sh run. A changed source with unchanged exports, a changed
export, or a missing file fails the check. Paths are relative to apps/macos.
"""
import argparse
import hashlib
import json
import platform
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
MANIFEST = ROOT / "design/assets-manifest.json"
ICON = "Kudzu/Kudzu.icon"
PRINTED_K = "02-echo-k-printed.svg"


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def native_icon():
    """The Icon Composer document's files; fails if its k isn't the generated one."""
    icon = ROOT / ICON
    document = json.loads((icon / "icon.json").read_text())
    images = {layer["image-name"] for group in document["groups"] for layer in group["layers"]}
    for name in images:
        if not (icon / "Assets" / name).is_file():
            raise ValueError(f"Missing native icon layer: {name}")
    if PRINTED_K not in images or digest(icon / "Assets" / PRINTED_K) != digest(ROOT / "design/app-icon/layers" / PRINTED_K):
        raise ValueError("The generated k differs from the native icon. Use Image > Replace in Icon Composer, save, then export again.")
    return {f"{ICON}/icon.json"} | {f"{ICON}/Assets/{name}" for name in images}


def inventory(icon_sources):
    sources = icon_sources | {
        "design/brand.json",
        "design/wordmark/kudzu-wordmark-echo.svg",
        "design/wordmark/kudzu-wordmark-echo-small.svg",
        "scripts/design/brand-assets.py",
        "scripts/design/design-assets.swift",
        "scripts/design/update-design-assets.sh",
    }
    outputs = {
        "design/app-icon/Kudzu-1024.png",
        f"design/app-icon/layers/{PRINTED_K}",
        "design/icon.png",
        "design/dmg/background.png",
        "design/wordmark/kudzu-wordmark-echo-small.png",
    }
    for pattern in ("design/menu-bar/*.svg", "Kudzu/Assets.xcassets/MenuBar*.imageset/*.png"):
        outputs.update(str(path.relative_to(ROOT)) for path in ROOT.glob(pattern))
    return {
        "sources": {name: digest(ROOT / name) for name in sorted(sources)},
        "outputs": {name: digest(ROOT / name) for name in sorted(outputs)},
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument("--write", action="store_true", help="Record exports after successful generation")
    mode.add_argument("--native-only", action="store_true", help="Check the imported icon layer before rendering")
    parser.add_argument("--renderer-version")
    args = parser.parse_args()
    if args.write and not args.renderer_version:
        parser.error("--write requires --renderer-version from the renderer used for this export")
    try:
        icon_sources = native_icon()
        if args.native_only:
            print("Native icon layer matches the generated artwork.")
            return 0
        current = inventory(icon_sources)
        if args.write:
            try:
                renderer_version = json.loads(args.renderer_version)
            except ValueError:
                renderer_version = args.renderer_version.strip()
            MANIFEST.write_text(json.dumps({
                "schemaVersion": 2,
                "exportedWith": {"iconComposer": renderer_version, "macOS": platform.mac_ver()[0]},
                **current,
            }, indent=2) + "\n")
            print(f"Recorded {len(current['outputs'])} design exports.")
            return 0
        saved = json.loads(MANIFEST.read_text())
        if saved.get("schemaVersion") != 2:
            raise ValueError("Unsupported design asset manifest version")
        changes = []
        for section in ("sources", "outputs"):
            expected = saved[section]
            actual = current[section]
            for name in sorted(expected.keys() | actual.keys()):
                if expected.get(name) != actual.get(name):
                    changes.append(f"{section}: {name}")
        if changes:
            raise ValueError("Design exports are stale or incomplete:\n  " + "\n  ".join(changes))
        print(f"Design exports are current ({len(current['outputs'])} files).")
        return 0
    except (OSError, ValueError, KeyError, TypeError) as error:
        print(f"Design asset check failed: {error}\nRun scripts/design/update-design-assets.sh on the design Mac and commit the exports and manifest.", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
