//! Names and triggers that don't collide with existing ones.

use std::collections::HashSet;

use unicode_segmentation::UnicodeSegmentation;

/// `base`, or `base 2`, `base 3`… the first not already in `existing` (case-insensitive).
pub fn make<'a>(base: &str, existing: impl IntoIterator<Item = &'a str>) -> String {
    first_free(base, existing, " ")
}

/// Like [`make`], but numbers without a space (`;snippet`, `;snippet2`), which suits triggers.
pub fn make_trigger<'a>(base: &str, existing: impl IntoIterator<Item = &'a str>) -> String {
    first_free(base, existing, "")
}

fn first_free<'a>(
    base: &str,
    existing: impl IntoIterator<Item = &'a str>,
    separator: &str,
) -> String {
    let taken: HashSet<String> = existing.into_iter().map(str::to_lowercase).collect();
    if !taken.contains(&base.to_lowercase()) {
        return base.to_string();
    }
    (2u64..)
        .map(|n| format!("{base}{separator}{n}"))
        .find(|candidate| !taken.contains(&candidate.to_lowercase()))
        .expect("an unbounded range always yields a free name")
}

/// A short snippet name from content: its first non-blank line, truncated to `limit` characters
/// with `…`; `fallback` when the content is blank.
pub fn name_from_content(content: &str, fallback: &str, limit: usize) -> String {
    let line = content
        .split([
            '\n', '\r', '\u{0B}', '\u{0C}', '\u{85}', '\u{2028}', '\u{2029}',
        ])
        .map(|l| l.trim_matches(|c: char| c.is_whitespace()))
        .find(|l| !l.is_empty())
        .unwrap_or("");
    if line.is_empty() {
        return fallback.to_string();
    }
    let graphemes: Vec<&str> = line.graphemes(true).collect();
    if graphemes.len() <= limit {
        return line.to_string();
    }
    format!("{}…", graphemes[..limit].concat().trim_end())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_names_from_two_case_insensitively_filling_gaps() {
        assert_eq!(make("New Snippet", ["Other"]), "New Snippet");
        assert_eq!(make("New Snippet", ["New Snippet"]), "New Snippet 2");
        assert_eq!(
            make(
                "New Snippet",
                ["New Snippet", "New Snippet 2", "new snippet 3"]
            ),
            "New Snippet 4"
        );
        assert_eq!(make("Foo", ["FOO"]), "Foo 2");
        assert_eq!(make("A", ["A", "A 3"]), "A 2");
    }

    #[test]
    fn triggers_number_without_space() {
        assert_eq!(make_trigger(";snippet", [";snippet"]), ";snippet2");
        assert_eq!(
            make_trigger(";snippet", [";SNIPPET", ";snippet2"]),
            ";snippet3"
        );
        assert_eq!(make_trigger(";snippet", []), ";snippet");
    }

    #[test]
    fn name_from_content_uses_first_line_truncates_and_falls_back() {
        assert_eq!(
            name_from_content("\n\n  Hello world \nsecond", "X", 40),
            "Hello world"
        );
        assert_eq!(
            name_from_content(&"a".repeat(100), "X", 10),
            format!("{}…", "a".repeat(10))
        );
        assert_eq!(name_from_content("  \n ", "Fallback", 40), "Fallback");
    }
}
