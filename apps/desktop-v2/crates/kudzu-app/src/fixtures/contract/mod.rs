//! Contract suites, one file per port, generic over the implementation: each fake runs its
//! suite here, and each macOS adapter in `kudzu-system` runs the same functions.
pub mod apps;
pub mod engine;
pub mod pasteboard;
pub mod settings_panes;
pub mod thumbnails;

#[cfg(test)]
mod fakes;
