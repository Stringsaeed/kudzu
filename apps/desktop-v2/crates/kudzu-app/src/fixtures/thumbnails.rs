//! Configured thumbnail output; records byte length and requested bound, never decodes.
use crate::{
    error::PortError,
    ports::{Thumbnail, Thumbnailer},
};
use std::sync::Mutex;
#[derive(Default)]
pub struct FakeThumbnailer {
    pub output: Mutex<Option<Thumbnail>>,
    pub requests: Mutex<Vec<(usize, u32)>>,
    pub failure: Mutex<Option<PortError>>,
}
impl Thumbnailer for FakeThumbnailer {
    fn thumbnail(&self, bytes: &[u8], max_px: u32) -> Result<Option<Thumbnail>, PortError> {
        if let Some(e) = *self.failure.lock().unwrap() {
            return Err(e);
        }
        self.requests.lock().unwrap().push((bytes.len(), max_px));
        Ok(self.output.lock().unwrap().clone())
    }
}
