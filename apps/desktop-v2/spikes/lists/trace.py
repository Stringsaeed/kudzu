"""Run after cargo build --release -p lists-spike; no build or OS state changes."""
import json
import math
import os
import pathlib
import statistics
import subprocess

root = pathlib.Path(__file__).resolve().parents[2]
out = root / "target/s14"
out.mkdir(exist_ok=True)
script = out / "scroll.json"
script.write_text(json.dumps([
    {"t": 1 + i / 120, "type": "wheel", "x": 500, "y": 300,
     "dy": -160 if i < 300 else 160} for i in range(600)
]))

def percentile(values, q):
    return sorted(values)[math.ceil(len(values) * q) - 1]

results = []
for scene in ("library", "clips"):
    for run in range(7):
        trace = out / f"{scene}-{run}.jsonl"
        cmd = [str(root / "target/release/lists-spike"), "--script", str(script),
               "--trace", str(trace), "--frames", "840", "--fps", "120"]
        if scene == "clips":
            cmd += ["--scene", "clips"]
        result = subprocess.run(cmd, capture_output=True, text=True, check=True,
                                env=os.environ | {"RUNE_COMMIT_BUDGET_MS": "0.000001"})
        (out / f"{scene}-{run}.log").write_text(result.stdout + result.stderr)
        frames = [json.loads(line) for line in trace.read_text().splitlines()]
        samples = [f["quality"]["cpu_ms"] for f in frames if "quality" in f
                   and 1000 <= f.get("t", f.get("t_ms", 0)) < 6000]
        if not samples:
            raise RuntimeError("no scroll frames: inspect trace schema")
        if "S14 views=1 " not in result.stderr or result.stderr.count("rune: commit took") != 1:
            raise RuntimeError("wheel trace committed a view: " + result.stderr)
        row = dict(scene=scene, run=run, frames=len(samples),
                   median_ms=statistics.median(samples), p95_ms=percentile(samples,.95),
                   counters=result.stderr.strip())
        results.append(row)
        print(json.dumps(row), flush=True)
(out / "summary.json").write_text(json.dumps(results, indent=2))
