//! Blocking HTTP, with mandatory deadline and response cap enforced while reading.
use crate::error::PortError;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::Duration;

/// Shared flag the caller sets to abandon an in-flight `get`.
pub type Cancel = Arc<AtomicBool>;

#[derive(Clone)]
pub struct FetchRequest {
    pub url: String,
    pub timeout: Duration,
    pub max_bytes: usize,
    pub cancel: Cancel,
}

impl FetchRequest {
    pub fn new(url: impl Into<String>, timeout: Duration, max_bytes: usize) -> Self {
        Self {
            url: url.into(),
            timeout,
            max_bytes,
            cancel: Cancel::default(),
        }
    }
}

pub trait Fetch: Send + Sync {
    /// `progress` is `(bytes_so_far, total_if_known)`.
    fn get(
        &self,
        request: &FetchRequest,
        progress: &dyn Fn(u64, Option<u64>),
    ) -> Result<Vec<u8>, PortError>;
}
