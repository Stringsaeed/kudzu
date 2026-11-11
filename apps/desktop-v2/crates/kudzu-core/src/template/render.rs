//! Recursive renderer and calendar offsets.

use crate::formatting::{FormatSpan, Formatting};
use chrono::{DateTime, Duration, FixedOffset, Months};
use unicode_segmentation::UnicodeSegmentation;
use uuid::Uuid;

use super::parser::{self, utf16_len};
use super::{
    icu, ActionKind, DateOffset, Diagnostic, Node, RenderAction, RenderContext, RenderResult,
    Severity, Tag, TemplateTag, MAX_TOTAL_DELAY_MS,
};

/// Maximum nesting depth: the root is depth 0 and a reference at depth 8 is rejected.
const MAX_DEPTH: usize = 8;

pub(crate) fn render(
    source: &str,
    formatting: Option<&Formatting>,
    context: &RenderContext<'_>,
) -> RenderResult {
    let mut renderer = Renderer {
        context,
        text: String::new(),
        utf16: 0,
        cursor: None,
        cursor_seen: false,
        diagnostics: Vec::new(),
        spans: formatting.map(|_| Vec::new()),
        actions: Vec::new(),
        paused_ms: 0,
    };
    let mut path: Vec<Uuid> = context.root_snippet_id.into_iter().collect();
    renderer.render(source, &mut path, 0, formatting);
    RenderResult {
        text: renderer.text,
        formatting: renderer.spans,
        cursor_offset: renderer.cursor,
        diagnostics: renderer.diagnostics,
        actions: renderer.actions,
    }
}

struct Renderer<'c, 'a> {
    context: &'c RenderContext<'a>,
    text: String,
    utf16: usize,
    cursor: Option<usize>,
    /// A root cursor tag was rendered; only the first counts, even if a key press cancels it.
    cursor_seen: bool,
    diagnostics: Vec<Diagnostic>,
    spans: Option<Vec<FormatSpan>>,
    actions: Vec<RenderAction>,
    /// Total pause so far, capped at [`MAX_TOTAL_DELAY_MS`].
    paused_ms: u32,
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
    fn render(
        &mut self,
        source: &str,
        path: &mut Vec<Uuid>,
        depth: usize,
        formatting: Option<&Formatting>,
    ) {
        let source_utf16: Vec<_> = if formatting.is_some() {
            source.encode_utf16().collect()
        } else {
            Vec::new()
        };
        let mut positions = formatting.map(|_| vec![None; source_utf16.len() + 1]);
        let parsed = parser::parse(source);
        self.diagnostics.extend(parsed.diagnostics);
        for node in &parsed.nodes {
            let tag = match node {
                Node::Text { text, range } => {
                    if let Some(positions) = &mut positions {
                        let raw = String::from_utf16_lossy(&source_utf16[range.clone()]);
                        literal_positions(&raw, range.start, self.utf16, positions);
                    }
                    self.append(text);
                    continue;
                }
                Node::Tag(tag) => tag,
            };
            if let Some(positions) = &mut positions {
                positions[tag.range.start] = Some(self.utf16);
            }
            match super::typed(tag) {
                TemplateTag::Cursor => {
                    if depth == 0 && !self.cursor_seen {
                        self.cursor_seen = true;
                        self.cursor = Some(self.utf16);
                    }
                }
                TemplateTag::Clipboard { trim, history } => {
                    let context = self.context;
                    let older;
                    let clipboard = if history == 0 {
                        context.clipboard.as_deref().unwrap_or("")
                    } else {
                        older = context
                            .clipboard_history
                            .and_then(|resolve| resolve(history as usize))
                            .unwrap_or_default();
                        &older
                    };
                    self.append(if trim { clipboard.trim() } else { clipboard });
                }
                TemplateTag::Selection { trim } => {
                    let context = self.context;
                    let selection = context.selection.as_deref().unwrap_or("");
                    self.append(if trim { selection.trim() } else { selection });
                }
                TemplateTag::Uuid { upper } => {
                    let id = self
                        .context
                        .new_uuid
                        .map_or(Uuid::nil(), |new| new())
                        .hyphenated()
                        .to_string();
                    self.append(&if upper { id.to_uppercase() } else { id });
                }
                TemplateTag::Random { options } => {
                    if !options.is_empty() {
                        let index =
                            self.context.pick.map_or(0, |pick| pick(options.len())) % options.len();
                        self.append(&options[index]);
                    }
                }
                TemplateTag::Key { key, count } => {
                    // The caret is placed with arrow keys after everything is inserted, which only
                    // works while it is still in the text that follows the last key press.
                    if self.cursor.take().is_some() && depth > 0 {
                        self.diagnose(
                            "A key press after the cursor cancels the cursor position".into(),
                            tag,
                        );
                    }
                    self.actions.push(RenderAction {
                        offset: self.utf16,
                        action: ActionKind::Key { key, count },
                    });
                }
                TemplateTag::Delay { ms } => {
                    let allowed = ms.min(MAX_TOTAL_DELAY_MS - self.paused_ms);
                    if allowed < ms {
                        self.diagnose(
                            format!(
                                "Pauses are limited to {} seconds in total; this one is cut short",
                                MAX_TOTAL_DELAY_MS / 1000
                            ),
                            tag,
                        );
                    }
                    if allowed > 0 {
                        self.paused_ms += allowed;
                        self.actions.push(RenderAction {
                            offset: self.utf16,
                            action: ActionKind::Pause { ms: allowed },
                        });
                    }
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
                        let formatting = self
                            .spans
                            .as_ref()
                            .and(self.context.resolve_formatting)
                            .and_then(|resolve| resolve(id))
                            .filter(|f| f.source == nested);
                        self.render(&nested, path, depth + 1, formatting.as_ref());
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
            if let Some(positions) = &mut positions {
                positions[tag.range.end] = Some(self.utf16);
            }
        }
        if let (Some(formatting), Some(positions), Some(spans)) =
            (formatting, positions, &mut self.spans)
        {
            spans.extend(formatting.spans.iter().filter_map(|span| {
                let start = positions.get(span.start).copied().flatten()?;
                let end = positions.get(span.end).copied().flatten()?;
                (start < end).then(|| FormatSpan {
                    start,
                    end,
                    style: span.style.clone(),
                })
            }));
        }
    }
}

// Match the parser's grapheme-aware escape handling. Tag interiors deliberately have no
// position: styling part of a variable is invalid, while styling its whole chip is stable.
fn literal_positions(raw: &str, start: usize, output: usize, positions: &mut [Option<usize>]) {
    let graphemes: Vec<_> = raw.graphemes(true).collect();
    let (mut i, mut input, mut rendered) = (0, start, output);
    positions[input] = Some(rendered);
    while i < graphemes.len() {
        let escaped = graphemes[i] == "\\"
            && (graphemes.get(i + 1) == Some(&"\\")
                || (graphemes.get(i + 1) == Some(&"{") && graphemes.get(i + 2) == Some(&"{")));
        if escaped {
            input += 1;
            i += 1;
        }
        let count = utf16_len(graphemes[i]);
        input += count;
        rendered += count;
        positions[input] = Some(rendered);
        i += 1;
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
