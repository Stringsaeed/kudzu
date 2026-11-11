//! Repeatable, instance-local UUID stream for scenes and service tests.
use crate::ports::IdSource;
use std::sync::Mutex;
use uuid::Uuid;
pub struct FixedIds(Mutex<u128>);
impl FixedIds {
    pub fn new(seed: u128) -> Self {
        Self(Mutex::new(seed))
    }
}
impl IdSource for FixedIds {
    fn next_id(&self) -> Uuid {
        let mut next = self.0.lock().unwrap();
        let id = Uuid::from_u128(*next);
        *next = next.checked_add(1).expect("fixture ids exhausted");
        id
    }
}
