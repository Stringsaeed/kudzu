//! Protocol tests: the engine's safety contracts, pinned by S39 before S34 restructured it.
//!
//! Each test drives the real coordinator (`macos/engine/`) through its three seams
//! (`super::Platform`): the event stream, the pasteboard and observations. The fakes in
//! `fakes.rs` play a desktop (`desk.rs`) with one target app and its text field
//! (`document.rs`), so a test asserts the full sequence of events the engine posted and the
//! state it left: the text and caret, the clipboard, the listener, what the host was told.
//! Nothing reaches the real desktop or pasteboard, and no test waits in real time.
//!
//! Where a test pins behaviour that deliberately differs from the engine this one was copied
//! from, it says so (D9, FRM-4). Everything else is the copied behaviour as it is.

mod desk;
mod document;
mod fakes;
mod keyboard;
mod panels;
mod rig;

mod focus;
mod forms;
mod guards;
mod held;
mod images;
mod latency;
mod listener;
mod paste;
mod refusals;
mod reports;
mod revalidation;
mod sequences;
mod transactions;
mod undo;
