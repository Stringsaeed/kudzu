"""Tests for check-design-assets.py. Run from apps/macos:
python3 -m unittest discover -s scripts/design -p test_design_assets.py
"""
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]


class DesignAssetCheck(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="kudzu-asset-check-")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name) / "apps/macos"
        manifest = json.loads((ROOT / "design/assets-manifest.json").read_text())
        files = set(manifest["sources"]) | set(manifest["outputs"]) | {
            "design/assets-manifest.json", "scripts/design/check-design-assets.py",
        }
        for name in files:
            target = self.root / name
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(ROOT / name, target)

    def check(self, *args):
        return subprocess.run([sys.executable, str(self.root / "scripts/design/check-design-assets.py"), *args], capture_output=True, text=True)

    def test_committed_exports_pass(self):
        result = self.check()
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_changed_tagline_requires_export(self):
        path = self.root / "design/brand.json"
        brand = json.loads(path.read_text())
        brand["tagline"] = "A changed tagline"
        path.write_text(json.dumps(brand))
        result = self.check()
        self.assertEqual(result.returncode, 1)
        self.assertIn("sources: design/brand.json", result.stderr)

    def test_changed_export_is_rejected(self):
        (self.root / "design/dmg/background.png").write_bytes(b"replaced background")
        result = self.check()
        self.assertEqual(result.returncode, 1)
        self.assertIn("outputs: design/dmg/background.png", result.stderr)

    def test_missing_menu_bar_glyph_is_rejected(self):
        (self.root / "Kudzu/Assets.xcassets/MenuBar.imageset/kudzu-menubar@2x.png").unlink()
        result = self.check()
        self.assertEqual(result.returncode, 1)
        self.assertIn("kudzu-menubar@2x.png", result.stderr)

    def test_unimported_artwork_cannot_be_attested(self):
        (self.root / "design/app-icon/layers/02-echo-k-printed.svg").write_text("new artwork")
        manifest = self.root / "design/assets-manifest.json"
        original = manifest.read_bytes()
        result = self.check("--write", "--renderer-version", "test renderer")
        self.assertEqual(result.returncode, 1)
        self.assertIn("Image > Replace", result.stderr)
        self.assertEqual(manifest.read_bytes(), original)

    def test_release_version_stamp_needs_no_new_artwork(self):
        (self.root / "Kudzu/Version.xcconfig").write_text("MARKETING_VERSION = 0.8.0-beta.1\n")
        result = self.check()
        self.assertEqual(result.returncode, 0, result.stderr)


if __name__ == "__main__":
    unittest.main()
