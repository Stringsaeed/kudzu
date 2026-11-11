//! Espanso match files → Kudzu library.

use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Component, Path, PathBuf};

use chrono::{SubsecRound, Utc};
use uuid::Uuid;

mod convert;
mod parse;

use super::{EspansoImportResult, ImportWarning};
use crate::interchange::InterchangeError;
use crate::library::model::{
    CaseMode, FireMode, GroupRecord, InsertionMethod, KudzuLibrary, SnippetRecord,
};

use super::yaml::Node;
use crate::interchange::rich;

use convert::Converter;
use parse::load_file;

#[derive(Debug, Clone, Default)]
pub(super) struct FormFieldConfig {
    pub(super) kind: String,
    pub(super) multiline: bool,
    pub(super) default: Option<String>,
    pub(super) values: Vec<String>,
}

#[derive(Debug, Clone)]
pub(super) struct Variable {
    pub(super) name: String,
    pub(super) kind: String,
    pub(super) params: Option<Node>,
    pub(super) line: Option<usize>,
}

impl Variable {
    pub(super) fn param(&self, key: &str) -> Option<&Node> {
        self.params.as_ref()?.get(key)
    }

    fn param_str(&self, key: &str) -> Option<&str> {
        self.param(key)?.as_str()
    }
}

#[derive(Debug, Clone)]
pub(super) struct Match {
    pub(super) triggers: Vec<String>,
    pub(super) regex: Option<String>,
    pub(super) label: Option<String>,
    pub(super) replace: Option<String>,
    pub(super) markdown: Option<String>,
    pub(super) html: Option<String>,
    pub(super) image_path: Option<String>,
    pub(super) form: Option<String>,
    pub(super) form_fields: HashMap<String, FormFieldConfig>,
    pub(super) vars: Vec<Variable>,
    pub(super) word: bool,
    pub(super) left_word: bool,
    pub(super) right_word: bool,
    pub(super) propagate_case: bool,
    pub(super) force_mode: Option<String>,
    pub(super) index: usize,
    pub(super) line: Option<usize>,
}

pub(super) struct MatchFile {
    pub(super) relative_path: String,
    pub(super) directory_components: Vec<String>,
    pub(super) group_name: String,
    pub(super) imports: Vec<String>,
    pub(super) global_vars: Vec<Variable>,
    pub(super) matches: Vec<Match>,
}

/// Where a warning comes from, shared by every warning about one match.
#[derive(Clone)]
pub(super) struct Origin<'a> {
    pub(super) file: &'a str,
    pub(super) match_index: usize,
    pub(super) line: Option<usize>,
    pub(super) trigger: Option<&'a str>,
}

impl Origin<'_> {
    pub(super) fn warn(&self, line: Option<usize>, message: String) -> ImportWarning {
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

pub(super) fn relative_path(file: &Path, base: Option<&Path>) -> (String, Vec<String>) {
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
        /// Formatting read from a `markdown:` or `html:` match.
        rich: Option<rich::RichText>,
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
            let mut rich = None;
            let content = if let Some(r) = &m.replace {
                r
            } else if let Some(f) = &m.form {
                f
            } else if let Some((source, kind)) = m
                .markdown
                .as_ref()
                .map(|md| (md, "Markdown"))
                .or_else(|| m.html.as_ref().map(|html| (html, "HTML")))
            {
                let read = if kind == "HTML" {
                    rich::read_html(source)
                } else {
                    rich::read_markdown(source)
                };
                match read {
                    Ok(text) => rich = Some(text),
                    Err(reason) => warnings.push(origin.warn(
                        None,
                        format!("{kind} match imported as plain text: {reason}."),
                    )),
                }
                source
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
                rich,
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
        let mut formatting = None;
        let content = if let Some(text) = &p.rich {
            let atoms = rich::espanso_atoms(&text.text);
            let converted = rich::to_template(text, &atoms, |segment| {
                ctx.convert_replace(segment, &mut warnings)
            });
            if let Some(message) = text.warning() {
                warnings.push(ctx.origin.warn(None, message));
            }
            formatting = Some(converted.formatting);
            converted.content
        } else if m.form.is_some() && m.replace.is_none() {
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
            formatting,
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

#[cfg(test)]
mod tests;
