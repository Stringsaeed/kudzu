//! Platform decoder for clipboard formats (including HEIC, TIFF and PDF).
use crate::error::PortError;
#[derive(Clone, PartialEq, Eq)]
pub struct Thumbnail {
    pub png: Vec<u8>,
    pub width: u32,
    pub height: u32,
}
pub trait Thumbnailer: Send + Sync {
    /// Original dimensions plus PNG bytes bounded to max_px without upscaling.
    fn thumbnail(&self, bytes: &[u8], max_px: u32) -> Result<Option<Thumbnail>, PortError>;
}
