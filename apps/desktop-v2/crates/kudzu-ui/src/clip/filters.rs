//! Labels and counts for the clipboard kind filters. Which folders exist, their order and
//! their names are the frame's (`main::state::{CLIP_FOLDERS, clip_filter_label}`); this adds
//! what the list and the picker's pills say about them.

use kudzu_core::clipboard::query::ClipFilter;
use kudzu_core::clipboard::History;

use crate::main::state::{clip_filter_label, CLIP_FOLDERS};

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
    fn counts_follow_the_folders() {
        let h = history(vec![clip("a", 1, true), clip("b", 2, false)]);
        let c = counts(&h);
        assert_eq!((c[0], c[1], c[2], c[3]), (2, 1, 2, 0));
    }
}
