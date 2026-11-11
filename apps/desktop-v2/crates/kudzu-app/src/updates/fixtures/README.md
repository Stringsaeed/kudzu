# Throwaway updater fixture

Signed **outside** this crate with the `minisign` CLI so verification is not
tested only against a signer in the same code. This is **not** the production
updater key. The secret was deleted and was never committed.

Generated with minisign 0.12:

```sh
minisign -G -p test.pub -s /tmp/kudzu-s31-updater.key -W
printf 'payload-kudzu-s31-signed-archive\n' > archive.bin
minisign -S -s /tmp/kudzu-s31-updater.key -m archive.bin \
  -t "$(printf 'timestamp:1730000000\tfile:Kudzu_9.9.9_universal.app.tar.gz')"
rm -f /tmp/kudzu-s31-updater.key
```

`archive.bin.minisig` is the feed `signature` string. S70 must emit the same
minisign file shape (trusted comment `timestamp:<unix>` then a tab then
`file:Kudzu_<version>_universal.app.tar.gz`) and sign with the production secret.
