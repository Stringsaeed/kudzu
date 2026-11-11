//! Where a rune-kit component cannot reach a metric through `Kit` (PLAN §1.4, §3 gap 17).
//!
//! Buttons, inputs and switches match exactly (§6 D6), and since S07 so do the select trigger
//! (32 pt), the menu, dialog and toast radii, the raised controls' hover and pressed depth, the
//! switch knob's stretch while held, and the checkbox's checked colours (D6 as widened after
//! S06). Every other primitive is rune-kit's component with Kudzu's `Kit` (`theme::kit`), as it
//! is. Each module here has a row in `docs/migration/PARITY_MATRIX.md` naming the metric
//! rune-kit could not take, and the upstream issue (Rune #573, or #360 for hover and press):
//!
//! - **decorated** (rune-kit's component, built, then given Kudzu's metrics): [`button`],
//!   [`input`] (`Input` and `InputGroup`), [`select`], [`menu`], [`toast`].
//! - **themed** through a copy of the kit: [`checkbox`].
//! - **wrapper** (built from Rune core, tagged `RUNE-GAP`): [`switch`], [`textarea`], [`dialog`].
//! - [`press`]: the hover and press depth layers the decorated controls share.
pub mod button;
pub mod checkbox;
pub mod dialog;
pub mod input;
pub mod menu;
pub mod press;
pub mod select;
pub mod switch;
pub mod textarea;
pub mod toast;
