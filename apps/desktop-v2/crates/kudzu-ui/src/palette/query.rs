//! The palette's query: a `#group` prefix pulled out of the text, and whether a
//! non-empty query should open the editor as a trigger or a name.

/// Ranked rows the palette shows. The search index's own default is 200.
pub const RESULT_LIMIT: usize = 60;

/// Where [`create_seed`] puts the trimmed query.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CreateAs {
    /// A single token that starts with the default trigger prefix.
    Trigger,
    /// Anything else, including an empty query from ⌘N.
    Name,
}

/// The unsaved editor a create action opens. `value` is the trimmed query.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CreateSeed {
    pub as_: CreateAs,
    pub value: String,
}

/// `(text sent to rank, group prefix)`.
///
/// The prefix is the first `(start or whitespace) + # + non-whitespace` in `query`,
/// lowercased, and may be empty (`#` alone). That span, including the whitespace that
/// introduced it, is replaced with a space and the remainder is trimmed. No match leaves
/// the query untouched; ranking trims it.
pub fn split_query(query: &str) -> (String, Option<String>) {
    let chars: Vec<(usize, char)> = query.char_indices().collect();
    for (i, &(byte, ch)) in chars.iter().enumerate() {
        let boundary = i == 0 || is_js_space(chars[i - 1].1);
        if !boundary || ch != '#' {
            continue;
        }
        // The match includes the whitespace that introduced the `#`, as `(?:^|\s)#` does.
        let start = if i == 0 { byte } else { chars[i - 1].0 };
        let mut end = byte + ch.len_utf8();
        let mut filter = String::new();
        for &(b, c) in chars.iter().skip(i + 1) {
            if is_js_space(c) {
                break;
            }
            filter.push(c);
            end = b + c.len_utf8();
        }
        let mut text = String::new();
        text.push_str(&query[..start]);
        text.push(' ');
        text.push_str(&query[end..]);
        return (text.trim().to_string(), Some(filter.to_lowercase()));
    }
    (query.to_string(), None)
}

/// `palette_create`: a single token that starts with a non-empty prefix is a trigger.
/// `query` is the field's text, not the text left after the `#` filter.
pub fn create_seed(query: &str, prefix: &str) -> CreateSeed {
    let value = query.trim().to_string();
    let trigger = !value.is_empty()
        && !value.contains(' ')
        && !prefix.is_empty()
        && value.starts_with(prefix);
    CreateSeed {
        as_: if trigger {
            CreateAs::Trigger
        } else {
            CreateAs::Name
        },
        value,
    }
}

/// JavaScript `\s`: ASCII whitespace plus the space separators `\s` matches without `/u`
/// being required, which in current JS is Unicode `White_Space`.
fn is_js_space(c: char) -> bool {
    c.is_whitespace()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_hash_prefix_is_the_group_filter_and_leaves_the_text() {
        let (text, group) = split_query("#Work invoice");
        assert_eq!(text, "invoice");
        assert_eq!(group.as_deref(), Some("work"));
    }

    #[test]
    fn only_the_first_hash_matches_and_a_mid_word_hash_does_not_start_one() {
        let (text, group) = split_query("cost#tag #Email later #nope");
        assert_eq!(group.as_deref(), Some("email"));
        assert_eq!(text, "cost#tag  later #nope");
    }

    #[test]
    fn a_bare_hash_matches_every_group_name() {
        let (text, group) = split_query("see #");
        assert_eq!(text, "see");
        assert_eq!(group.as_deref(), Some(""));
    }

    #[test]
    fn no_hash_keeps_the_query_including_surrounding_space() {
        let (text, group) = split_query("  hello  ");
        assert_eq!(text, "  hello  ");
        assert!(group.is_none());
    }

    #[test]
    fn a_prefixed_token_is_a_trigger_and_anything_else_is_a_name() {
        assert_eq!(create_seed(";zz", ";").as_, CreateAs::Trigger);
        assert_eq!(create_seed(";zz", ";").value, ";zz");
        assert_eq!(create_seed("brand new", ";").as_, CreateAs::Name);
        assert_eq!(create_seed("alpha", ";").as_, CreateAs::Name);
        assert_eq!(create_seed(";zz", "").as_, CreateAs::Name);
        assert_eq!(create_seed("  ", ";").value, "");
        assert_eq!(create_seed("  ", ";").as_, CreateAs::Name);
    }
}
