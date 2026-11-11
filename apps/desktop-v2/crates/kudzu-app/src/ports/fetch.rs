//! Blocking HTTP, with mandatory deadline and response cap enforced while reading.
use crate::error::PortError;
use std::time::Duration;
#[derive(Clone)]
pub struct FetchRequest {
    pub url: String,
    pub timeout: Duration,
    pub max_bytes: usize,
}
pub trait Fetch: Send + Sync {
    fn get(&self, request: &FetchRequest) -> Result<Vec<u8>, PortError>;
}
