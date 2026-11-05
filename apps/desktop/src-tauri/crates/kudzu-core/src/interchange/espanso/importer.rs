//! Espanso match files → Kudzu library.

use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Component, Path, PathBuf};

use chrono::{SubsecRound, Utc};
use uuid::Uuid;

use super::yaml::{self, Node};
use super::{tags, EspansoImportResult, ImportWarning};
use crate::interchange::{strftime, InterchangeError};
use crate::types::{CaseMode, FireMode, GroupRecord, InsertionMethod, KudzuLibrary, SnippetRecord};

#[derive(Debug, Clone, Default)]
struct FormFieldConfig {
    kind: String,
    multiline: bool,
    default: Option<String>,
    values: Vec<String>,
}

#[derive(Debug, Clone)]
struct Variable {
    name: String,
    kind: String,
    params: Option<Node>,
    line: Option<usize>,
}

impl Variable {
    fn param(&self, key: &str) -> Option<&Node> {
        self.params.as_ref()?.get(key)
    }

    fn param_str(&self, key: &str) -> Option<&str> {
        self.param(key)?.as_str()
    }
}

#[derive(Debug, Clone)]
struct Match {
    triggers: Vec<String>,
    regex: Option<String>,
    label: Option<String>,
    replace: Option<String>,
    markdown: Option<String>,
    html: Option<String>,
    image_path: Option<String>,
    form: Option<String>,
    form_fields: HashMap<String, FormFieldConfig>,
    vars: Vec<Variable>,
    word: bool,
    left_word: bool,
    right_word: bool,
    propagate_case: bool,
    force_mode: Option<String>,
    index: usize,
    line: Option<usize>,
}

struct MatchFile {
    relative_path: String,
    directory_components: Vec<String>,
    group_name: String,
    imports: Vec<String>,
    global_vars: Vec<Variable>,
    matches: Vec<Match>,
}

/// Where a warning comes from, shared by every warning about one match.
#[derive(Clone)]
struct Origin<'a> {
    file: &'a str,
    match_index: usize,
    line: Option<usize>,
    trigger: Option<&'a str>,
}

impl Origin<'_> {
    fn warn(&self, line: Option<usize>, message: String) -> ImportWarning {
        ImportWarning {
            file: self.file.to_string(),
            line: line.or(self.line),
            match_index: Some(self.match_index),
            trigger: self.trigger.map(str::to_string),
            message,
        }
    }
}

// MARK: - Discovery

/// `dir/match` when it exists, else `dir` itself.
pub(super) fn resolve_match_root(dir: &Path) -> PathBuf {
    let sub = dir.join("match");
    if sub.is_dir() {
        normalize(&sub)
    } else {
        normalize(dir)
    }
}

/// YAML files Espanso auto-loads: recursive, Finder-sorted, skipping hidden and `_`-prefixed entries.
pub(super) fn discover_match_files(root: &Path) -> Result<Vec<PathBuf>, InterchangeError> {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), InterchangeError> {
        let read = fs::read_dir(dir)
            .map_err(|e| InterchangeError::io(format!("Couldn't list {}", dir.display()), e))?;
        let mut entries: Vec<(String, PathBuf, bool)> = read
            .filter_map(Result::ok)
            .filter_map(|e| {
                let name = e.file_name().to_str()?.to_string();
                let is_dir = e.file_type().ok()?.is_dir();
                Some((name, e.path(), is_dir))
            })
            .collect();
        entries.sort_by(|a, b| natural_cmp(&a.0, &b.0));
        for (name, path, is_dir) in entries {
            // Hidden files are skipped; `_` files load only when explicitly imported.
            if name.starts_with('.') || name.starts_with('_') {
                continue;
            }
            if is_dir {
                walk(&path, out)?;
            } else if is_yaml(&path) {
                out.push(normalize(&path));
            }
        }
        Ok(())
    }
    let mut out = Vec::new();
    if root.exists() {
        walk(root, &mut out)?;
    }
    Ok(out)
}

fn is_yaml(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|e| e.to_str())
            .map(str::to_lowercase)
            .as_deref(),
        Some("yml" | "yaml")
    )
}

/// Case-insensitive comparison with digit runs compared numerically, like Finder's sort.
fn natural_cmp(a: &str, b: &str) -> Ordering {
    let (mut ai, mut bi) = (a.chars().peekable(), b.chars().peekable());
    loop {
        match (ai.peek().copied(), bi.peek().copied()) {
            (None, None) => return a.cmp(b),
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(x), Some(y)) if x.is_ascii_digit() && y.is_ascii_digit() => {
                let take = |it: &mut std::iter::Peekable<std::str::Chars>| {
                    let mut digits = String::new();
                    while let Some(c) = it.peek().copied().filter(char::is_ascii_digit) {
                        digits.push(c);
                        it.next();
                    }
                    digits
                };
                let (da, db) = (take(&mut ai), take(&mut bi));
                let (ta, tb) = (da.trim_start_matches('0'), db.trim_start_matches('0'));
                let ord = ta.len().cmp(&tb.len()).then_with(|| ta.cmp(tb));
                if ord != Ordering::Equal {
                    return ord;
                }
            }
            (Some(x), Some(y)) => {
                let ord = x.to_lowercase().cmp(y.to_lowercase());
                if ord != Ordering::Equal {
                    return ord;
                }
                ai.next();
                bi.next();
            }
        }
    }
}

/// Lexically removes `.` and `..` components (like `URL.standardizedFileURL`).
pub(super) fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                if !out.pop() {
                    out.push("..");
                }
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// The deepest directory containing every file.
pub(super) fn common_parent(files: &[PathBuf]) -> Option<PathBuf> {
    let mut iter = files.iter().map(|f| {
        normalize(f)
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_default()
    });
    let first = iter.next()?;
    let mut common: Vec<Component> = first.components().collect();
    let rest: Vec<PathBuf> = iter.collect();
    for other in &rest {
        let n = common
            .iter()
            .zip(other.components())
            .take_while(|(a, b)| **a == *b)
            .count();
        common.truncate(n);
    }
    if common.is_empty() {
        return None;
    }
    Some(common.iter().collect())
}

fn relative_path(file: &Path, base: Option<&Path>) -> (String, Vec<String>) {
    if let Some(rel) = base.and_then(|b| file.strip_prefix(b).ok()) {
        let parts: Vec<String> = rel
            .components()
            .map(|c| c.as_os_str().to_string_lossy().into_owned())
            .collect();
        if !parts.is_empty() {
            let dirs = parts[..parts.len() - 1].to_vec();
            return (parts.join("/"), dirs);
        }
    }
    (
        file.file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default(),
        vec![],
    )
}

// MARK: - Pipeline

/// Loads `initial_files` (following `imports:`) and converts them into a library.
pub(super) fn perform_import(
    initial_files: &[PathBuf],
    base: Option<&Path>,
) -> EspansoImportResult {
    let mut warnings = Vec::new();
    let mut files = Vec::new();
    let mut visited = HashSet::new();
    for file in initial_files {
        load_file(file, base, &mut visited, &mut files, &mut warnings);
    }

    // Globals from every loaded file; later files win on duplicate names.
    let mut library_globals: HashMap<String, Variable> = HashMap::new();
    for file in &files {
        for v in &file.global_vars {
            library_globals.insert(v.name.clone(), v.clone());
        }
    }

    // Pass 1: groups, snippet IDs, and the trigger map for nested `type: match`.
    let mut groups: Vec<GroupRecord> = Vec::new();
    let mut group_ids: HashMap<String, Uuid> = HashMap::new();
    let mut next_sort: HashMap<Option<Uuid>, i64> = HashMap::new();
    let mut ensure_group = |path: &[String], groups: &mut Vec<GroupRecord>| -> Option<Uuid> {
        let mut parent = None;
        for i in 0..path.len() {
            let key = path[..=i].join("/");
            let id = *group_ids.entry(key).or_insert_with(|| {
                let sort = next_sort.entry(parent).or_insert(0);
                let group = GroupRecord {
                    parent_id: parent,
                    sort_index: *sort,
                    ..GroupRecord::new(path[i].clone())
                };
                *sort += 1;
                let id = group.id;
                groups.push(group);
                id
            });
            parent = Some(id);
        }
        parent
    };

    struct Pending<'a> {
        id: Uuid,
        group_id: Uuid,
        file: &'a MatchFile,
        m: &'a Match,
        content: &'a str,
    }
    let mut pending: Vec<Pending> = Vec::new();
    let mut trigger_ids: HashMap<String, Uuid> = HashMap::new();

    for file in &files {
        let mut path = file.directory_components.clone();
        ensure_group(&path, &mut groups);
        path.push(file.group_name.clone());
        let group_id = ensure_group(&path, &mut groups).expect("path is non-empty");

        for m in &file.matches {
            let origin = Origin {
                file: &file.relative_path,
                match_index: m.index,
                line: m.line,
                trigger: m
                    .triggers
                    .first()
                    .map(String::as_str)
                    .or(m.regex.as_deref()),
            };
            if let Some(regex) = &m.regex {
                warnings.push(
                    Origin {
                        trigger: Some(regex),
                        ..origin
                    }
                    .warn(
                        None,
                        format!("Regex trigger '{regex}' is not supported in v1 and was skipped."),
                    ),
                );
                continue;
            }
            if m.triggers.is_empty() {
                warnings.push(
                    Origin {
                        trigger: None,
                        ..origin
                    }
                    .warn(
                        None,
                        "Match is missing a valid 'trigger' or 'triggers' list and was skipped."
                            .into(),
                    ),
                );
                continue;
            }
            if m.image_path.is_some() {
                warnings.push(origin.warn(
                    None,
                    "Image match ('image_path') is not supported in v1 and was skipped.".into(),
                ));
                continue;
            }
            let content = if let Some(r) = &m.replace {
                r
            } else if let Some(f) = &m.form {
                f
            } else if let Some(md) = &m.markdown {
                warnings.push(origin.warn(
                    None,
                    "Markdown match imported as plain text; importing rich text formatting is not supported.".into(),
                ));
                md
            } else if let Some(html) = &m.html {
                warnings.push(origin.warn(
                    None,
                    "HTML match imported as plain text; importing rich text formatting is not supported.".into(),
                ));
                html
            } else {
                warnings.push(origin.warn(
                    None,
                    "Match has no replacement content ('replace', 'form', 'markdown', or 'html') and was skipped."
                        .into(),
                ));
                continue;
            };
            if (m.left_word || m.right_word) && !m.word {
                warnings.push(origin.warn(
                    None,
                    "Partial word boundary ('left_word'/'right_word') mapped to whole-word mode.".into(),
                ));
            }
            let id = Uuid::new_v4();
            for t in &m.triggers {
                trigger_ids.entry(t.clone()).or_insert(id);
            }
            pending.push(Pending {
                id,
                group_id,
                file,
                m,
                content,
            });
        }
    }

    // Pass 2: convert templates, resolving variables and nested matches.
    let timestamp = Utc::now().trunc_subsecs(0);
    let mut snippets = Vec::new();
    for p in pending {
        let (m, file) = (p.m, p.file);
        let mut vars = library_globals.clone();
        for v in &file.global_vars {
            vars.insert(v.name.clone(), v.clone());
        }
        // `type: global` locals just reference a global of the same name.
        for v in m.vars.iter().filter(|v| v.kind != "global") {
            vars.insert(v.name.clone(), v.clone());
        }
        let ctx = Converter {
            vars: &vars,
            trigger_ids: &trigger_ids,
            origin: Origin {
                file: &file.relative_path,
                match_index: m.index,
                line: m.line,
                trigger: m.triggers.first().map(String::as_str),
            },
        };
        let content = if m.form.is_some() && m.replace.is_none() {
            ctx.convert_form(p.content, &m.form_fields, &mut warnings)
        } else {
            ctx.convert_replace(p.content, &mut warnings)
        };
        let name = m
            .label
            .as_deref()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .map_or_else(|| m.triggers[0].clone(), str::to_string);
        let insertion = match m.force_mode.as_deref().map(str::to_lowercase).as_deref() {
            Some("clipboard" | "paste") => Some(InsertionMethod::Paste),
            Some("keys" | "type") => Some(InsertionMethod::Type),
            Some("auto") => Some(InsertionMethod::Automatic),
            _ => None,
        };
        snippets.push(SnippetRecord {
            id: p.id,
            group_id: Some(p.group_id),
            fire_mode: if m.word || m.left_word || m.right_word {
                FireMode::WholeWord
            } else {
                FireMode::Immediate
            },
            case_mode: if m.propagate_case {
                CaseMode::Propagate
            } else {
                CaseMode::Exact
            },
            insertion,
            created_at: timestamp,
            updated_at: timestamp,
            ..SnippetRecord::new(name, m.triggers.clone(), content)
        });
    }

    EspansoImportResult {
        library: KudzuLibrary {
            groups,
            snippets,
            ..KudzuLibrary::default()
        },
        warnings,
    }
}

fn load_file(
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

fn parse_form_fields(node: &Node) -> HashMap<String, FormFieldConfig> {
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

// MARK: - Template conversion

struct Converter<'a> {
    vars: &'a HashMap<String, Variable>,
    trigger_ids: &'a HashMap<String, Uuid>,
    origin: Origin<'a>,
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
    fn convert_form(
        &self,
        raw: &str,
        fields: &HashMap<String, FormFieldConfig>,
        warnings: &mut Vec<ImportWarning>,
    ) -> String {
        self.convert(raw, Some(fields), self.origin.line, warnings)
    }

    /// Converts a `replace:` text: `$|$` and `{{var}}` references.
    fn convert_replace(&self, raw: &str, warnings: &mut Vec<ImportWarning>) -> String {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn natural_sort_is_case_insensitive_and_numeric() {
        let mut names = vec!["b.yml", "A10.yml", "a2.yml", "Work", "base.yml"];
        names.sort_by(|a, b| natural_cmp(a, b));
        assert_eq!(
            names,
            vec!["a2.yml", "A10.yml", "b.yml", "base.yml", "Work"]
        );
    }

    #[test]
    fn normalize_and_common_parent() {
        assert_eq!(
            normalize(Path::new("/a/b/./../c/d.yml")),
            PathBuf::from("/a/c/d.yml")
        );
        let files = [
            PathBuf::from("/x/m/base.yml"),
            PathBuf::from("/x/m/work/a.yml"),
        ];
        assert_eq!(common_parent(&files), Some(PathBuf::from("/x/m")));
    }
}
