//! Clipboard history widgets shared by the main window (S58) and the picker (S60). Nothing in
//! here names `MainState` or any window state: views are generic over the state and reach their
//! model through an accessor.
//!
//! - `model`: the list model (rows, active row, selection).
//! - `filters`: the kind filters, their labels and counts.
//! - `motion`: which rows animate (the change census).
//! - `tile`, `list`: the row's leading visual and the virtual list.
//!
//! S59 adds `preview`, `facts`, `timeline`, `secret`, `large` and `content/**` beside these.

pub mod content;
pub mod facts;
pub mod filters;
pub mod large;
pub mod list;
pub mod model;
pub mod motion;
pub mod preview;
pub mod secret;
pub mod tile;
pub mod timeline;
