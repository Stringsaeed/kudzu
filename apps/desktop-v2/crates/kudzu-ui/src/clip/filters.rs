//! Labels, counts and glyphs for the clipboard kind filters. Which folders exist, their order
//! and their names are `kudzu_core`'s (`CLIP_FOLDERS`, `clip_filter_label`); this adds what the
//! list and the picker's pills say about them.

use kudzu_core::clipboard::query::{clip_filter_label, ClipFilter, CLIP_FOLDERS};
use kudzu_core::clipboard::History;
use rune::Icon;

use crate::theme::icons;

/// The picker's pill text: "All" where the sidebar says "History".
pub fn pill_label(filter: ClipFilter) -> &'static str {
    if filter == ClipFilter::All {
        "All"
    } else {
        clip_filter_label(filter)
    }
}

/// The heading over the list: "Clipboard" for everything, else the folder's name.
pub fn heading(filter: ClipFilter) -> &'static str {
    if filter == ClipFilter::All {
        "Clipboard"
    } else {
        clip_filter_label(filter)
    }
}

/// How many entries each folder holds, in `CLIP_FOLDERS` order.
pub fn counts(history: &History) -> [usize; 8] {
    CLIP_FOLDERS.map(|f| history.entries.iter().filter(|e| f.contains(e)).count())
}

/// The glyph over an empty folder: its kind's (`KIND_ICON`), the pin for Pinned, the list for
/// History.
pub fn glyph(filter: ClipFilter) -> &'static Icon {
    match filter {
        ClipFilter::All => &icons::CLIPBOARD_LIST,
        ClipFilter::Pinned => &icons::PIN,
        ClipFilter::Text | ClipFilter::Color => &icons::ALIGN_LEFT,
        ClipFilter::Code => &icons::BRACES,
        ClipFilter::Link => &icons::GLOBE,
        ClipFilter::Image => &icons::IMAGE,
        ClipFilter::File => &icons::FILE,
        ClipFilter::Other => &icons::PACKAGE,
    }
}

/// "No images yet": what an empty folder says.
pub fn nothing_yet(filter: ClipFilter) -> String {
    format!("No {} yet", pill_label(filter).to_lowercase())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clip::model::tests::{clip, history};

    #[test]
    fn headings_and_empty_copy_follow_the_folder() {
        assert_eq!(heading(ClipFilter::All), "Clipboard");
        assert_eq!(heading(ClipFilter::Image), "Images");
        assert_eq!(pill_label(ClipFilter::All), "All");
        assert_eq!(nothing_yet(ClipFilter::Pinned), "No pinned yet");
        assert_eq!(nothing_yet(ClipFilter::Color), "No colors yet");
    }

    #[test]
    fn each_kind_folder_has_its_own_glyph() {
        assert_eq!(glyph(ClipFilter::Image).name, icons::IMAGE.name);
        assert_eq!(glyph(ClipFilter::Link).name, icons::GLOBE.name);
        assert_eq!(glyph(ClipFilter::Pinned).name, icons::PIN.name);
        assert_eq!(glyph(ClipFilter::All).name, icons::CLIPBOARD_LIST.name);
    }

    #[test]
    fn counts_follow_the_folders() {
        let h = history(vec![clip("a", 1, true), clip("b", 2, false)]);
        let c = counts(&h);
        assert_eq!((c[0], c[1], c[2], c[3]), (2, 1, 2, 0));
    }
}
