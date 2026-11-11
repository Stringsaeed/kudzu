# S15 permission companion spike

From `apps/desktop-v2`:

```sh
cargo test -p permissions-spike -- --nocapture
cargo run -p permissions-spike -- --probe
```

`--probe` packs a signed `.app` (ad-hoc, or `KUDZU_DEV_SIGN` when set), opens the Accessibility privacy pane the shipping companion uses, places this window beside it, and proves drag-out pasteboard types. It does not grant, revoke, or flip any privacy switch, and it does not drop the icon into the list. System Settings is quit afterwards only if this process opened it.

`cargo test` and a bare `cargo run` open no windows.
