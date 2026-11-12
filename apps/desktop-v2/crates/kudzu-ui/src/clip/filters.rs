//! Labels, counts and glyphs for the clipboard kind filters. Which folders exist, their order
//! and their names are `kudzu_core`'s (`CLIP_FOLDERS`, `clip_filter_label`); this adds what the
//! list and the picker's pills say about them.

use kudzu_core::clipboard::query::{clip_filter_key, clip_filter_label, ClipFilter, CLIP_FOLDERS};
use kudzu_core::clipboard::History;
use rune::{row, text, El, Icon, Tx};

use crate::theme::{icons, radius, Theme};

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

/// The kinds the picker has pills for (`PICKER_FILTERS`); Code and Colors are found by query.
pub const PICKER_FILTERS: [ClipFilter; 6] = [
    ClipFilter::All,
    ClipFilter::Pinned,
    ClipFilter::Text,
    ClipFilter::Image,
    ClipFilter::Link,
    ClipFilter::File,
];

/// The next (or previous) pill, wrapping: Tab and Shift-Tab in the picker. A filter that has
/// no pill starts from the first.
pub fn cycle(current: ClipFilter, backwards: bool) -> ClipFilter {
    let n = PICKER_FILTERS.len();
    let at = PICKER_FILTERS
        .iter()
        .position(|f| *f == current)
        .unwrap_or(0);
    PICKER_FILTERS[(at + if backwards { n - 1 } else { 1 }) % n]
}

/// Kind filters as one single-select row of pills (a radio group), on a well. The pills never
/// take keyboard focus: Tab cycles them from the search field. Element keys are
/// `<key>-<filter key>`.
pub fn pills<S: 'static>(
    theme: &Theme,
    key: &str,
    value: ClipFilter,
    on_select: fn(&mut S, ClipFilter, &mut Tx),
) -> El {
    let c = &theme.c;
    let track = theme.depth.well.iter().fold(
        row()
            .key(key.to_owned())
            .role("radiogroup")
            .label("Show")
            .shrink()
            .padding(2.0)
            .center_y()
            .gap(4.0)
            .radius(radius::XL4)
            .fill(c.foreground.alpha(0.035)),
        |track, shadow| track.box_shadow(*shadow),
    );
    track.children(PICKER_FILTERS.into_iter().map(|filter| {
        let on = filter == value;
        let label = pill_label(filter);
        // The radio itself stays unfilled (a radio's computed background is transparent); the
        // selected pill is the layer behind it, as the sliding indicator is in the source.
        let radio = row()
            .key(format!("{key}-{}", clip_filter_key(filter)))
            .role("radio")
            .label(label)
            .checked(on)
            .h(24.0)
            .px(10.0)
            .center()
            .preserve_focus_on_press(true)
            .child(
                text(label)
                    .font(rune::TextStyle::new(11.5, 500.0))
                    .color(if on { c.foreground } else { c.muted_foreground }),
            )
            .on_press_tx(move |s: &mut S, tx: &mut Tx| on_select(s, filter, tx));
        let mut pill = row().h(24.0).radius(12.0).child(radio);
        if on {
            pill = theme
                .depth
                .soft
                .iter()
                .fold(pill.fill(c.surface_raised), |r, shadow| {
                    r.box_shadow(*shadow)
                });
        }
        pill
    }))
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
    fn tab_cycles_the_six_pills_both_ways_and_wraps() {
        assert_eq!(cycle(ClipFilter::All, false), ClipFilter::Pinned);
        assert_eq!(cycle(ClipFilter::File, false), ClipFilter::All);
        assert_eq!(cycle(ClipFilter::All, true), ClipFilter::File);
        assert_eq!(cycle(ClipFilter::Link, true), ClipFilter::Image);
        assert_eq!(
            cycle(ClipFilter::Code, false),
            ClipFilter::Pinned,
            "no pill: from the first"
        );
        assert_eq!(
            PICKER_FILTERS.map(pill_label),
            ["All", "Pinned", "Text", "Images", "Links", "Files"]
        );
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
