//! Static analysis: form fields, referenced snippets, cursor and clipboard use.

use std::collections::HashSet;

use uuid::Uuid;

use super::{
    parser, typed, FormFieldKind, FormFieldSpec, Node, ParsedTemplate, Tag, TemplateInfo,
    TemplateTag,
};

pub(crate) fn analyze(parsed: &ParsedTemplate) -> TemplateInfo {
    let mut info = TemplateInfo {
        form_fields: Vec::new(),
        referenced_snippet_ids: Vec::new(),
        has_cursor: false,
        uses_clipboard: false,
        uses_selection: false,
        has_key_presses: false,
    };
    let mut names = HashSet::new();
    let mut ids = HashSet::new();
    for node in &parsed.nodes {
        let Node::Tag(tag) = node else { continue };
        match typed(tag) {
            TemplateTag::Snippet { id } => {
                if ids.insert(id) {
                    info.referenced_snippet_ids.push(id);
                }
            }
            TemplateTag::Clipboard { .. } => info.uses_clipboard = true,
            TemplateTag::Cursor => info.has_cursor = true,
            TemplateTag::Selection { .. } => info.uses_selection = true,
            TemplateTag::Key { .. } => info.has_key_presses = true,
            other => {
                if let Some(field) = field_spec(other) {
                    if names.insert(field.name.clone()) {
                        info.form_fields.push(field);
                    }
                }
            }
        }
    }
    info
}

fn field_spec(tag: TemplateTag) -> Option<FormFieldSpec> {
    match tag {
        TemplateTag::Field {
            name,
            default,
            multiline,
        } => Some(FormFieldSpec {
            name,
            kind: if multiline {
                FormFieldKind::Multiline
            } else {
                FormFieldKind::Text
            },
            options: Vec::new(),
            default_value: default,
        }),
        TemplateTag::Choice {
            name,
            options,
            default,
        } => Some(FormFieldSpec {
            name,
            kind: FormFieldKind::Choice,
            options,
            default_value: default,
        }),
        _ => None,
    }
}

/// Fields depth first, with the renderer's path guard and eight-level limit.
pub(crate) fn form_fields(
    source: &str,
    resolve_snippet: &dyn Fn(Uuid) -> Option<String>,
) -> Vec<FormFieldSpec> {
    let mut walker = Walker {
        resolve_snippet,
        fields: Vec::new(),
        names: HashSet::new(),
    };
    walker.visit(source, &mut Vec::new(), 0);
    walker.fields
}

struct Walker<'r> {
    resolve_snippet: &'r dyn Fn(Uuid) -> Option<String>,
    fields: Vec<FormFieldSpec>,
    names: HashSet<String>,
}

impl Walker<'_> {
    fn visit(&mut self, source: &str, path: &mut Vec<Uuid>, depth: usize) {
        for node in parser::parse(source).nodes {
            let Node::Tag(tag) = node else { continue };
            self.visit_tag(&tag, path, depth);
        }
    }

    fn visit_tag(&mut self, tag: &Tag, path: &mut Vec<Uuid>, depth: usize) {
        match typed(tag) {
            TemplateTag::Snippet { id } => {
                if depth < 8 && !path.contains(&id) {
                    if let Some(nested) = (self.resolve_snippet)(id) {
                        path.push(id);
                        self.visit(&nested, path, depth + 1);
                        path.pop();
                    }
                }
            }
            other => {
                if let Some(field) = field_spec(other) {
                    if self.names.insert(field.name.clone()) {
                        self.fields.push(field);
                    }
                }
            }
        }
    }
}
