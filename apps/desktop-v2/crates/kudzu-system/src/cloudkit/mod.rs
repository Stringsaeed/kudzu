//! Instance-owned CloudKit transport. The sync service owns policy and persistence.
//!
//! S40 constructs one `CloudKit` for its sync service. Port calls run on that service's
//! worker; callbacks only enqueue. `stop` fences and drains callbacks before cancelling
//! CloudKit. A subsequent start waits for cancellation before creating another engine.
//! No container is accessed until the process entitlement has been checked.

mod delegate;
mod delivery;
mod entitlement;
mod errors;
mod native;
mod pending;
mod records;
mod sending;
mod wiring;

pub use wiring::CloudKit;

#[cfg(test)]
mod tests;
