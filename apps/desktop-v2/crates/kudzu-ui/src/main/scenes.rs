//! Headless `--window main --scene <name>` registry (`PLAN.md` §1.2, §2.2).
//!
//! Append-only: one entry per line so parallel Wave 9+ screen steps (`S42`, `S43`, `S50`–`S65`) add
//! their reference scenes without colliding and delete their corresponding `NOT_YET_BUILT` entries
//! in `xtask/src/parity/reference.rs`.

use crate::shell::MainState;

pub type Scene = (&'static str, fn(&mut MainState));

pub const SCENES: &[Scene] = &[];
