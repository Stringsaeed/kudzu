//! ECMAScript whitespace (BOM is whitespace; NEL is not).
pub(super) fn is_js_whitespace(c: char) -> bool {
    c == '\u{feff}' || (c.is_whitespace() && c != '\u{85}')
}
