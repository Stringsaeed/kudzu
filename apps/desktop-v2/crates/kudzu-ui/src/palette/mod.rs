//! Quick Search: the query, the list, the keys, and when the preview moves.
mod keys;
mod notice;
mod place;
mod preview;
mod query;
mod rank;

pub mod model;
pub mod scenes;
pub mod view;

mod drive;

pub(crate) use drive::{apply, clamped, on_input_key, on_key, pick, place, poll, setup};
pub use keys::{Effect, PaletteKeys};
pub use notice::{classify, NoticeKind};
pub use place::{display_local_origin, FRACTION, HEIGHT, WIDTH};
pub use preview::{PreviewClock, PreviewTick, COPY_HIDE_AFTER, DIM_AFTER, START_AFTER};
pub use query::{create_seed, split_query, CreateAs, CreateSeed, RESULT_LIMIT};
pub use rank::{form_badge, kind_of, present, used_line, Kind, Presentation, Row, Scope};
