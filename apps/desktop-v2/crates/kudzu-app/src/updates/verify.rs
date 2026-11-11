use super::error::UpdateError;
use minisign_verify::{PublicKey, Signature};

/// Production minisign public key, compiled in. Same key as the Tauri app's
/// `plugins.updater.pubkey` after base64-decode. S70 signs with the matching secret
/// (`TAURI_SIGNING_PRIVATE_KEY` mapped to `KUDZU_UPDATE_KEY`).
pub const PRODUCTION_PUBKEY: &str =
    "untrusted comment: minisign public key: 6E52269F4241F007\nRWQH8EFCnyZSbuM5BGaktuexN/70p9Q1ZPCwwNt5ilvPkDnn57keAIOC\n";

/// The Tauri config wrapping of [`PRODUCTION_PUBKEY`]. S70 must keep this encoding.
pub const PRODUCTION_PUBKEY_TAURI_B64: &str =
    "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IDZFNTIyNjlGNDI0MUYwMDcKUldRSDhFRkNueVpTYnVNNUJHYWt0dWV4Ti83MHA5UTFaUEN3d050NWlsdlBrRG5uNTdrZUFJT0MK";

pub fn verify(key: &PublicKey, bytes: &[u8], signature: &str) -> Result<(), UpdateError> {
    let signature = Signature::decode(signature).map_err(|_| UpdateError::Signature)?;
    key.verify(bytes, &signature, false)
        .map_err(|_| UpdateError::Signature)
}
