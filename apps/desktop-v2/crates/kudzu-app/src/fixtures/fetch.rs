//! Configured responses only; never reaches the network.
use crate::{
    error::PortError,
    ports::{Fetch, FetchRequest},
};
use std::{collections::BTreeMap, sync::Mutex};
#[derive(Default)]
pub struct FakeFetch {
    responses: Mutex<BTreeMap<String, Result<Vec<u8>, PortError>>>,
    pub requests: Mutex<Vec<FetchRequest>>,
}
impl FakeFetch {
    pub fn respond(&self, url: &str, response: Result<Vec<u8>, PortError>) {
        self.responses.lock().unwrap().insert(url.into(), response);
    }
}
impl Fetch for FakeFetch {
    fn get(&self, request: &FetchRequest) -> Result<Vec<u8>, PortError> {
        self.requests.lock().unwrap().push(request.clone());
        if request.timeout.is_zero() {
            return Err(PortError::TimedOut);
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
        Ok(response)
    }
}
