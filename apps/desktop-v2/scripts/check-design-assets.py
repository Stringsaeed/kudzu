#!/usr/bin/env python3
"""Verify committed design exports without requiring macOS rendering tools in CI."""
import argparse
import hashlib
import json
import platform
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
MANIFEST = ROOT / "design/assets-manifest.json"


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def native_icon():
    icon = ROOT / "design/app-icon/Kudzu.icon"
    document = json.loads((icon / "icon.json").read_text())
    images = {layer["image-name"] for group in document["groups"] for layer in group["layers"]}
    for name in images:
        if not (icon / "Assets" / name).is_file():
            raise ValueError(f"Missing native icon layer: {name}")
    name = "02-echo-k-printed.svg"
    if name not in images or digest(icon / "Assets" / name) != digest(ROOT / "design/app-icon/layers" / name):
        raise ValueError("The generated k differs from the native icon. Use Image > Replace in Icon Composer, save, then export again.")
    return {"design/app-icon/Kudzu.icon/icon.json"} | {f"design/app-icon/Kudzu.icon/Assets/{name}" for name in images}


def inventory(icon_sources):
    sources = icon_sources | {
        "design/brand.json", "design/wordmark/kudzu-wordmark-echo-small.svg",
        "design/app-icon/layers/02-echo-k-printed.svg",
        "scripts/brand-assets.py", "scripts/design-assets.swift", "scripts/update-design-assets.sh",
    }
    outputs = {"assets/icons/icon.icns"} | {
        f"assets/tray/{name}{scale}.png"
        for name in ("normal", "paused", "permissions", "secure-input")
        for scale in ("", "@2x", "@3x")
    }
    return {
        "sources": {name: digest(ROOT / name) for name in sorted(sources)},
        "outputs": {name: digest(ROOT / name) for name in sorted(outputs)},
    }



def main():
    parser = argparse.ArgumentParser(description=__doc__)
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
                "schemaVersion": 1,
                "exportedWith": {"iconComposer": renderer_version, "macOS": platform.mac_ver()[0]},
                **current,
            }, indent=2) + "\n")
            print(f"Recorded {len(current['outputs'])} design exports.")
            return 0
        saved = json.loads(MANIFEST.read_text())
        if saved.get("schemaVersion") != 1:
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
        print(f"Design asset check failed: {error}\nRun scripts/update-design-assets.sh on the design Mac and commit the exports and manifest.", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
