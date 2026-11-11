//! Inline image data: base64, data URLs, and PNG/JPEG dimensions.

use base64::engine::general_purpose::{GeneralPurpose, GeneralPurposeConfig};
use base64::engine::{DecodePaddingMode, Engine};

use super::Dropped;

/// Whitespace-tolerant base64 with optional padding, as the hand-written codec accepted.
const BASE64: GeneralPurpose = GeneralPurpose::new(
    &base64::alphabet::STANDARD,
    GeneralPurposeConfig::new()
        .with_decode_padding_mode(DecodePaddingMode::Indifferent)
        .with_decode_allow_trailing_bits(true),
);

pub(super) fn encode_base64(bytes: &[u8]) -> String {
    BASE64.encode(bytes)
}

/// Strict base64 (ASCII whitespace ignored, padding optional).
pub(super) fn decode_base64(text: &str) -> Option<Vec<u8>> {
    let mut compact: Vec<u8> = text.bytes().filter(|b| !b.is_ascii_whitespace()).collect();
    // The original reader accepted any amount of terminal padding, even padding alone.
    while compact.last() == Some(&b'=') {
        compact.pop();
    }
    // A lone final sextet contributed no bytes. Complete it so the crate validates that
    // symbol too, then discard the three bytes introduced by completing the quartet.
    let incomplete = compact.len() % 4 == 1;
    if incomplete {
        compact.extend_from_slice(b"AAA");
    }
    let mut decoded = BASE64.decode(compact).ok()?;
    if incomplete {
        decoded.truncate(decoded.len() - 3);
    }
    Some(decoded)
}

/// The bytes of a base64 `data:` URL. Only raster types Kudzu can embed are accepted here; the
/// bytes themselves decide PNG or JPEG later.
pub(super) fn data_url_bytes(url: &str) -> Result<Vec<u8>, Dropped> {
    let url = url.trim();
    let Some(rest) = url
        .get(..5)
        .filter(|p| p.eq_ignore_ascii_case("data:"))
        .map(|_| &url[5..])
    else {
        return Err(Dropped::ImagesNotEmbedded);
    };
    let (meta, data) = rest.split_once(',').ok_or(Dropped::ImagesUnsupported)?;
    let meta = meta.to_ascii_lowercase();
    let mut parts = meta.split(';');
    let mime = parts.next().unwrap_or("").trim();
    if !matches!(mime, "image/png" | "image/jpeg" | "image/jpg")
        || !parts.any(|p| p.trim() == "base64")
    {
        return Err(Dropped::ImagesUnsupported);
    }
    if data.len() > 4 * 196_608 {
        return Err(Dropped::ImagesTooLarge);
    }
    decode_base64(data).ok_or(Dropped::ImagesUnsupported)
}

/// MIME type, width and height of PNG or JPEG bytes.
pub(super) fn sniff(bytes: &[u8]) -> Option<(&'static str, u32, u32)> {
    let be32 = |at: usize| -> Option<u32> {
        Some(u32::from_be_bytes(bytes.get(at..at + 4)?.try_into().ok()?))
    };
    let be16 = |at: usize| -> Option<u32> {
        Some(u32::from(u16::from_be_bytes(
            bytes.get(at..at + 2)?.try_into().ok()?,
        )))
    };
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        if bytes.get(12..16)? != b"IHDR" {
            return None;
        }
        return Some(("image/png", be32(16)?, be32(20)?));
    }
    if !bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return None;
    }
    let mut i = 2;
    while i + 3 < bytes.len() {
        if bytes[i] != 0xFF {
            return None;
        }
        let marker = bytes[i + 1];
        if marker == 0xFF {
            i += 1;
            continue;
        }
        if marker == 0xD8 || marker == 0x01 || (0xD0..=0xD7).contains(&marker) {
            i += 2;
            continue;
        }
        let length = be16(i + 2)? as usize;
        if (0xC0..=0xCF).contains(&marker) && !matches!(marker, 0xC4 | 0xC8 | 0xCC) {
            return Some(("image/jpeg", be16(i + 7)?, be16(i + 5)?));
        }
        if length < 2 {
            return None;
        }
        i += 2 + length;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_preserves_legacy_decode_acceptance() {
        for (encoded, decoded) in [
            ("", &b""[..]),
            ("=", &b""[..]),
            ("=====", &b""[..]),
            ("A", &b""[..]),
            ("A===", &b""[..]),
            ("YWJjZ", &b"abc"[..]),
            ("YWJjZ====", &b"abc"[..]),
            ("YWI", &b"ab"[..]),
            ("YWI=====", &b"ab"[..]),
            ("YWJ", &b"ab"[..]),
            (" Y W\nI\t=\r= ", &b"ab"[..]),
        ] {
            assert_eq!(
                decode_base64(encoded).as_deref(),
                Some(decoded),
                "{encoded:?}"
            );
        }
        for encoded in ["=A", "YW=I", "YWJj*", "YWJjé", "YWI=Z", "YW\u{a0}I"] {
            assert_eq!(decode_base64(encoded), None, "{encoded:?}");
        }
    }

    #[test]
    fn base64_round_trips() {
        for data in [&b""[..], b"a", b"ab", b"abc", b"\x00\xff\x10\x80"] {
            assert_eq!(decode_base64(&encode_base64(data)).unwrap(), data);
        }
        assert_eq!(encode_base64(b"ab"), "YWI=");
        assert!(decode_base64("YW=I").is_none());
        assert!(decode_base64("Y*").is_none());
    }

    #[test]
    fn reads_dimensions() {
        let png = decode_base64("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGNgYGD4DwABBAEAX+XDSwAAAABJRU5ErkJggg==").unwrap();
        assert_eq!(sniff(&png), Some(("image/png", 1, 1)));
        // SOI, an APP0 segment, then SOF0 with height 3 and width 7.
        let jpeg = [
            0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x04, 0x00, 0x00, 0xFF, 0xC0, 0x00, 0x0B, 0x08, 0x00,
            0x03, 0x00, 0x07,
        ];
        assert_eq!(sniff(&jpeg), Some(("image/jpeg", 7, 3)));
        assert_eq!(sniff(b"GIF89a"), None);
        assert_eq!(sniff(&[0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x00]), None);
    }

    #[test]
    fn data_urls() {
        assert_eq!(
            data_url_bytes("https://x/y.png"),
            Err(Dropped::ImagesNotEmbedded)
        );
        assert_eq!(
            data_url_bytes("data:image/gif;base64,R0lG"),
            Err(Dropped::ImagesUnsupported)
        );
        assert_eq!(
            data_url_bytes("data:image/svg+xml,<svg/>"),
            Err(Dropped::ImagesUnsupported)
        );
        assert_eq!(
            data_url_bytes("DATA:Image/PNG;Base64,YWI="),
            Ok(b"ab".to_vec())
        );
    }
}
