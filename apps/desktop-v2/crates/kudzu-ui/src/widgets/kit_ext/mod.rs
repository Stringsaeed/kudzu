//! Where a rune-kit component cannot reach a metric through `Kit` (PLAN §1.4, §3 gap 17).
//!
//! Only buttons, inputs and switches must match exactly (§6 D6); every other primitive is
//! rune-kit's component with Kudzu's `Kit` (`theme::kit`), as it is. Each module here has a row
//! in `docs/migration/PARITY_MATRIX.md` naming the metric rune-kit could not take:
//!
//! - **decorated** (rune-kit's component, built, then given Kudzu's metrics): [`button`],
//!   [`input`] (`Input` and `InputGroup`).
//! - **wrapper** (built from Rune core, tagged `RUNE-GAP`): [`switch`], [`textarea`].
pub mod button;
pub mod input;
pub mod switch;
pub mod textarea;
