# Throwaway updater fixtures

Signed **outside** this crate with the `minisign` CLI so verification is not
tested only against a signer in the same code. These are **not** the production
updater key. Secrets were deleted and were never committed.

Happy-path key (`test.pub`) with minisign 0.12:

```sh
minisign -G -p test.pub -s /tmp/kudzu-s31-updater.key -W
printf 'payload-kudzu-s31-signed-archive\n' > archive.bin
minisign -S -s /tmp/kudzu-s31-updater.key -m archive.bin \
  -t "$(printf 'timestamp:1730000000\tfile:Kudzu_9.9.9_universal.app.tar.gz')"
rm -f /tmp/kudzu-s31-updater.key
```

`archive.bin.minisig` is raw minisign text. `archive.bin.sig` is that text
base64-wrapped, matching today's Tauri `.sig` → `latest.json` field.

Comment-policy signatures (`comment.pub`) cover a missing `file:` key, a
Tauri-era `file:Kudzu.app.tar.gz`, and a `file:` name that does not match
the feed version. S70 must emit the production shape: trusted comment
`timestamp:<unix>` then a tab then `file:Kudzu_<version>_universal.app.tar.gz`,
and put the **base64-wrapped** minisign file in `latest.json`.
