//! Headless `--window main --scene <name>` registry (`PLAN.md` §1.2, §2.2).
//!
//! Append-only: one entry per line so parallel Wave 9+ screen steps (`S42`, `S43`, `S50`–`S65`) add
//! their reference scenes without colliding and delete their corresponding `NOT_YET_BUILT` entries
//! in `xtask/src/parity/reference.rs`.

use super::onboarding::scenes as onboarding;
use crate::shell::MainState;

pub type Scene = (&'static str, fn(&mut MainState));

pub const SCENES: &[Scene] = &[
    ("onboarding-welcome", onboarding::welcome),
    ("onboarding-access", onboarding::access),
    ("onboarding-import", onboarding::import),
    ("onboarding-practice", onboarding::practice),
    ("onboarding-espanso", onboarding::espanso),
    ("onboarding-espanso-retry", onboarding::espanso_retry),
    ("onboarding-finish-retry", onboarding::finish_retry),
    ("library", crate::main::library::scenes::library),
    ("library-empty", crate::main::library::scenes::library_empty),
    ("snippet-bulk", crate::main::library::scenes::snippet_bulk),
    (
        "snippet-deleted-toast",
        crate::main::library::scenes::snippet_deleted_toast,
    ),
    ("library-5000", crate::main::library::scenes::library_5000),
];
