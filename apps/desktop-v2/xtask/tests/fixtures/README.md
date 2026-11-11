# Throwaway updater key (test fixture)

`throwaway-tauri-key`, `.pub` and `.password` are a minisign key pair in the exact format Tauri's
CLI writes (`tauri signer generate`): the files hold the base64 wrapping of minisign's text
format, as `TAURI_SIGNING_PRIVATE_KEY` does in CI. The password is
`throwaway-not-a-real-password`.

**This key signs nothing real.** It was generated for these tests with the frozen app's CLI, in a
temporary directory, and has no relation to the production updater key (key id
`6E52269F4241F007`). Nothing trusts it, and `cargo xtask release` only accepts it for an ad-hoc
build (`APPLE_SIGNING_IDENTITY=-`).
