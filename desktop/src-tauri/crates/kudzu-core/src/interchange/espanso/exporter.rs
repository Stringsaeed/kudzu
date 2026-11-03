//! Kudzu library → Espanso match files.

use std::collections::{BTreeMap, HashMap, HashSet};

use uuid::Uuid;
use yaml_rust2::yaml::Hash;
use yaml_rust2::{Yaml, YamlEmitter};

use crate::interchange::strftime;
use crate::template::{self, AttributeValue, Node, Tag, TemplateTag};
use crate::types::{CaseMode, FireMode, GroupRecord, InsertionMethod, KudzuLibrary, SnippetRecord};

fn s(text: &str) -> Yaml {
    Yaml::String(text.to_string())
}

fn map(pairs: Vec<(&str, Yaml)>) -> Yaml {
    let mut hash = Hash::new();
    for (k, v) in pairs {
        hash.insert(s(k), v);
    }
    Yaml::Hash(hash)
}

pub(super) fn export(library: &KudzuLibrary) -> BTreeMap<String, String> {
    let groups: HashMap<Uuid, &GroupRecord> = library.groups.iter().map(|g| (g.id, g)).collect();
    let snippets: HashMap<Uuid, &SnippetRecord> =
        library.snippets.iter().map(|s| (s.id, s)).collect();

    // Several groups can map to one path (same name, or a root group named "base"); their
    // matches share the file rather than overwriting each other.
    let mut files: BTreeMap<String, Vec<Yaml>> = BTreeMap::new();
    let mut ordered: Vec<Option<Uuid>> = library.groups.iter().map(|g| Some(g.id)).collect();
    ordered.push(None);
    let mut by_group: HashMap<Option<Uuid>, Vec<&SnippetRecord>> = HashMap::new();
    for snippet in &library.snippets {
        let gid = snippet.group_id.filter(|g| groups.contains_key(g));
        by_group.entry(gid).or_default().push(snippet);
    }
    let mut seen = HashSet::new();
    for gid in ordered {
        if !seen.insert(gid) {
            continue;
        }
        let Some(members) = by_group.get(&gid) else {
            continue;
        };
        let nodes: Vec<Yaml> = members
            .iter()
            .filter_map(|s| match_node(s, &snippets))
            .collect();
        if nodes.is_empty() {
            continue;
        }
        files
            .entry(relative_file_path(gid, &groups))
            .or_default()
            .extend(nodes);
    }

    files
        .into_iter()
        .map(|(path, matches)| {
            let doc = map(vec![("matches", Yaml::Array(matches))]);
            let mut out = String::new();
            let mut emitter = YamlEmitter::new(&mut out);
            emitter.multiline_strings(true);
            emitter
                .dump(&doc)
                .expect("writing YAML to a String cannot fail");
            let body = out.strip_prefix("---\n").unwrap_or(&out).to_string();
            (path, body + "\n")
        })
        .collect()
}

fn relative_file_path(group_id: Option<Uuid>, groups: &HashMap<Uuid, &GroupRecord>) -> String {
    let Some(group) = group_id.and_then(|id| groups.get(&id)) else {
        return "base.yml".into();
    };
    let mut components = vec![sanitize(&group.name)];
    let mut visited = HashSet::from([group.id]);
    let mut parent = group.parent_id;
    while let Some(pid) = parent {
        let Some(p) = groups.get(&pid).filter(|_| visited.insert(pid)) else {
            break;
        };
        components.insert(0, sanitize(&p.name));
        parent = p.parent_id;
    }
    let file = format!("{}.yml", components.pop().unwrap_or_else(|| "base".into()));
    if components.is_empty() {
        file
    } else {
        format!("{}/{file}", components.join("/"))
    }
}

/// A path component Espanso will auto-load: no `/`, no leading `_` (which Espanso skips).
fn sanitize(name: &str) -> String {
    let cleaned = name.trim().replace('/', "_");
    let cleaned = cleaned.trim_start_matches('_');
    if cleaned.is_empty() {
        "group".into()
    } else {
        cleaned.to_string()
    }
}

fn match_node(snippet: &SnippetRecord, snippets: &HashMap<Uuid, &SnippetRecord>) -> Option<Yaml> {
    let first = snippet.triggers.first()?;
    let mut pairs: Vec<(&str, Yaml)> = Vec::new();
    if snippet.triggers.len() == 1 {
        pairs.push(("trigger", s(first)));
    } else {
        pairs.push((
            "triggers",
            Yaml::Array(snippet.triggers.iter().map(|t| s(t)).collect()),
        ));
    }
    if !snippet.name.is_empty() && &snippet.name != first {
        pairs.push(("label", s(&snippet.name)));
    }

    let nodes = template::parse(&snippet.content).nodes;
    let tag_named = |names: &[&str]| {
        nodes
            .iter()
            .any(|n| matches!(n, Node::Tag(t) if names.contains(&t.name.as_str())))
    };
    let has_form = tag_named(&["field", "choice"]);
    let has_other_vars = tag_named(&["date", "time", "clipboard", "snippet"]);

    // The `form:` shorthand fits when only form tags need variables.
    if has_form && !has_other_vars {
        let (form, fields) = form_shorthand(&nodes);
        pairs.push(("form", s(&form)));
        if let Some(fields) = fields {
            pairs.push(("form_fields", fields));
        }
    } else {
        let (replace, vars) = replace_and_vars(&nodes, snippets);
        pairs.push(("replace", s(&replace)));
        if !vars.is_empty() {
            pairs.push(("vars", Yaml::Array(vars)));
        }
    }
    if snippet.fire_mode == FireMode::WholeWord {
        pairs.push(("word", Yaml::Boolean(true)));
    }
    if snippet.case_mode == CaseMode::Propagate {
        pairs.push(("propagate_case", Yaml::Boolean(true)));
    }
    match snippet.insertion {
        Some(InsertionMethod::Paste) => pairs.push(("force_mode", s("clipboard"))),
        Some(InsertionMethod::Type) => pairs.push(("force_mode", s("keys"))),
        _ => {}
    }
    Some(map(pairs))
}

/// Field options for a form control, or `None` when it has nothing beyond the defaults.
fn field_config(tag: &TemplateTag) -> Option<Yaml> {
    match tag {
        TemplateTag::Field {
            default, multiline, ..
        } => {
            let mut pairs = Vec::new();
            if *multiline {
                pairs.push(("multiline", Yaml::Boolean(true)));
            }
            if let Some(d) = default.as_deref().filter(|d| !d.is_empty()) {
                pairs.push(("default", s(d)));
            }
            (!pairs.is_empty()).then(|| map(pairs))
        }
        TemplateTag::Choice {
            options, default, ..
        } => {
            let mut pairs = vec![
                ("type", s("choice")),
                (
                    "values",
                    Yaml::Array(options.iter().map(|o| s(o)).collect()),
                ),
            ];
            if let Some(d) = default.as_deref().filter(|d| !d.is_empty()) {
                pairs.push(("default", s(d)));
            }
            Some(map(pairs))
        }
        _ => None,
    }
}

fn form_shorthand(nodes: &[Node]) -> (String, Option<Yaml>) {
    let mut text = String::new();
    let mut fields = Hash::new();
    let mut seen = HashSet::new();
    for node in nodes {
        match node {
            Node::Text { text: t, .. } => text.push_str(&escape_braces(t)),
            Node::Tag(tag) => {
                let typed = template::typed(tag);
                match &typed {
                    TemplateTag::Cursor => text.push_str("$|$"),
                    TemplateTag::Field { name, .. } | TemplateTag::Choice { name, .. } => {
                        text.push_str(&format!("[[{name}]]"));
                        if seen.insert(name.clone()) {
                            if let Some(config) = field_config(&typed) {
                                fields.insert(s(name), config);
                            }
                        }
                    }
                    _ => text.push_str(&escape_braces(&tag_source(tag))),
                }
            }
        }
    }
    (text, (!fields.is_empty()).then_some(Yaml::Hash(fields)))
}

fn replace_and_vars(
    nodes: &[Node],
    snippets: &HashMap<Uuid, &SnippetRecord>,
) -> (String, Vec<Yaml>) {
    let mut text = String::new();
    let mut vars = Vec::new();
    let mut counter = 0;
    // Form tags mixed with other variables go into one verbose `form1` variable.
    let mut layout = Vec::new();
    let mut form_fields = Hash::new();
    let mut seen_fields = HashSet::new();

    for node in nodes {
        let tag = match node {
            Node::Text { text: t, .. } => {
                text.push_str(&escape_braces(t));
                continue;
            }
            Node::Tag(tag) => tag,
        };
        let typed = template::typed(tag);
        match &typed {
            TemplateTag::Cursor => text.push_str("$|$"),
            TemplateTag::Date { format, locale, .. } | TemplateTag::Time { format, locale, .. } => {
                counter += 1;
                let name = format!("date_{counter}");
                let mut params = vec![("format", s(&strftime::to_strftime(format)))];
                let offset =
                    string_attribute(tag, "offset").and_then(strftime::kudzu_offset_to_seconds);
                if let Some(seconds) = offset.filter(|&s| s != 0) {
                    params.push(("offset", Yaml::Integer(seconds)));
                }
                if let Some(l) = locale.as_deref().filter(|l| !l.is_empty()) {
                    params.push(("locale", s(l)));
                }
                vars.push(map(vec![
                    ("name", s(&name)),
                    ("type", s("date")),
                    ("params", map(params)),
                ]));
                text.push_str(&format!("{{{{{name}}}}}"));
            }
            TemplateTag::Clipboard { .. } => {
                counter += 1;
                let name = format!("clipboard_{counter}");
                vars.push(map(vec![("name", s(&name)), ("type", s("clipboard"))]));
                text.push_str(&format!("{{{{{name}}}}}"));
            }
            TemplateTag::Snippet { id } => {
                match snippets.get(id).and_then(|t| t.triggers.first()) {
                    Some(trigger) => {
                        counter += 1;
                        let name = format!("match_{counter}");
                        vars.push(map(vec![
                            ("name", s(&name)),
                            ("type", s("match")),
                            ("params", map(vec![("trigger", s(trigger))])),
                        ]));
                        text.push_str(&format!("{{{{{name}}}}}"));
                    }
                    None => text.push_str(&escape_braces(&tag_source(tag))),
                }
            }
            TemplateTag::Field { name, .. } | TemplateTag::Choice { name, .. } => {
                text.push_str(&format!("{{{{form1.{name}}}}}"));
                if seen_fields.insert(name.clone()) {
                    layout.push(format!("{name}: [[{name}]]"));
                    if let Some(config) = field_config(&typed) {
                        form_fields.insert(s(name), config);
                    }
                }
            }
            TemplateTag::Unknown => text.push_str(&escape_braces(&tag_source(tag))),
        }
    }

    if !seen_fields.is_empty() {
        let mut params = vec![("layout", s(&layout.join("\n")))];
        if !form_fields.is_empty() {
            params.push(("fields", Yaml::Hash(form_fields)));
        }
        vars.insert(
            0,
            map(vec![
                ("name", s("form1")),
                ("type", s("form")),
                ("params", map(params)),
            ]),
        );
    }
    (text, vars)
}

fn string_attribute<'a>(tag: &'a Tag, name: &str) -> Option<&'a str> {
    tag.attributes
        .iter()
        .rev()
        .find(|a| a.name == name)
        .and_then(|a| match &a.value {
            AttributeValue::String(v) => Some(v.as_str()),
            _ => None,
        })
}

/// The tag as written in the template: verbatim when unrecognized, canonical otherwise.
fn tag_source(tag: &Tag) -> String {
    template::serialize(&[Node::Tag(tag.clone())])
}

fn escape_braces(literal: &str) -> String {
    literal.replace("{{", "\\{\\{")
}
