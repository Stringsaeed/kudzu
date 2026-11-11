//! Image rules for the editor.
//!
//! A data URL is a PNG or JPEG under 196,608 characters ([`crate::formatting::safe_image`]).
//! One snippet holds at most 524,288 characters of those URLs. Widths the user sets are clamped
//! to 16..=512. Alt text is trimmed, `{`, `}`, and line breaks become spaces, an empty result
//! becomes "Image", and the result is at most 200 UTF-16 units.
//!
//! Loading a saved span is stricter about what already exists and looser about width: a stored
//! width of 1..=512 is kept. Clamping to 16 applies when the user changes the width.

use crate::formatting::safe_image;

/// Why an image edit was refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ImageError {
    /// The data URL is not a PNG or JPEG [`safe_image`] accepts.
    #[error("Choose a PNG or JPEG image.")]
    UnsafeData,
    /// The snippet would hold more than [`MAX_SNIPPET_IMAGE_LEN`] characters of data URLs.
    #[error("This snippet has too many images. Remove an image or use a smaller one.")]
    TooMany,
    /// No image in the document has that pill.
    #[error("That image isn't in the snippet.")]
    Missing,
}

/// Longest data URL [`crate::formatting::safe_image`] accepts.
pub const MAX_DATA_URL_LEN: usize = 196_608;
/// Longest total of image data URLs in one snippet.
pub const MAX_SNIPPET_IMAGE_LEN: usize = 524_288;
/// Narrowest width the alt/width form allows.
pub const MIN_IMAGE_WIDTH: u32 = 16;
/// Widest width the form allows. Compaction also stops at 512 px on a side.
pub const MAX_IMAGE_WIDTH: u32 = 512;
/// `maxLength` of the text-alternative field.
pub const MAX_ALT_UNITS: usize = 200;

/// Whether `data_url` may be embedded. Delegates to [`safe_image`].
pub fn accept_data_url(data_url: &str) -> bool {
    safe_image(data_url)
}

/// `used` is the sum of data-URL lengths that will remain after this insert.
pub fn within_budget(used: usize) -> bool {
    used <= MAX_SNIPPET_IMAGE_LEN
}

/// Clamp a width the user typed. `0` and anything below 16 become 16.
pub fn clamp_width(width: u32) -> u32 {
    width.clamp(MIN_IMAGE_WIDTH, MAX_IMAGE_WIDTH)
}

/// A stored span width. `None` when the value is outside 1..=512.
pub fn stored_width(width: u32) -> Option<u32> {
    (1..=MAX_IMAGE_WIDTH).contains(&width).then_some(width)
}

/// Sanitise alt text the way the insert form does, without the surrounding brackets.
pub fn sanitize_alt(raw: &str) -> String {
    let mut cleaned = String::new();
    for c in trim_js(raw).chars() {
        if c == '{' || c == '}' || c == '\n' || c == '\r' {
            cleaned.push(' ');
        } else {
            cleaned.push(c);
        }
    }
    let text = if cleaned.is_empty() {
        "Image"
    } else {
        &cleaned
    };
    utf16_prefix(text, MAX_ALT_UNITS)
}

/// The literal the insert form writes: `[` + sanitised alt + `]`.
pub fn alt_literal(raw: &str) -> String {
    format!("[{}]", sanitize_alt(raw))
}

fn trim_js(text: &str) -> &str {
    let is_space = |c: char| {
        matches!(
            c,
            '\u{0009}'..='\u{000d}'
                | '\u{0020}'
                | '\u{00a0}'
                | '\u{1680}'
                | '\u{2000}'..='\u{200a}'
                | '\u{2028}'
                | '\u{2029}'
                | '\u{202f}'
                | '\u{205f}'
                | '\u{3000}'
                | '\u{feff}'
        )
    };
    let mut start = 0;
    let mut end = text.len();
    for (i, c) in text.char_indices() {
        if is_space(c) {
            start = i + c.len_utf8();
        } else {
            break;
        }
    }
    for (i, c) in text.char_indices().rev() {
        if i < start {
            break;
        }
        if is_space(c) {
            end = i;
        } else {
            break;
        }
    }
    if start >= end {
        ""
    } else {
        &text[start..end]
    }
}

fn utf16_prefix(text: &str, max: usize) -> String {
    let mut units = 0;
    let mut end = 0;
    for c in text.chars() {
        let n = c.len_utf16();
        if units + n > max {
            break;
        }
        units += n;
        end += c.len_utf8();
    }
    text[..end].to_string()
}
