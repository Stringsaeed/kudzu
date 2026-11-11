//! YAML match files → raw structures: file loading with `imports:`, and the match, variable
//! and form-field shapes.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use super::super::yaml::{self, Node};
use super::super::ImportWarning;
use super::{normalize, relative_path, FormFieldConfig, Match, MatchFile, Variable};

pub(super) fn load_file(
    path: &Path,
    base: Option<&Path>,
    visited: &mut HashSet<PathBuf>,
    files: &mut Vec<MatchFile>,
    warnings: &mut Vec<ImportWarning>,
) {
    let path = normalize(path);
    let canonical = fs::canonicalize(&path).unwrap_or_else(|_| path.clone());
    if !visited.insert(canonical) {
        return;
    }
    let (relative, dirs) = relative_path(&path, base);
    let file_warning = |message: String| ImportWarning {
        file: relative.clone(),
        line: None,
        match_index: None,
        trigger: None,
        message,
    };
    if !path.exists() {
        warnings.push(file_warning(format!(
            "Match file not found at '{relative}'."
        )));
        return;
    }
    let text = match fs::read_to_string(&path) {
        Ok(t) => t,
        Err(e) => {
            warnings.push(file_warning(format!("Failed to read file: {e}")));
            return;
        }
    };
    let group_name = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let Some(file) = parse_match_file(&text, relative.clone(), dirs, group_name, warnings) else {
        return;
    };
    let imports = file.imports.clone();
    files.push(file);
    let parent = path.parent().map(Path::to_path_buf).unwrap_or_default();
    for import in imports {
        let target = if Path::new(&import).is_absolute() {
            PathBuf::from(&import)
        } else {
            parent.join(&import)
        };
        load_file(&target, base, visited, files, warnings);
    }
}

// MARK: - YAML → raw structures

fn parse_match_file(
    text: &str,
    relative_path: String,
    directory_components: Vec<String>,
    group_name: String,
    warnings: &mut Vec<ImportWarning>,
) -> Option<MatchFile> {
    let mut file = MatchFile {
        relative_path,
        directory_components,
        group_name,
        imports: vec![],
        global_vars: vec![],
        matches: vec![],
    };
    let root = match yaml::parse(text) {
        Ok(Some(root)) => root,
        Ok(None) => return Some(file),
        Err(e) => {
            warnings.push(ImportWarning {
                file: file.relative_path,
                line: None,
                match_index: None,
                trigger: None,
                message: format!("YAML syntax error: {e}"),
            });
            return None;
        }
    };
    if root.as_map().is_none() {
        // A file holding only a comment or an empty scalar is still an empty match file.
        if root.as_str() == Some("") {
            return Some(file);
        }
        warnings.push(ImportWarning {
            file: file.relative_path,
            line: Some(root.line),
            match_index: None,
            trigger: None,
            message: "Expected top-level YAML mapping in match file.".into(),
        });
        return None;
    }
    file.imports = seq(root.get("imports"))
        .iter()
        .filter_map(|n| n.as_str().map(str::to_string))
        .collect();
    file.global_vars = seq(root.get("global_vars"))
        .iter()
        .filter_map(parse_variable)
        .collect();
    for (index, node) in seq(root.get("matches")).iter().enumerate() {
        match parse_match(node, index) {
            Some(m) => file.matches.push(m),
            None => warnings.push(ImportWarning {
                file: file.relative_path.clone(),
                line: Some(node.line),
                match_index: Some(index),
                trigger: None,
                message: format!("Malformed match entry at index {index} was skipped."),
            }),
        }
    }
    Some(file)
}

fn seq(node: Option<&Node>) -> &[Node] {
    node.and_then(Node::as_seq).unwrap_or(&[])
}

fn parse_match(node: &Node, index: usize) -> Option<Match> {
    node.as_map()?;
    let text = |key: &str| node.str_at(key).map(str::to_string);
    let mut triggers: Vec<String> = Vec::new();
    if let Some(t) = node.str_at("trigger").filter(|t| !t.is_empty()) {
        triggers.push(t.to_string());
    }
    triggers.extend(
        seq(node.get("triggers"))
            .iter()
            .filter_map(Node::as_str)
            .filter(|t| !t.is_empty())
            .map(str::to_string),
    );
    Some(Match {
        triggers,
        regex: text("regex"),
        label: text("label"),
        replace: text("replace"),
        markdown: text("markdown"),
        html: text("html"),
        image_path: text("image_path"),
        form: text("form"),
        form_fields: node
            .get("form_fields")
            .map(parse_form_fields)
            .unwrap_or_default(),
        vars: seq(node.get("vars"))
            .iter()
            .filter_map(parse_variable)
            .collect(),
        word: node.bool_at("word").unwrap_or(false),
        left_word: node.bool_at("left_word").unwrap_or(false),
        right_word: node.bool_at("right_word").unwrap_or(false),
        propagate_case: node.bool_at("propagate_case").unwrap_or(false),
        force_mode: text("force_mode"),
        index,
        line: Some(node.line),
    })
}

fn parse_variable(node: &Node) -> Option<Variable> {
    Some(Variable {
        name: node.str_at("name")?.to_string(),
        kind: node.str_at("type")?.to_string(),
        params: node.get("params").filter(|p| p.as_map().is_some()).cloned(),
        line: Some(node.line),
    })
}

pub(super) fn parse_form_fields(node: &Node) -> HashMap<String, FormFieldConfig> {
    let Some(entries) = node.as_map() else {
        return HashMap::new();
    };
    let mut out = HashMap::new();
    for (key, config) in entries {
        let (Some(name), Some(_)) = (key.as_str(), config.as_map()) else {
            continue;
        };
        let trim = config.bool_at("trim_string_values").unwrap_or(true);
        let values = match config.get("values") {
            Some(v) if v.as_seq().is_some() => v
                .as_seq()
                .unwrap_or(&[])
                .iter()
                .filter_map(|n| n.as_str().map(str::to_string))
                .collect(),
            Some(v) => v
                .as_str()
                .unwrap_or("")
                .split([
                    '\n', '\r', '\u{0B}', '\u{0C}', '\u{85}', '\u{2028}', '\u{2029}',
                ])
                .map(|l| if trim { l.trim_matches([' ', '\t']) } else { l })
                .filter(|l| !l.is_empty())
                .map(str::to_string)
                .collect(),
            None => vec![],
        };
        out.insert(
            name.to_string(),
            FormFieldConfig {
                kind: config.str_at("type").unwrap_or("text").to_string(),
                multiline: config.bool_at("multiline").unwrap_or(false),
                default: config.str_at("default").map(str::to_string),
                values,
            },
        );
    }
    out
}
