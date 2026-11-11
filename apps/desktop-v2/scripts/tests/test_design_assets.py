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
        self.root = Path(self.temp.name) / "apps/desktop-v2"
        manifest = json.loads((ROOT / "design/assets-manifest.json").read_text())
        files = {name for name in manifest["sources"]} | set(manifest["outputs"]) | {
            "design/assets-manifest.json", "scripts/check-design-assets.py",
        }
        for name in files:
            target = self.root / name
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(ROOT / name, target)

    def check(self, *args):
        return subprocess.run([sys.executable, str(self.root / "scripts/check-design-assets.py"), *args], capture_output=True, text=True)

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
        (self.root / "assets/icons/icon.icns").write_bytes(b"replaced icon")
        result = self.check()
        self.assertEqual(result.returncode, 1)
        self.assertIn("outputs: assets/icons/icon.icns", result.stderr)

    def test_missing_export_is_rejected(self):
        (self.root / "assets/tray/normal.png").unlink()
        result = self.check()
        self.assertEqual(result.returncode, 1)
        self.assertIn("assets/tray/normal.png", result.stderr)

    def test_unimported_artwork_cannot_be_attested(self):
        (self.root / "design/app-icon/layers/02-echo-k-printed.svg").write_text("new artwork")
        manifest = self.root / "design/assets-manifest.json"
        original = manifest.read_bytes()
        result = self.check("--write", "--renderer-version", "test renderer")
        self.assertEqual(result.returncode, 1)
        self.assertIn("Image > Replace", result.stderr)
        self.assertEqual(manifest.read_bytes(), original)

    def test_release_version_stamp_needs_no_new_artwork(self):
        (self.root / "package.json").write_text('{"version":"0.3.0-beta.1"}')
        result = self.check()
        self.assertEqual(result.returncode, 0, result.stderr)



if __name__ == "__main__":
    unittest.main()
