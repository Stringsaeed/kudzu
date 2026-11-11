//! The raw pasteboard data of an entry, encoded for exact restoration, and lookups into it.

use super::classify::find;
use super::{uti, PasteItem};

const BLOB_MAGIC: &[u8; 4] = b"KZC1";

/// Serializes pasteboard items: magic, then per item a type count and `(type, data)` pairs,
/// every length a little-endian u32/u64.
pub fn encode_items(items: &[PasteItem]) -> Vec<u8> {
    let size: usize = items
        .iter()
        .flatten()
        .map(|(t, d)| t.len() + d.len() + 12)
        .sum();
    let mut out = Vec::with_capacity(size + 8);
    out.extend_from_slice(BLOB_MAGIC);
    out.extend_from_slice(&(items.len() as u32).to_le_bytes());
    for item in items {
        out.extend_from_slice(&(item.len() as u32).to_le_bytes());
        for (t, d) in item {
            out.extend_from_slice(&(t.len() as u32).to_le_bytes());
            out.extend_from_slice(t.as_bytes());
            out.extend_from_slice(&(d.len() as u64).to_le_bytes());
            out.extend_from_slice(d);
        }
    }
    out
}

/// Inverse of [`encode_items`]; `None` on any malformed input.
pub fn decode_items(bytes: &[u8]) -> Option<Vec<PasteItem>> {
    struct Reader<'a>(&'a [u8]);
    impl<'a> Reader<'a> {
        fn take(&mut self, n: usize) -> Option<&'a [u8]> {
            if self.0.len() < n {
                return None;
            }
            let (a, b) = self.0.split_at(n);
            self.0 = b;
            Some(a)
        }
        fn u32(&mut self) -> Option<usize> {
            Some(u32::from_le_bytes(self.take(4)?.try_into().ok()?) as usize)
        }
        fn u64(&mut self) -> Option<usize> {
            usize::try_from(u64::from_le_bytes(self.take(8)?.try_into().ok()?)).ok()
        }
    }
    let mut r = Reader(bytes);
    if r.take(4)? != BLOB_MAGIC {
        return None;
    }
    let count = r.u32()?;
    let mut items = Vec::with_capacity(count.min(1024));
    for _ in 0..count {
        let n = r.u32()?;
        let mut item = Vec::with_capacity(n.min(256));
        for _ in 0..n {
            let tl = r.u32()?;
            let t = std::str::from_utf8(r.take(tl)?).ok()?.to_string();
            let dl = r.u64()?;
            item.push((t, r.take(dl)?.to_vec()));
        }
        items.push(item);
    }
    r.0.is_empty().then_some(items)
}

/// Full plain text of decoded items, if any.
pub fn text_of(items: &[PasteItem]) -> Option<String> {
    find(items, uti::PLAIN_TEXT)
        .or_else(|| find(items, uti::LEGACY_TEXT))
        .and_then(|d| String::from_utf8(d.to_vec()).ok())
}

/// Bytes and MIME type of the best image in decoded items.
pub fn image_of(items: &[PasteItem]) -> Option<(&[u8], &'static str)> {
    uti::IMAGES
        .iter()
        .find_map(|(t, mime)| find(items, t).map(|d| (d, *mime)))
}

/// MIME type for an image file path the UI can display, by extension.
pub fn image_mime_for_path(path: &str) -> Option<&'static str> {
    let ext = path.rsplit_once('.')?.1.to_ascii_lowercase();
    Some(match ext.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "heic" | "heif" => "image/heic",
        "tif" | "tiff" => "image/tiff",
        "bmp" => "image/bmp",
        "avif" => "image/avif",
        _ => return None,
    })
}
