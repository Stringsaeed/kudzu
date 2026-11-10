//! Inline image data: base64, data URLs, and PNG/JPEG dimensions.

use super::Dropped;

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

pub(super) fn encode_base64(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = (u32::from(chunk[0]) << 16)
            | (u32::from(*chunk.get(1).unwrap_or(&0)) << 8)
            | u32::from(*chunk.get(2).unwrap_or(&0));
        for k in 0..4 {
            if k <= chunk.len() {
                out.push(ALPHABET[(n >> (18 - 6 * k)) as usize & 63] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

/// Strict base64 (ASCII whitespace ignored, padding optional).
pub(super) fn decode_base64(text: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(text.len() / 4 * 3);
    let (mut acc, mut bits) = (0u32, 0u32);
    let mut padding = false;
    for b in text.bytes() {
        if b.is_ascii_whitespace() {
            continue;
        }
        if b == b'=' {
            padding = true;
            continue;
        }
        if padding {
            return None;
        }
        let v = match b {
            b'A'..=b'Z' => b - b'A',
            b'a'..=b'z' => b - b'a' + 26,
            b'0'..=b'9' => b - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => return None,
        };
        acc = (acc << 6) | u32::from(v);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    Some(out)
}

/// The bytes of a base64 `data:` URL. Only raster types Kudzu can embed are accepted here; the
/// bytes themselves decide PNG or JPEG later.
pub(super) fn data_url_bytes(url: &str) -> Result<Vec<u8>, Dropped> {
    let url = url.trim();
    let Some(rest) = url.get(..5).filter(|p| p.eq_ignore_ascii_case("data:")).map(|_| &url[5..]) else {
        return Err(Dropped::ImagesNotEmbedded);
    };
    let (meta, data) = rest.split_once(',').ok_or(Dropped::ImagesUnsupported)?;
    let meta = meta.to_ascii_lowercase();
    let mut parts = meta.split(';');
    let mime = parts.next().unwrap_or("").trim();
    if !matches!(mime, "image/png" | "image/jpeg" | "image/jpg") || !parts.any(|p| p.trim() == "base64") {
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
        Some(u32::from(u16::from_be_bytes(bytes.get(at..at + 2)?.try_into().ok()?)))
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
            0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x04, 0x00, 0x00, 0xFF, 0xC0, 0x00, 0x0B, 0x08, 0x00, 0x03, 0x00, 0x07,
        ];
        assert_eq!(sniff(&jpeg), Some(("image/jpeg", 7, 3)));
        assert_eq!(sniff(b"GIF89a"), None);
        assert_eq!(sniff(&[0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x00]), None);
    }

    #[test]
    fn data_urls() {
        assert_eq!(data_url_bytes("https://x/y.png"), Err(Dropped::ImagesNotEmbedded));
        assert_eq!(data_url_bytes("data:image/gif;base64,R0lG"), Err(Dropped::ImagesUnsupported));
        assert_eq!(data_url_bytes("data:image/svg+xml,<svg/>"), Err(Dropped::ImagesUnsupported));
        assert_eq!(data_url_bytes("DATA:Image/PNG;Base64,YWI="), Ok(b"ab".to_vec()));
    }
}
