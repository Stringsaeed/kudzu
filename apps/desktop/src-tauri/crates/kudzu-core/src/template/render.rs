//! Recursive renderer and calendar offsets.

use chrono::{DateTime, Duration, FixedOffset, Months};
use uuid::Uuid;

use super::parser::{self, utf16_len};
use super::{
    icu, DateOffset, Diagnostic, Node, RenderContext, RenderResult, Severity, Tag, TemplateTag,
};

/// Maximum nesting depth: the root is depth 0 and a reference at depth 8 is rejected.
const MAX_DEPTH: usize = 8;

pub(crate) fn render(source: &str, context: &RenderContext<'_>) -> RenderResult {
    let mut renderer = Renderer {
        context,
        text: String::new(),
        utf16: 0,
        cursor: None,
        diagnostics: Vec::new(),
    };
    let mut path: Vec<Uuid> = context.root_snippet_id.into_iter().collect();
    renderer.render(source, &mut path, 0);
    RenderResult {
        text: renderer.text,
        cursor_offset: renderer.cursor,
        diagnostics: renderer.diagnostics,
    }
}

struct Renderer<'c, 'a> {
    context: &'c RenderContext<'a>,
    text: String,
    utf16: usize,
    cursor: Option<usize>,
    diagnostics: Vec<Diagnostic>,
}

impl Renderer<'_, '_> {
    fn append(&mut self, text: &str) {
        self.text.push_str(text);
        self.utf16 += utf16_len(text);
    }

    fn diagnose(&mut self, message: String, tag: &Tag) {
        self.diagnostics.push(Diagnostic {
            severity: Severity::Warning,
            message,
            range: tag.range.clone(),
        });
    }

    /// `path` holds the snippet IDs on the active reference chain (a stack, so separate branches may repeat).
    fn render(&mut self, source: &str, path: &mut Vec<Uuid>, depth: usize) {
        let parsed = parser::parse(source);
        self.diagnostics.extend(parsed.diagnostics);
        for node in &parsed.nodes {
            let tag = match node {
                Node::Text { text, .. } => {
                    self.append(text);
                    continue;
                }
                Node::Tag(tag) => tag,
            };
            match super::typed(tag) {
                TemplateTag::Cursor => {
                    if depth == 0 && self.cursor.is_none() {
                        self.cursor = Some(self.utf16);
                    }
                }
                TemplateTag::Clipboard { trim } => {
                    let clipboard = self.context.clipboard.as_deref().unwrap_or("");
                    self.append(if trim { clipboard.trim() } else { clipboard });
                }
                TemplateTag::Field { name, default, .. }
                | TemplateTag::Choice { name, default, .. } => {
                    let context = self.context;
                    let value = context
                        .form_values
                        .get(&name)
                        .map(String::as_str)
                        .or(default.as_deref())
                        .unwrap_or("");
                    self.append(value);
                }
                TemplateTag::Snippet { id } => {
                    if path.contains(&id) {
                        self.diagnose("Snippet cycle detected".into(), tag);
                    } else if depth >= MAX_DEPTH {
                        self.diagnose(
                            format!("Maximum snippet depth of {MAX_DEPTH} exceeded"),
                            tag,
                        );
                    } else if let Some(nested) = (self.context.resolve_snippet)(id) {
                        path.push(id);
                        self.render(&nested, path, depth + 1);
                        path.pop();
                    } else {
                        self.diagnose(
                            format!(
                                "Missing snippet {}",
                                id.hyphenated().to_string().to_uppercase()
                            ),
                            tag,
                        );
                    }
                }
                TemplateTag::Date {
                    format,
                    offset,
                    locale,
                }
                | TemplateTag::Time {
                    format,
                    offset,
                    locale,
                } => {
                    let mut date = self.context.now;
                    if let Some(offset) = offset {
                        match offset.apply(&date) {
                            Some(adjusted) => date = adjusted,
                            None => self
                                .diagnose("Date offset could not be applied; ignored".into(), tag),
                        }
                    }
                    let locale = locale.as_deref().unwrap_or(&self.context.locale);
                    let formatted = icu::format(&date, &format, locale);
                    self.append(&formatted);
                }
                TemplateTag::Unknown => {
                    // Verbatim source for unrecognized tags.
                    let mut source = String::new();
                    parser::write_tag(tag, &mut source);
                    self.append(&source);
                }
            }
        }
    }
}

impl DateOffset {
    /// Parses a signed offset such as `+1w2d` or `-1y2m`; `None` for invalid input.
    pub fn parse(source: &str) -> Option<DateOffset> {
        let (negative, rest) = match source.as_bytes().first()? {
            b'+' => (false, &source[1..]),
            b'-' => (true, &source[1..]),
            _ => return None,
        };
        let mut parts = Vec::new();
        let mut chars = rest.char_indices().peekable();
        while chars.peek().is_some() {
            let start = chars.peek()?.0;
            let mut end = start;
            while let Some(&(i, c)) = chars.peek() {
                if !c.is_ascii_digit() {
                    break;
                }
                end = i + 1;
                chars.next();
            }
            if end == start {
                return None;
            }
            let magnitude: i64 = rest[start..end].parse().ok()?;
            let unit_start = end;
            let mut unit_end = end;
            while let Some(&(i, c)) = chars.peek() {
                if !c.is_alphabetic() {
                    break;
                }
                unit_end = i + c.len_utf8();
                chars.next();
            }
            let unit = &rest[unit_start..unit_end];
            if !matches!(unit, "d" | "w" | "m" | "y" | "h" | "min") {
                return None;
            }
            parts.push((
                unit.to_string(),
                if negative { -magnitude } else { magnitude },
            ));
        }
        if parts.is_empty() {
            return None;
        }
        Some(DateOffset { parts })
    }

    /// Applies each component in written order; months and years clamp to the month end.
    /// `None` when the result is out of range.
    pub fn apply(&self, date: &DateTime<FixedOffset>) -> Option<DateTime<FixedOffset>> {
        let mut result = *date;
        for (unit, amount) in &self.parts {
            let amount = *amount;
            result = match unit.as_str() {
                "d" => result.checked_add_signed(Duration::try_days(amount)?)?,
                "w" => result.checked_add_signed(Duration::try_weeks(amount)?)?,
                "h" => result.checked_add_signed(Duration::try_hours(amount)?)?,
                "min" => result.checked_add_signed(Duration::try_minutes(amount)?)?,
                "m" => add_months(result, amount)?,
                "y" => add_months(result, amount.checked_mul(12)?)?,
                _ => return None,
            };
        }
        Some(result)
    }
}

fn add_months(date: DateTime<FixedOffset>, months: i64) -> Option<DateTime<FixedOffset>> {
    let magnitude = Months::new(u32::try_from(months.unsigned_abs()).ok()?);
    if months >= 0 {
        date.checked_add_months(magnitude)
    } else {
        date.checked_sub_months(magnitude)
    }
}
