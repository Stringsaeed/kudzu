//! Clipboard image and Finder icon retrieval.
use super::*;
use std::io::Read;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaKind {
    Thumbnail,
    Image,
    File(usize),
    Icon(usize),
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Media {
    pub mime: &'static str,
    pub bytes: Vec<u8>,
}
impl ClipboardService {
    pub fn media(&self, id: Uuid, kind: MediaKind) -> Option<Media> {
        let h = self.watch().get();
        let e = h.get(id)?;
        let (mime, bytes) = match kind {
            MediaKind::Thumbnail => ("image/png", fs::read(self.store.thumb(id)).ok()?),
            MediaKind::Image => {
                let items = self.store.items(id)?;
                let (bytes, mime) = clipboard::image_of(&items)?;
                (mime, bytes.to_vec())
            }
            MediaKind::File(i) => {
                let path = e.files.get(i)?;
                let mime = clipboard::image_mime_for_path(path)?;
                let meta = fs::metadata(path).ok()?;
                if !meta.is_file() || meta.len() > MAX_IMAGE_FILE {
                    return None;
                }
                let mut bytes = Vec::new();
                fs::File::open(path)
                    .ok()?
                    .take(MAX_IMAGE_FILE + 1)
                    .read_to_end(&mut bytes)
                    .ok()?;
                if bytes.len() as u64 > MAX_IMAGE_FILE {
                    return None;
                }
                (mime, bytes)
            }
            MediaKind::Icon(i) => (
                "image/png",
                self.icons.file_icon(e.files.get(i)?, 64).ok().flatten()?,
            ),
        };
        Some(Media { mime, bytes })
    }
}
