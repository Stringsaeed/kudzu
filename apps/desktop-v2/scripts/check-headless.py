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
    listed = subprocess.run([sandbox, "-p", profile, str(BINARY), "--list-windows"], env=env,
                            cwd=scratch, capture_output=True, text=True, check=True).stdout.split()
    for key in ["main", "palette", "picker", "form", "permissions"]:
        assert key in listed, f"--list-windows is missing {key}: {listed}"

    # Scripts that cross windows run on Rune's virtual desktop, on fixtures in this folder.
    search = {"t": 0.2, "type": "shortcut", "chord": "ctrl+alt+shift+space"}
    clipboard = {"t": 0.2, "type": "shortcut", "chord": "ctrl+alt+shift+v"}
    escape = {"t": 0.6, "type": "key", "name": "Escape"}
    command_q = {"t": 0.6, "type": "key", "name": "q", "cmd": True}
    reopen = {"t": 1.0, "reopen": True}

    def desktop(name, events, window="main", extra=()):
        script = scratch / f"{name}.json"
        script.write_text(json.dumps(events + [{"t": 1.4, "type": "mark", "name": "end"}]))
        out = scratch / f"{name}-tree.json"
        args = [sandbox, "-p", profile, str(BINARY), "--desktop", str(scratch / "snap.png"),
                "--script", str(script), "--window", window, "--tree", str(out), *extra]
        result = subprocess.run(args, env=env, cwd=scratch, capture_output=True, text=True)
        print(name, result.returncode, result.stderr.strip().splitlines()[-1:])
        result.check_returncode()
        return {w["key"]: w["visible"] for w in json.loads(out.read_text())["windows"]}

    shown = desktop("shortcut-opens-palette", [search], "palette")
    assert shown["palette"] and shown["main"], shown
    shown = desktop("escape-hides-palette", [search, escape], "palette")
    assert not shown["palette"] and shown["main"], shown
    shown = desktop("shortcut-opens-picker", [clipboard], "picker")
    assert shown["picker"], shown
    shown = desktop("command-q-closes-windows", [search, command_q], "palette")
    assert not shown["palette"] and not shown["main"], shown
    shown = desktop("reopen-opens-main", [search, command_q, reopen])
    assert shown["main"] and not shown["palette"], shown
    shown = desktop("hidden-ignored-on-first-run", [], extra=["--hidden"])
    assert shown["main"], shown
    print("All modes and window scripts passed under a filesystem write sandbox, on fixtures.")
