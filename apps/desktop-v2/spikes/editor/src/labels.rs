//! Chip labels and tones for the twelve template tags, copied from the editor's
//! label table. The spike does not read the frozen app.

use kudzu_core::template::{
    AttributeValue, Key, Tag, TemplateTag, MAX_CLIPBOARD_HISTORY, MAX_DELAY_MS, MAX_KEY_COUNT,
};

/// `time`, `clipboard`, `cursor`, `snippet`, `form`, `error`, `key`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
}

const DATE_PRESETS: &[(&str, &str)] = &[
    ("yyyy-MM-dd", "ISO date"),
    ("MMMM d, yyyy", "Long date"),
    ("MMM d, yyyy", "Medium date"),
    ("M/d/yy", "Short date"),
    ("d MMMM yyyy", "Day month year"),
    ("EEEE", "Weekday"),
    ("EEEE, MMMM d", "Weekday + date"),
    ("HH:mm", "24-hour time"),
    ("h:mm a", "12-hour time"),
    ("HH:mm:ss", "Time with seconds"),
    ("yyyy-MM-dd'T'HH:mm:ssXXX", "ISO timestamp"),
];

pub fn date_presets() -> &'static [(&'static str, &'static str)] {
    DATE_PRESETS
}

/// Human label and tone for `tag`. `source` is the exact `{{…}}` slice.
/// A nested snippet with no name lookup reads `↳ Missing snippet`.
pub fn chip_face(tag: &Tag, source: &str) -> (String, Tone) {
    chip_face_named(tag, source, &|_| None)
}

/// [`chip_face`] with a snippet-id lookup. The id is the hyphenated UUID.
pub fn chip_face_named(
    tag: &Tag,
    source: &str,
    snippet_name: &dyn Fn(&str) -> Option<&str>,
) -> (String, Tone) {
    if tag.verbatim.is_some() {
        return (source.to_string(), Tone::Error);
    }
    let typed = kudzu_core::template::typed(tag);
    let label = match &typed {
        TemplateTag::Date { format, .. } => dated("Date", format, attr_str(tag, "offset")),
        TemplateTag::Time { format, .. } => dated("Time", format, attr_str(tag, "offset")),
        TemplateTag::Clipboard { trim, history } => clipboard(*trim, *history),
        TemplateTag::Selection { trim } => {
            if *trim {
                "Selection (trimmed)".into()
            } else {
                "Selection".into()
            }
        }
        TemplateTag::Uuid { upper } => {
            if *upper {
                "UUID (capitals)".into()
            } else {
                "UUID".into()
            }
        }
        TemplateTag::Random { options } => {
            let n = options.len();
            format!(
                "Random \u{00b7} {n} {}",
                if n == 1 { "option" } else { "options" }
            )
        }
        TemplateTag::Key { key, count } => key_label(*key, *count),
        TemplateTag::Delay { ms } => format!("Pause {}", format_delay(*ms)),
        TemplateTag::Cursor => "Cursor".into(),
        TemplateTag::Snippet { id } => format!(
            "↳ {}",
            snippet_name(&id.to_string()).unwrap_or("Missing snippet")
        ),
        TemplateTag::Field {
            name, multiline, ..
        } => {
            if *multiline {
                format!("{name} ¶")
            } else {
                name.clone()
            }
        }
        TemplateTag::Choice { name, .. } => format!("{name} ▾"),
        TemplateTag::Unknown => source.to_string(),
    };
    (label, tone_of(tag, &typed))
}

fn dated(kind: &str, format: &str, offset: Option<&str>) -> String {
    let base = DATE_PRESETS
        .iter()
        .find(|(k, _)| *k == format)
        .map(|(_, v)| *v)
        .unwrap_or(format);
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
        Key::Return => ("Return", "\u{21a9}"),
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

fn format_delay(ms: u32) -> String {
    let ms = ms.clamp(1, MAX_DELAY_MS);
    if ms < 1000 {
        format!("{ms} ms")
    } else {
        let secs = (ms as f64 / 1000.0 * 100.0).round() / 100.0;
        let text = format!("{secs:.2}");
        let text = text.trim_end_matches('0').trim_end_matches('.');
        format!("{text} s")
    }
}

fn tone_of(tag: &Tag, typed: &TemplateTag) -> Tone {
    if tag.verbatim.is_some() {
        return Tone::Error;
    }
    match typed {
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
