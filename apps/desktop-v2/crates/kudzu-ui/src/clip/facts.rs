//! The metadata rows under a preview: when, where from, how big, how long it stays
//! (`ClipDetails` in `clip-preview.tsx`). Pure: the view only lays the rows out.

use chrono::{DateTime, FixedOffset};
use kudzu_core::clipboard::facts::{clip_time, exact_time, expires_in, format_bytes, text_facts};
use kudzu_core::clipboard::{ClipEntry, ClipKind};

/// One row of the facts grid.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fact {
    pub label: &'static str,
    pub value: String,
    /// The "From" row: the value is an app name, drawn after its icon.
    pub app: bool,
}

fn fact(label: &'static str, value: impl Into<String>) -> Fact {
    Fact {
        label,
        value: value.into(),
        app: false,
    }
}

/// The rows for `e`. `full` is the complete text once loaded (word counts need it);
/// `compact` is the picker's short list.
pub fn details(
    e: &ClipEntry,
    full: Option<&str>,
    retention_days: u32,
    now: DateTime<FixedOffset>,
    compact: bool,
) -> Vec<Fact> {
    let mut rows = vec![fact("Copied", exact_time(e.copied_at, now))];
    if e.copy_count > 1 {
        rows.push(fact("First copied", exact_time(e.first_copied_at, now)));
        if compact {
            rows.push(fact("Times", format!("{}×", e.copy_count)));
        }
    }
    if let Some(source) = &e.source {
        rows.push(Fact {
            label: "From",
            value: source.name.clone(),
            app: true,
        });
    }
    if matches!(e.kind, ClipKind::Text | ClipKind::Code) {
        rows.push(fact("Length", text_facts(e, full)));
    }
    if let Some(image) = e.image {
        rows.push(fact(
            "Dimensions",
            format!("{} × {} px", image.width, image.height),
        ));
    }
    if e.kind == ClipKind::File {
        rows.push(fact("Items", e.files.len().to_string()));
    }
    if matches!(e.kind, ClipKind::Image | ClipKind::Other) {
        rows.push(fact("Size", format_bytes(e.size)));
    }
    if let (true, Some(at)) = (e.use_count > 0, e.last_used_at) {
        rows.push(fact(
            "Reused",
            format!(
                "{}× · last {}",
                e.use_count,
                clip_time(at, now, false).to_lowercase()
            ),
        ));
    }
    rows.push(fact("Expires", expires_in(e, retention_days, now)));
    rows
}

/// "3 formats", "1 format".
pub fn formats_label(e: &ClipEntry) -> String {
    let n = e.types.len();
    format!("{n} format{}", if n == 1 { "" } else { "s" })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clip::model::tests::{clip, now};
    use kudzu_core::clipboard::{ClipSource, ImageSize};

    fn labels(rows: &[Fact]) -> Vec<&'static str> {
        rows.iter().map(|f| f.label).collect()
    }

    #[test]
    fn a_text_clip_lists_when_length_and_expiry() {
        let e = clip("hello world", 5, false);
        let rows = details(&e, Some("hello world"), 7, now(), false);
        assert_eq!(labels(&rows), ["Copied", "Length", "Expires"]);
        assert_eq!(rows[1].value, "11 characters · 2 words");
        assert!(rows[0].value.starts_with("Today at "));
    }

    #[test]
    fn a_recopied_clip_adds_first_copied_and_compact_adds_times() {
        let mut e = clip("x", 5, false);
        e.copy_count = 3;
        e.first_copied_at -= chrono::Duration::days(2);
        assert_eq!(
            labels(&details(&e, None, 7, now(), false)),
            ["Copied", "First copied", "Length", "Expires"]
        );
        assert_eq!(
            labels(&details(&e, None, 7, now(), true)),
            ["Copied", "First copied", "Times", "Length", "Expires"]
        );
    }

    #[test]
    fn the_source_row_is_an_app_row() {
        let mut e = clip("x", 5, false);
        e.source = Some(ClipSource {
            bundle_id: "a".into(),
            name: "Slack".into(),
            path: None,
        });
        let rows = details(&e, None, 7, now(), false);
        assert_eq!(
            rows[1],
            Fact {
                label: "From",
                value: "Slack".into(),
                app: true
            }
        );
    }

    #[test]
    fn images_files_and_other_data_have_their_own_rows() {
        let mut image = clip("", 5, false);
        image.kind = ClipKind::Image;
        image.text = None;
        image.image = Some(ImageSize {
            width: 640,
            height: 480,
        });
        image.size = 2048;
        let rows = details(&image, None, 7, now(), false);
        assert_eq!(labels(&rows), ["Copied", "Dimensions", "Size", "Expires"]);
        assert_eq!(rows[1].value, "640 × 480 px");
        let mut file = clip("", 5, false);
        file.kind = ClipKind::File;
        file.files = vec!["/a".into(), "/b".into()];
        assert_eq!(details(&file, None, 7, now(), false)[1], fact("Items", "2"));
    }

    #[test]
    fn a_reused_clip_says_how_often_and_when() {
        let mut e = clip("x", 60, false);
        e.use_count = 2;
        e.last_used_at = Some(e.copied_at + chrono::Duration::minutes(30));
        let rows = details(&e, None, 7, now(), false);
        assert!(rows
            .iter()
            .any(|f| f.label == "Reused" && f.value.starts_with("2× · last ")));
    }

    #[test]
    fn a_pinned_clip_never_expires_and_formats_are_counted() {
        let mut e = clip("x", 5, true);
        e.types = vec!["public.utf8-plain-text".into()];
        assert_eq!(
            details(&e, None, 7, now(), false).last().unwrap().value,
            "Never (pinned)"
        );
        assert_eq!(formats_label(&e), "1 format");
        e.types.push("public.html".into());
        assert_eq!(formats_label(&e), "2 formats");
    }
}
