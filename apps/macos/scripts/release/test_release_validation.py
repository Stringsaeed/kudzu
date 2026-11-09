import copy
import datetime
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

from validate_profile import validate

ROOT = Path(__file__).resolve().parents[2]


class ReleaseValidationTests(unittest.TestCase):
    def test_profile_requires_every_restricted_entitlement_and_a_current_identity(self):
        profile = {
            "ExpirationDate": datetime.datetime.now(datetime.timezone.utc) + datetime.timedelta(days=1),
            "TeamIdentifier": ["V3HN8HXZYK"],
            "DeveloperCertificates": [b"fixture certificate"],
            "Entitlements": {
                "com.apple.application-identifier": "V3HN8HXZYK.sh.saeed.kudzu",
                "com.apple.developer.team-identifier": "V3HN8HXZYK",
                "com.apple.developer.icloud-container-identifiers": ["iCloud.sh.saeed.kudzu.library"],
                "com.apple.developer.icloud-services": ["CloudKit"],
                "com.apple.developer.icloud-container-environment": "Production",
                "com.apple.developer.aps-environment": "production",
            },
        }
        validate(profile)
        # Developer ID profiles issued by Apple permit service specialization with
        # a string wildcard; container, app, team and signer checks still apply.
        wildcard = copy.deepcopy(profile)
        wildcard["Entitlements"]["com.apple.developer.icloud-services"] = "*"
        validate(wildcard)
        for key, wrong in (
            ("com.apple.application-identifier", "OTHER.sh.saeed.kudzu"),
            ("com.apple.developer.team-identifier", "OTHER"),
            ("com.apple.developer.icloud-container-identifiers", ["iCloud.sh.saeed.kudzu.library.other"]),
            ("com.apple.developer.icloud-services", ["CloudDocuments"]),
            ("com.apple.developer.icloud-container-environment", "Development"),
            ("com.apple.developer.aps-environment", "development"),
        ):
            with self.subTest(key=key):
                invalid = copy.deepcopy(profile)
                invalid["Entitlements"][key] = wrong
                with self.assertRaises(ValueError):
                    validate(invalid)
        for key, wrong in (
            ("ExpirationDate", datetime.datetime.now(datetime.timezone.utc) - datetime.timedelta(days=1)),
            ("TeamIdentifier", ["OTHER"]),
            ("DeveloperCertificates", []),
        ):
            with self.subTest(key=key):
                invalid = copy.deepcopy(profile)
                invalid[key] = wrong
                with self.assertRaises(ValueError):
                    validate(invalid)

    def test_signed_version_floor_runs_before_any_credential_or_build_operation(self):
        environment = {k: v for k, v in os.environ.items() if not k.startswith(("APPLE_", "TAURI_"))}
        environment.update(UNSIGNED="0", ALLOW_VERSION_OVERRIDE="1")
        for version, channel, allowed in (("0.6.1", "stable", False), ("0.7.0-beta.1", "stable", False),
                                          ("0.7.0", "stable", True), ("0.7.0-beta.1", "beta", True)):
            with self.subTest(version=version, channel=channel):
                environment.update(VERSION=version, CHANNEL=channel)
                result = subprocess.run(["bash", str(ROOT / "scripts/build-app.sh")],
                                        env=environment, capture_output=True, text=True)
                self.assertNotEqual(result.returncode, 0)
                self.assertIn("APPLE_SIGNING_IDENTITY is not set" if allowed else "native releases must be at least 0.7.0",
                              result.stderr)

    def test_beta_feed_distinguishes_noop_absence_and_errors(self):
        workflow = ROOT.parents[1] / ".github/workflows/build-macos.yml"
        ruby = 'x=YAML.load_file(ARGV[0]); puts x["jobs"]["macos"]["steps"].find { |s| s["name"] == "Update the beta feed" }["run"]'
        script = subprocess.check_output(["ruby", "-ryaml", "-e", ruby, str(workflow)], text=True)
        for mode, current, expected in (("ok", '{"version":"0.8.0"}', 0),
                                         ("ok", '{"version":"0.6.0"}', 1),
                                         ("absent", '', 1), ("ok", '{bad', None),
                                         ("download-error", '{"version":"0.8.0"}', None),
                                         ("list-error", '', None), ("assets-error", '', None)):
            with self.subTest(mode=mode, current=current), tempfile.TemporaryDirectory() as temporary:
                work = Path(temporary)
                (work / "dist").mkdir()
                (work / "dist/latest.json").write_text('{"version":"0.7.0"}')
                (work / "current.json").write_text(current)
                (work / "scripts/release").mkdir(parents=True)
                (work / "scripts/release/feed.mjs").write_bytes((ROOT / "scripts/release/feed.mjs").read_bytes())
                gh = work / "gh"
                gh.write_text('''#!/bin/bash
set -eu
case "$1 $2" in
  "api repos/fixture/repo/releases") [ "$MODE" != list-error ] || exit 17; echo update-feed ;;
  "release view") [ "$MODE" != assets-error ] || exit 19; [ "$MODE" = absent ] || echo beta.json ;;
  "release download") [ "$MODE" != download-error ] || exit 23; mkdir -p "$RUNNER_TEMP/feed"; cp "$RUNNER_TEMP/current.json" "$RUNNER_TEMP/feed/beta.json" ;;
  "release upload") echo upload >> "$RUNNER_TEMP/uploads" ;;
  *) exit 29 ;;
esac
''')
                gh.chmod(0o700)
                env = os.environ.copy()
                env.update(PATH=str(work) + os.pathsep + env["PATH"], MODE=mode,
                           RUNNER_TEMP=str(work), GITHUB_REPOSITORY="fixture/repo", GITHUB_SHA="fixture", TAG="v0.7.0")
                result = subprocess.run(["bash", "-eu", "-o", "pipefail", "-c", script],
                                        cwd=work, env=env, capture_output=True, text=True)
                if expected is None:
                    self.assertNotEqual(result.returncode, 0, result.stdout)
                else:
                    self.assertEqual(result.returncode, 0, result.stderr)
                uploads = work / "uploads"
                self.assertEqual(len(uploads.read_text().splitlines()) if uploads.exists() else 0, expected or 0)


if __name__ == "__main__":
    unittest.main()
