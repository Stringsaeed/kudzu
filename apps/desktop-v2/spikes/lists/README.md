# S14 list spike

Run from `apps/desktop-v2`:

```sh
cargo test -p lists-spike -- --nocapture
cargo run --release -p lists-spike
python3 spikes/lists/trace.py
```

The binary normally runs assertions and seven 600-frame wheel traces through Rune's
headless `Runtime`. Rune tooling flags select a full GPU-backed headless App instead:

```sh
target/release/lists-spike --script target/s14/scroll.json --trace target/s14/manual.jsonl --frames 840 --fps 120
```

`trace.py` creates that script, runs seven times each for library (64 pt items) and
clipboard (58 pt items), and summarizes the actual Rune `quality.cpu_ms` samples
for the five-second scroll interval. Headers are 30 pt every 100 items. It saves
raw traces and logs under `target/s14`, with no product adapters or permissions.
Each App prints its total view count on shutdown; subtract the initial view.
Neither runtime nor headless traces measure native display latency.

The workspace glob automatically discovers this package. Its new lock entry is
provided in `workspace-lock.patch` because the shared root lock is outside S14's
owned paths. Apply that patch at integration before running locked commands.
