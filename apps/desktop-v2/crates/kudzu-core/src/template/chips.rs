//! Chip labels and tones for the twelve template tags, plus unknown and verbatim tags.
//!
//! The label is what the editor shows in place of the `{{…}}` source. Verbatim tags (the parser
//! kept the original bytes) are an error and show that source. Any other tag the parser accepted
//! but [`typed`](crate::template::typed) does not recognise is a form chip whose label is the source.

use super::presets::preset_label;
use super::{
    AttributeValue, Key, Tag, TemplateTag, MAX_CLIPBOARD_HISTORY, MAX_DELAY_MS, MAX_KEY_COUNT,
};

/// Colour group for a chip. Names match the Tauri app's `ChipTone`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Tone {
    Time,
    Clipboard,
    Cursor,
    Snippet,
    Form,
    Error,
    Key,
}

impl Tone {
    /// The tone name stored in fixtures and used as a CSS suffix.
    pub fn name(self) -> &'static str {
        match self {
            Tone::Time => "time",
            Tone::Clipboard => "clipboard",
            Tone::Cursor => "cursor",
            Tone::Snippet => "snippet",
            Tone::Form => "form",
            Tone::Error => "error",
            Tone::Key => "key",
        }
    }

    /// The tone named by [`Tone::name`], if `name` is one of those seven.
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "time" => Tone::Time,
            "clipboard" => Tone::Clipboard,
            "cursor" => Tone::Cursor,
            "snippet" => Tone::Snippet,
            "form" => Tone::Form,
            "error" => Tone::Error,
            "key" => Tone::Key,
            _ => return None,
        })
    }
}

/// Rune's `data-pill` value for a chip: `{tone}:{source}`, for example `time:{{date}}`.
///
/// This is the id that survives a rich copy. It is not [`crate::template::doc::PillId`].
/// `occurrence` is 1 for the first chip in the field with this tone and source. A later
/// identical chip is `{tone}:{source}#{occurrence}` so two pills in one HTML fragment stay
/// distinct. [`parse_html_pill_id`] strips that `#n` (n ≥ 2).
pub fn html_pill_id(tone: Tone, source: &str, occurrence: u32) -> String {
    let base = format!("{}:{source}", tone.name());
    if occurrence <= 1 {
        base
    } else {
        format!("{base}#{occurrence}")
    }
}

/// `(tone, tag source)` from a [`html_pill_id`]. `None` when the id is not one tag.
///
/// A trailing `#n` (n ≥ 2, no leading zero) is a duplicate marker and is stripped when the
/// shorter string is a single tag and the longer one is not. A tag source always ends at `}}`,
/// so the marker cannot be confused with a source that itself ends in `#n`.
pub fn parse_html_pill_id(id: &str) -> Option<(Tone, String)> {
    let (name, body) = id.split_once(':')?;
    let tone = Tone::from_name(name)?;
    let source = strip_duplicate_suffix(body).to_string();
    is_single_tag(&source).then_some((tone, source))
}

fn strip_duplicate_suffix(body: &str) -> &str {
    let Some((head, tail)) = body.rsplit_once('#') else {
        return body;
    };
    let duplicate = !tail.is_empty()
        && tail != "1"
        && !tail.starts_with('0')
        && tail.bytes().all(|b| b.is_ascii_digit())
        && head.ends_with('}');
    if duplicate && is_single_tag(head) && !is_single_tag(body) {
        head
    } else {
        body
    }
}

/// `source` is exactly one tag covering the whole string.
pub(super) fn is_single_tag(source: &str) -> bool {
    let parsed = super::parse(source);
    match parsed.nodes.as_slice() {
        [super::Node::Tag(tag)] => {
            tag.range.start == 0 && tag.range.end == source.encode_utf16().count()
        }
        _ => false,
    }
}

/// Whether a chip is a tag or an image standing in for a literal run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PillKind {
    Tag,
    Image,
}

/// Label and tone for `tag`. `source` is the exact `{{…}}` slice.
///
/// `snippet_name` resolves a nested snippet id (hyphenated UUID text) to a library name.
/// Without a name the label is `↳ Missing snippet`.
pub fn chip_label(tag: &Tag, source: &str, snippet_name: &dyn Fn(&str) -> Option<&str>) -> String {
    if tag.verbatim.is_some() {
        return source.to_string();
    }
    match super::typed(tag) {
        TemplateTag::Date { format, .. } => dated("Date", &format, attr_str(tag, "offset")),
        TemplateTag::Time { format, .. } => dated("Time", &format, attr_str(tag, "offset")),
        TemplateTag::Clipboard { trim, history } => clipboard(trim, history),
        TemplateTag::Selection { trim } => if trim {
            "Selection (trimmed)"
        } else {
            "Selection"
        }
        .into(),
        TemplateTag::Uuid { upper } => if upper { "UUID (capitals)" } else { "UUID" }.into(),
        TemplateTag::Random { options } => {
            let n = options.len();
            format!(
                "Random \u{00b7} {n} {}",
                if n == 1 { "option" } else { "options" }
            )
        }
        TemplateTag::Key { key, count } => key_label(key, count),
        TemplateTag::Delay { ms } => format!("Pause {}", format_delay(ms)),
        TemplateTag::Cursor => "Cursor".into(),
        TemplateTag::Snippet { id } => {
            format!(
                "\u{21b3} {}",
                snippet_name(&id.to_string()).unwrap_or("Missing snippet")
            )
        }
        TemplateTag::Field {
            name, multiline, ..
        } => {
            if multiline {
                format!("{name} \u{00b6}")
            } else {
                name
            }
        }
        TemplateTag::Choice { name, .. } => format!("{name} \u{25be}"),
        TemplateTag::Unknown => source.to_string(),
    }
}

/// Tone for `tag`. Verbatim sources are [`Tone::Error`]; other unknown tags are [`Tone::Form`].
pub fn chip_tone(tag: &Tag) -> Tone {
    if tag.verbatim.is_some() {
        return Tone::Error;
    }
    match super::typed(tag) {
        TemplateTag::Date { .. }
        | TemplateTag::Time { .. }
        | TemplateTag::Uuid { .. }
        | TemplateTag::Random { .. } => Tone::Time,
        TemplateTag::Clipboard { .. } | TemplateTag::Selection { .. } => Tone::Clipboard,
        TemplateTag::Key { .. } | TemplateTag::Delay { .. } => Tone::Key,
        TemplateTag::Cursor => Tone::Cursor,
        TemplateTag::Snippet { .. } => Tone::Snippet,
        TemplateTag::Field { .. } | TemplateTag::Choice { .. } | TemplateTag::Unknown => Tone::Form,
    }
}

fn dated(kind: &str, format: &str, offset: Option<&str>) -> String {
    let base = preset_label(format).unwrap_or(format);
    match offset.filter(|s| !s.is_empty()) {
        Some(off) => format!("{kind} \u{00b7} {base} {off}"),
        None => format!("{kind} \u{00b7} {base}"),
    }
}

fn clipboard(trim: bool, history: u32) -> String {
    let history = history.min(MAX_CLIPBOARD_HISTORY);
    let base = match history {
        0 => "Clipboard".to_string(),
        1 => "Previous copy".to_string(),
        n => format!("Clipboard \u{00b7} {n} back"),
    };
    if trim {
        format!("{base} (trimmed)")
    } else {
        base
    }
}

fn key_label(key: Key, count: u32) -> String {
    let (label, cap) = match key {
        Key::Tab => ("Tab", "\u{21e5}"),
        Key::Return => ("Return", "\u{21b5}"),
        Key::Escape => ("Escape", "esc"),
        Key::Space => ("Space", "Space"),
        Key::Backspace => ("Backspace", "\u{232b}"),
        Key::Delete => ("Forward delete", "\u{2326}"),
        Key::Left => ("Left", "\u{2190}"),
        Key::Right => ("Right", "\u{2192}"),
        Key::Up => ("Up", "\u{2191}"),
        Key::Down => ("Down", "\u{2193}"),
    };
    let count = count.clamp(1, MAX_KEY_COUNT);
    // A one-character cap is drawn before the word. "esc" and "Space" already say what they are.
    let shown = if cap == label || cap.chars().count() > 1 {
        label.to_string()
    } else {
        format!("{cap} {label}")
    };
    if count > 1 {
        format!("{shown} \u{00d7}{count}")
    } else {
        shown
    }
}

/// "300 ms", "1.5 s". Matches `Number((ms / 1000).toFixed(2))` in the Tauri app.
pub fn format_delay(ms: u32) -> String {
    let ms = ms.clamp(1, MAX_DELAY_MS);
    if ms < 1000 {
        return format!("{ms} ms");
    }
    let secs = (f64::from(ms) / 1000.0 * 100.0).round() / 100.0;
    let text = format!("{secs:.2}");
    let text = text.trim_end_matches('0').trim_end_matches('.');
    format!("{text} s")
}

fn attr_str<'a>(tag: &'a Tag, key: &str) -> Option<&'a str> {
    tag.attributes
        .iter()
        .rev()
        .find(|a| a.name == key)
        .and_then(|a| match &a.value {
            AttributeValue::String(s) => Some(s.as_str()),
            _ => None,
        })
}
