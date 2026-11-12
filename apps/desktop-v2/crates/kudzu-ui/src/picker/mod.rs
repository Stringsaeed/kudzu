//! The clipboard picker (`PLAN.md` S60): a panel over any app. Type to search, see a live
//! preview, paste into the app you came from without the mouse.
//!
//! - `keys`: which key asks for what, as pure data.
//! - `model`: the query, filter, list, footer flash and timers (pure state).
//! - `drive`: applies effects to the window: services, workers, polls, timers.
//! - `view`, `footer`: the layout. `place`: where it sits. `scenes`: headless states.
//!
//! The rows, tiles, pills and previews are `crate::clip`'s, shared with the main window.

pub mod drive;
mod footer;
pub mod keys;
pub mod model;
pub mod place;
mod rows;
pub mod scenes;
pub mod view;

#[cfg(test)]
mod tests;

pub(crate) use drive::{on_key, place, poll};
pub use place::{HEIGHT, WIDTH};
pub use view::view;
