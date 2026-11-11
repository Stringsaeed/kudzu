//! Espanso replacement text → Kudzu template source: `{{var}}` references, forms, dates and
//! the rest of the variable types.

use std::collections::HashMap;

use uuid::Uuid;

use super::super::yaml::Node;
use super::super::{tags, ImportWarning};
use super::parse::parse_form_fields;
use super::{FormFieldConfig, Origin, Variable};
use crate::interchange::strftime;

// MARK: - Template conversion

pub(super) struct Converter<'a> {
    pub(super) vars: &'a HashMap<String, Variable>,
    pub(super) trigger_ids: &'a HashMap<String, Uuid>,
    pub(super) origin: Origin<'a>,
}

/// The index of the next `close` pair at or after `from`.
fn find_pair(chars: &[char], from: usize, close: char) -> Option<usize> {
    (from..chars.len().saturating_sub(1)).find(|&i| chars[i] == close && chars[i + 1] == close)
}

fn at(chars: &[char], i: usize, pattern: &str) -> bool {
    pattern
        .chars()
        .enumerate()
        .all(|(k, c)| chars.get(i + k) == Some(&c))
}

impl Converter<'_> {
    /// Converts a `form:` shorthand: `[[field]]` placeholders plus `$|$` and `{{var}}`.
    pub(super) fn convert_form(
        &self,
        raw: &str,
        fields: &HashMap<String, FormFieldConfig>,
        warnings: &mut Vec<ImportWarning>,
    ) -> String {
        self.convert(raw, Some(fields), self.origin.line, warnings)
    }

    /// Converts a `replace:` text: `$|$` and `{{var}}` references.
    pub(super) fn convert_replace(&self, raw: &str, warnings: &mut Vec<ImportWarning>) -> String {
        self.convert(raw, None, self.origin.line, warnings)
    }

    fn convert(
        &self,
        raw: &str,
        form_fields: Option<&HashMap<String, FormFieldConfig>>,
        line: Option<usize>,
        warnings: &mut Vec<ImportWarning>,
    ) -> String {
        let chars: Vec<char> = raw.chars().collect();
        let mut out = String::new();
        let mut i = 0;
        while i < chars.len() {
            if at(&chars, i, "$|$") {
                out.push_str(&tags::cursor_tag());
                i += 3;
                continue;
            }
            // Espanso's escaped braces `\{\{` become Kudzu's `\{{`.
            if at(&chars, i, "\\{\\{") {
                out.push_str("\\{{");
                i += 4;
                continue;
            }
            if form_fields.is_none() && at(&chars, i, "\\{{") {
                out.push_str("\\{{");
                i += 3;
                continue;
            }
            if let Some(fields) = form_fields {
                if at(&chars, i, "[[") {
                    if let Some(close) = find_pair(&chars, i + 2, ']') {
                        let name: String = chars[i + 2..close].iter().collect();
                        let name = name.trim_matches([' ', '\t']);
                        if !name.is_empty() {
                            out.push_str(&self.field_tag(name, fields.get(name), line, warnings));
                            i = close + 2;
                            continue;
                        }
                    }
                }
            }
            if at(&chars, i, "{{") {
                if let Some(close) = find_pair(&chars, i + 2, '}') {
                    let reference: String = chars[i + 2..close].iter().collect();
                    out.push_str(&self.resolve(
                        reference.trim_matches([' ', '\t']),
                        line,
                        warnings,
                    ));
                    i = close + 2;
                } else {
                    out.push_str("\\{{");
                    i += 2;
                }
                continue;
            }
            // Kudzu templates read `\\` as one literal backslash, so double every other backslash.
            if chars[i] == '\\' {
                out.push_str("\\\\");
            } else {
                out.push(chars[i]);
            }
            i += 1;
        }
        out
    }

    fn resolve(
        &self,
        reference: &str,
        line: Option<usize>,
        warnings: &mut Vec<ImportWarning>,
    ) -> String {
        let literal = format!("\\{{{{{reference}}}}}");
        // Verbose form reference: `form.field`.
        if let Some((form_name, field)) = reference.split_once('.') {
            if let Some(form) = self.vars.get(form_name).filter(|v| v.kind == "form") {
                let fields = form
                    .param("fields")
                    .map(parse_form_fields)
                    .unwrap_or_default();
                return self.field_tag(field, fields.get(field), form.line.or(line), warnings);
            }
        }
        let Some(var) = self.vars.get(reference) else {
            warnings.push(self.origin.warn(
                line,
                format!("Undefined variable '{{{{{reference}}}}}' kept as literal text."),
            ));
            return literal;
        };
        let line = var.line.or(line);
        match var.kind.as_str() {
            "date" => {
                let converted = strftime::to_icu(var.param_str("format").unwrap_or("%Y-%m-%d"));
                if !converted.unsupported_specifiers.is_empty() {
                    warnings.push(self.origin.warn(
                        line,
                        format!(
                            "Unsupported strftime specifier(s) {} in date variable '{}' kept as literal.",
                            converted.unsupported_specifiers.join(", "),
                            var.name
                        ),
                    ));
                }
                let offset = var
                    .param_str("offset")
                    .and_then(|s| s.trim().parse::<i64>().ok())
                    .and_then(strftime::seconds_to_kudzu_offset);
                if var.param("tz").is_some() {
                    warnings.push(self.origin.warn(
                        line,
                        format!("Date variable '{}' specifies 'tz' which is not supported in v1 and was ignored.", var.name),
                    ));
                }
                tags::date_tag(
                    Some(&converted.icu_format),
                    offset.as_deref(),
                    var.param_str("locale"),
                )
            }
            "clipboard" => tags::clipboard_tag(false),
            "random" => {
                let choices: Vec<String> = var
                    .param("choices")
                    .and_then(Node::as_seq)
                    .map(|items| {
                        items
                            .iter()
                            .filter_map(|n| n.as_str().map(str::to_string))
                            .collect()
                    })
                    .unwrap_or_default();
                if choices.is_empty() {
                    warnings.push(self.origin.warn(
                        line,
                        format!(
                            "Random variable '{}' has no 'params.choices'; kept as literal text.",
                            var.name
                        ),
                    ));
                    return literal;
                }
                tags::random_tag(&choices)
            }
            "echo" => {
                // A fixed value: written out as literal text.
                let text = var.param_str("echo").unwrap_or_default().to_string();
                crate::template::serialize(&[crate::template::Node::Text { text, range: 0..0 }])
            }
            "match" => {
                let Some(trigger) = var.param_str("trigger").filter(|t| !t.is_empty()) else {
                    warnings.push(self.origin.warn(
                        line,
                        format!("Nested match variable '{}' is missing 'params.trigger'; kept as literal text.", var.name),
                    ));
                    return literal;
                };
                match self.trigger_ids.get(trigger) {
                    Some(&id) => tags::snippet_tag(id),
                    None => {
                        warnings.push(self.origin.warn(
                            line,
                            format!(
                                "Unresolved nested match trigger '{trigger}' for variable '{}'; kept as literal text.",
                                var.name
                            ),
                        ));
                        literal
                    }
                }
            }
            "form" => {
                // A bare `{{form}}` reference expands the whole layout.
                if let Some(layout) = var.param_str("layout") {
                    let fields = var
                        .param("fields")
                        .map(parse_form_fields)
                        .unwrap_or_default();
                    return self.convert(layout, Some(&fields), line, warnings);
                }
                warnings.push(self.origin.warn(
                    line,
                    format!("Form variable '{}' referenced directly without a field name; kept as literal text.", var.name),
                ));
                literal
            }
            other => {
                warnings.push(self.origin.warn(
                    line,
                    format!("Unsupported variable type '{other}' for variable '{}'; kept as literal text.", var.name),
                ));
                literal
            }
        }
    }

    fn field_tag(
        &self,
        name: &str,
        config: Option<&FormFieldConfig>,
        line: Option<usize>,
        warnings: &mut Vec<ImportWarning>,
    ) -> String {
        let Some(config) = config else {
            return tags::field_tag(name, None, false);
        };
        match config.kind.to_lowercase().as_str() {
            "choice" | "list" => tags::choice_tag(name, &config.values, config.default.as_deref()),
            "text" | "" => tags::field_tag(name, config.default.as_deref(), config.multiline),
            _ => {
                warnings.push(self.origin.warn(
                    line,
                    format!(
                        "Unknown form field control type '{}' for field '{name}'; imported as text field.",
                        config.kind
                    ),
                ));
                tags::field_tag(name, config.default.as_deref(), config.multiline)
            }
        }
    }
}
