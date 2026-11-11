use super::error::UpdateError;
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use minisign_verify::{PublicKey, Signature};

/// Production minisign public key, compiled in. Same key as the Tauri app's
/// `plugins.updater.pubkey` after base64-decode. S70 signs with the matching secret
/// (`TAURI_SIGNING_PRIVATE_KEY` mapped to `KUDZU_UPDATE_KEY`).
pub const PRODUCTION_PUBKEY: &str =
    "untrusted comment: minisign public key: 6E52269F4241F007\nRWQH8EFCnyZSbuM5BGaktuexN/70p9Q1ZPCwwNt5ilvPkDnn57keAIOC\n";

/// The Tauri config wrapping of [`PRODUCTION_PUBKEY`]. S70 must keep this encoding.
pub const PRODUCTION_PUBKEY_TAURI_B64: &str =
    "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IDZFNTIyNjlGNDI0MUYwMDcKUldRSDhFRkNueVpTYnVNNUJHYWt0dWV4Ti83MHA5UTFaUEN3d050NWlsdlBrRG5uNTdrZUFJT0MK";

pub fn expected_archive_name(version: &str) -> String {
    format!("Kudzu_{version}_universal.app.tar.gz")
}

pub fn decode_signature(signature: &str) -> Result<Signature, UpdateError> {
    let trimmed = signature.trim();
    if let Ok(bytes) = STANDARD.decode(trimmed) {
        if let Ok(text) = std::str::from_utf8(&bytes) {
            if let Ok(decoded) = Signature::decode(text) {
                return Ok(decoded);
            }
        }
    }
    Signature::decode(trimmed).map_err(|_| UpdateError::Signature)
}

/// Cryptographic verification plus trusted-comment binding.
/// Returns the version taken from `file:Kudzu_<version>_universal.app.tar.gz`.
pub fn verify(
    key: &PublicKey,
    bytes: &[u8],
    signature: &str,
    feed_version: &str,
    url: &str,
) -> Result<String, UpdateError> {
    let signature = decode_signature(signature)?;
    key.verify(bytes, &signature, false)
        .map_err(|_| UpdateError::Signature)?;
    bind_archive(signature.trusted_comment(), feed_version, url)
}

pub fn bind_archive(comment: &str, feed_version: &str, url: &str) -> Result<String, UpdateError> {
    let name = file_from_comment(comment)?;
    let expected = expected_archive_name(feed_version);
    if name != expected {
        return Err(UpdateError::Signature);
    }
    if url_file_name(url) != Some(name) {
        return Err(UpdateError::Signature);
    }
    version_from_archive_name(name).map(str::to_owned)
}

fn file_from_comment(comment: &str) -> Result<&str, UpdateError> {
    comment
        .split('\t')
        .filter_map(|part| part.split_once(':'))
        .find(|(key, _)| *key == "file")
        .map(|(_, value)| value)
        .filter(|value| !value.is_empty())
        .ok_or(UpdateError::Signature)
}

fn version_from_archive_name(name: &str) -> Result<&str, UpdateError> {
    name.strip_prefix("Kudzu_")
        .and_then(|rest| rest.strip_suffix("_universal.app.tar.gz"))
        .filter(|version| !version.is_empty() && !version.contains('/') && !version.contains('\\'))
        .ok_or(UpdateError::Signature)
}

pub fn url_file_name(url: &str) -> Option<&str> {
    let path = url.split('#').next()?.split('?').next()?;
    path.rsplit('/').next().filter(|name| !name.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    const ARCHIVE: &[u8] = include_bytes!("fixtures/archive.bin");
    const ARCHIVE_SIG: &str = include_str!("fixtures/archive.bin.minisig");
    const ARCHIVE_SIG_B64: &str = include_str!("fixtures/archive.bin.sig");
    const TEST_PUB: &str = include_str!("fixtures/test.pub");
    const COMMENT_PUB: &str = include_str!("fixtures/comment.pub");
    const TAURI_ERA: &str = include_str!("fixtures/archive.tauri-era.minisig");
    const NO_FILE: &str = include_str!("fixtures/archive.no-file.minisig");
    const WRONG_FILE: &str = include_str!("fixtures/archive.wrong-file.minisig");
    const ARCHIVE_URL: &str =
        "https://github.com/Stringsaeed/kudzu/releases/download/v9.9.9/Kudzu_9.9.9_universal.app.tar.gz";

    fn test_key() -> PublicKey {
        PublicKey::decode(TEST_PUB).unwrap()
    }

    fn comment_key() -> PublicKey {
        PublicKey::decode(COMMENT_PUB).unwrap()
    }

    #[test]
    fn raw_and_tauri_wrapped_signatures_verify() {
        assert_eq!(
            verify(&test_key(), ARCHIVE, ARCHIVE_SIG, "9.9.9", ARCHIVE_URL).unwrap(),
            "9.9.9"
        );
        assert_eq!(
            verify(
                &test_key(),
                ARCHIVE,
                ARCHIVE_SIG_B64.trim(),
                "9.9.9",
                ARCHIVE_URL
            )
            .unwrap(),
            "9.9.9"
        );
    }

    #[test]
    fn rejects_mismatched_file() {
        let url = "https://github.com/Stringsaeed/kudzu/releases/download/v9.9.9/evil.tar.gz";
        assert_eq!(
            verify(&test_key(), ARCHIVE, ARCHIVE_SIG, "9.9.9", url),
            Err(UpdateError::Signature)
        );
        assert_eq!(
            verify(&comment_key(), ARCHIVE, WRONG_FILE, "9.9.9", ARCHIVE_URL),
            Err(UpdateError::Signature)
        );
    }

    #[test]
    fn rejects_mismatched_version() {
        let url = "https://github.com/Stringsaeed/kudzu/releases/download/v8.8.8/Kudzu_8.8.8_universal.app.tar.gz";
        assert_eq!(
            verify(&test_key(), ARCHIVE, ARCHIVE_SIG, "8.8.8", url),
            Err(UpdateError::Signature)
        );
    }

    #[test]
    fn rejects_missing_file_key() {
        assert_eq!(
            bind_archive("timestamp:1730000000", "9.9.9", ARCHIVE_URL),
            Err(UpdateError::Signature)
        );
        assert_eq!(
            verify(&comment_key(), ARCHIVE, NO_FILE, "9.9.9", ARCHIVE_URL),
            Err(UpdateError::Signature)
        );
    }

    #[test]
    fn rejects_tauri_era_trusted_comment() {
        assert_eq!(
            bind_archive(
                "timestamp:1730000000\tfile:Kudzu.app.tar.gz",
                "9.9.9",
                ARCHIVE_URL
            ),
            Err(UpdateError::Signature)
        );
        assert_eq!(
            verify(&comment_key(), ARCHIVE, TAURI_ERA, "9.9.9", ARCHIVE_URL),
            Err(UpdateError::Signature)
        );
    }

    #[test]
    fn tauri_pubkey_wrapping_round_trips() {
        let decoded = STANDARD
            .decode(PRODUCTION_PUBKEY_TAURI_B64)
            .expect("tauri wrapping");
        assert_eq!(decoded, PRODUCTION_PUBKEY.as_bytes());
    }
}
