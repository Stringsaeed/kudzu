#!/usr/bin/env python3
"""Exercise Rune's early headless dispatch with writes confined to one temp directory."""
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]
BINARY = ROOT / "target/debug/kudzu"
with tempfile.TemporaryDirectory(prefix="kudzu-headless-") as directory:
    scratch = Path(directory).resolve()
    env = dict(os.environ, RUNE_DIRS_ROOT=str(scratch), HOME=str(scratch),
               TMPDIR=str(scratch), CFFIXED_USER_HOME=str(scratch),
               XDG_CACHE_HOME=str(scratch / "cache"))
    sandbox = shutil.which("sandbox-exec")
    if not sandbox:
        raise SystemExit("sandbox-exec is required to prove writes stay inside the temp folder")
    profile = '(version 1)(allow default)(deny file-write*)(allow file-write* (subpath ' + json.dumps(str(scratch)) + ') (literal "/dev/null"))'
    for flag, output in [("--tree", "tree.json"), ("--snap", "snap.png"),
                         ("--list-windows", None), ("--list-scenes", None)]:
        args = [sandbox, "-p", profile, str(BINARY), flag]
        if output:
            args.append(str(scratch / output))
        result = subprocess.run(args, env=env, cwd=scratch, capture_output=True, text=True)
        print(flag, result.returncode, result.stdout.strip(), result.stderr.strip())
        result.check_returncode()
        if output:
            assert (scratch / output).stat().st_size > 0
    tree = json.loads((scratch / "tree.json").read_text())
    assert "Kudzu Next" in json.dumps(tree)
    assert (scratch / "snap.png").read_bytes().startswith(b"\x89PNG\r\n\x1a\n")
    print("All four modes passed under a filesystem write sandbox; no S01 native adapters exist.")
