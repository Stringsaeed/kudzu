//! Configured responses only; never reaches the network.
use crate::{
    error::PortError,
    ports::{Fetch, FetchRequest},
};
use std::sync::mpsc;
use std::sync::Mutex;
use std::{collections::BTreeMap, sync::atomic::Ordering};

struct Block {
    started: mpsc::Sender<()>,
    release: mpsc::Receiver<()>,
}

struct Pause {
    url: Option<String>,
    reported: mpsc::Sender<(u64, Option<u64>)>,
    resume: mpsc::Receiver<()>,
}

#[derive(Default)]
pub struct FakeFetch {
    responses: Mutex<BTreeMap<String, Result<Vec<u8>, PortError>>>,
    pub requests: Mutex<Vec<FetchRequest>>,
    /// `None` delivers the body in one progress event.
    pub chunk: Mutex<Option<usize>>,
    pub unknown_total: Mutex<bool>,
    block: Mutex<Option<Block>>,
    pause: Mutex<Option<Pause>>,
}

impl FakeFetch {
    pub fn respond(&self, url: &str, response: Result<Vec<u8>, PortError>) {
        self.responses.lock().unwrap().insert(url.into(), response);
    }

    /// The next `get` waits until the returned sender is used. The receiver fires when `get` starts.
    pub fn block_next(&self) -> (mpsc::Receiver<()>, mpsc::Sender<()>) {
        let (started_tx, started_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        *self.block.lock().unwrap() = Some(Block {
            started: started_tx,
            release: release_rx,
        });
        (started_rx, release_tx)
    }

    /// After the first progress callback, `get` waits until the returned sender is used.
    pub fn pause_after_progress(&self) -> (mpsc::Receiver<(u64, Option<u64>)>, mpsc::Sender<()>) {
        self.pause_after_progress_url(None)
    }

    /// Like [`pause_after_progress`], limited to `url` when it is `Some`.
    pub fn pause_after_progress_url(
        &self,
        url: Option<&str>,
    ) -> (mpsc::Receiver<(u64, Option<u64>)>, mpsc::Sender<()>) {
        let (reported_tx, reported_rx) = mpsc::channel();
        let (resume_tx, resume_rx) = mpsc::channel();
        *self.pause.lock().unwrap() = Some(Pause {
            url: url.map(str::to_owned),
            reported: reported_tx,
            resume: resume_rx,
        });
        (reported_rx, resume_tx)
    }
}

impl Fetch for FakeFetch {
    fn get(
        &self,
        request: &FetchRequest,
        progress: &dyn Fn(u64, Option<u64>),
    ) -> Result<Vec<u8>, PortError> {
        self.requests.lock().unwrap().push(request.clone());
        if request.timeout.is_zero() {
            return Err(PortError::TimedOut);
        }
        if request.cancel.load(Ordering::Acquire) {
            return Err(PortError::Cancelled);
        }
        if let Some(block) = self.block.lock().unwrap().take() {
            let _ = block.started.send(());
            let _ = block.release.recv();
        }
        if request.cancel.load(Ordering::Acquire) {
            return Err(PortError::Cancelled);
        }
        let response = self
            .responses
            .lock()
            .unwrap()
            .get(&request.url)
            .cloned()
            .unwrap_or(Err(PortError::Unavailable))?;
        if response.len() > request.max_bytes {
            return Err(PortError::TooLarge);
        }
        let unknown = *self.unknown_total.lock().unwrap();
        let chunk = self.chunk.lock().unwrap().unwrap_or(response.len().max(1));
        let chunk = chunk.max(1);
        let total = if unknown {
            None
        } else {
            Some(response.len() as u64)
        };
        let mut sent = 0usize;
        while sent < response.len() {
            if request.cancel.load(Ordering::Acquire) {
                return Err(PortError::Cancelled);
            }
            sent = (sent + chunk).min(response.len());
            progress(sent as u64, total);
            let pause = {
                let mut slot = self.pause.lock().unwrap();
                match slot.as_ref() {
                    Some(pause) if pause.url.as_deref().is_none_or(|url| url == request.url) => {
                        slot.take()
                    }
                    _ => None,
                }
            };
            if let Some(pause) = pause {
                let _ = pause.reported.send((sent as u64, total));
                let _ = pause.resume.recv();
                if request.cancel.load(Ordering::Acquire) {
                    return Err(PortError::Cancelled);
                }
            }
        }
        Ok(response)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use std::time::Duration;

    #[test]
    fn progress_reports_bytes_and_known_total() {
        let fetch = FakeFetch::default();
        fetch.respond("https://example.test/a", Ok((0..10).collect()));
        *fetch.chunk.lock().unwrap() = Some(4);
        let events = Mutex::new(Vec::new());
        let body = fetch
            .get(
                &FetchRequest::new("https://example.test/a", Duration::from_secs(1), 100),
                &|downloaded, total| events.lock().unwrap().push((downloaded, total)),
            )
            .unwrap();
        assert_eq!(body.len(), 10);
        assert_eq!(
            *events.lock().unwrap(),
            [(4, Some(10)), (8, Some(10)), (10, Some(10))]
        );
    }

    #[test]
    fn unknown_total_omits_size() {
        let fetch = FakeFetch::default();
        fetch.respond("https://example.test/a", Ok(vec![1, 2, 3, 4]));
        *fetch.chunk.lock().unwrap() = Some(2);
        *fetch.unknown_total.lock().unwrap() = true;
        let events = Mutex::new(Vec::new());
        fetch
            .get(
                &FetchRequest::new("https://example.test/a", Duration::from_secs(1), 100),
                &|downloaded, total| events.lock().unwrap().push((downloaded, total)),
            )
            .unwrap();
        assert_eq!(*events.lock().unwrap(), [(2, None), (4, None)]);
    }

    #[test]
    fn cancel_mid_download_returns_cancelled() {
        let fetch = FakeFetch::default();
        fetch.respond("https://example.test/a", Ok(vec![0; 8]));
        *fetch.chunk.lock().unwrap() = Some(2);
        let cancel = Arc::new(AtomicBool::new(false));
        let mut request = FetchRequest::new("https://example.test/a", Duration::from_secs(1), 100);
        request.cancel = cancel.clone();
        let result = fetch.get(&request, &|downloaded, _| {
            if downloaded >= 2 {
                cancel.store(true, Ordering::Release);
            }
        });
        assert_eq!(result, Err(PortError::Cancelled));
    }
}
