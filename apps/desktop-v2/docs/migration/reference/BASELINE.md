# Installed Tauri release baseline

Measured 2026-11-11 on the owner's Mac. Only passive process inspection was performed. No launch, activation, keystrokes, permission or system setting changes were made to Kudzu. Kudzu Dev was not built or run.

Machine: MacBook Pro Mac14,5, Apple M2 Max (12 CPU / 30 GPU cores), 32 GB RAM. macOS 27.2 (26B123). Built-in Liquid Retina XDR, 3024 × 1964 Retina, main display, no mirroring. No machine serial numbers or hardware identifiers are retained.

Installed bundle version: **0.7.0**, bundle id `sh.saeed.kudzu` (read from Info.plist). The browser reference comes from the source commit recorded in README; the installed binary is not asserted to be built from that exact commit.

Installed executable: `/Applications/Kudzu.app/Contents/MacOS/kudzu-desktop`, PID 56152, already running for about 4 h 54 min at the initial sample. These measurements describe the session as found; window visibility and prewarming were not changed or asserted.

| Measurement | Result | Method / limitation |
|---|---|---|
| Main-process resident memory | 98,528 KiB (96.22 MiB) during the 20 s sample; initial reading 100,880 KiB | `ps -p 56152 -o pid=,rss=,%cpu=,time=`; RSS is not physical footprint and excludes WebKit helper processes. |
| Idle CPU | 0.0% in all 21 samples, 1 s apart; accumulated CPU stayed at 1:42.76 | Same command, 20 s observation. `ps` CPU is a decaying average, not an instantaneous utilization meter. No assertion of zero redraws. |
| Complete app physical footprint | **owner to run** | After 60 s with windows hidden and panels prewarmed, run `footprint <main-pid>` and include attributable WebKit processes. Do not compare the Rune 150 MB budget to main-process RSS alone. |
| Hidden launch → services ready | **owner to run** | On a separately scheduled owner session, instrument release startup with a monotonic start mark and a mark after store/engine/hotkey initialization. One discarded launch, then seven warm launches; report median. No equivalent Tauri timing marks were added by S02. |
| Launch → main content | **owner to run** | Record process start and first frame with main content visible, using a monotonic instrumentation mark plus synchronized screen recording. Discard one, median of seven warm launches. Record cache/launch conditions; a genuinely cold result must be identified separately. |
| Hotkey → palette visible | **owner to run** | Prewarm panel, record hotkey handler arrival and first encoded palette frame with monotonic marks; synchronized ≥120 fps recording measures visible delay. 100 opens; report handler-to-encode p95 and visible p95, with refresh rate and recording resolution. Repeat for picker containing 5,000 synthetic clips in isolated Dev data. |

Raw passive samples: [baseline-process.json](baseline-process.json). Owner timings remain unmeasured to preserve the active production session, as explicitly required by the task. Browser-preview startup times are not native release baselines.
