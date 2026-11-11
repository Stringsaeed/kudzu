//! Enforce architecture against Cargo's resolved graph and first-party source paths.
use serde_json::Value;
use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
    process::Command,
};

type Result<T> = std::result::Result<T, String>;

pub fn run() -> Result<()> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .canonicalize()
        .map_err(|e| e.to_string())?;
    let output = Command::new("cargo")
        .args([
            "metadata",
            "--format-version",
            "1",
            "--locked",
            "--filter-platform",
        ])
        .arg(std::env::var("TARGET").unwrap_or_else(|_| {
            let out = Command::new("rustc").arg("-vV").output().expect("rustc");
            String::from_utf8_lossy(&out.stdout)
                .lines()
                .find_map(|s| s.strip_prefix("host: "))
                .unwrap()
                .to_owned()
        }))
        .current_dir(&root)
        .output()
        .map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).into_owned());
    }
    let metadata = serde_json::from_slice(&output.stdout).map_err(|e| e.to_string())?;
    validate(&root, &metadata)
}

fn contained(root: &Path, path: &Path) -> Result<PathBuf> {
    let path = path
        .canonicalize()
        .map_err(|e| format!("{}: {e}", path.display()))?;
    if !path.starts_with(root) {
        return Err(format!(
            "first-party path escapes workspace: {}",
            path.display()
        ));
    }
    Ok(path)
}

/// First-party crates with rules below. Any other first-party crate fails the check,
/// except throwaway spikes under `spikes/`, which may compose every layer.
const KNOWN: &[&str] = &[
    "kudzu",
    "kudzu-core",
    "kudzu-engine",
    "kudzu-app",
    "kudzu-system",
    "kudzu-appkit",
    "kudzu-ui",
    "xtask",
];

const LAYERS: &[&str] = &[
    "kudzu-core",
    "kudzu-app",
    "kudzu-engine",
    "kudzu-system",
    "kudzu-appkit",
    "kudzu-ui",
];

fn allowed(name: &str, spike: bool) -> &'static [&'static str] {
    if spike {
        return LAYERS;
    }
    match name {
        "kudzu-engine" | "kudzu-app" => &["kudzu-core"],
        "kudzu-system" => &["kudzu-app", "kudzu-core", "kudzu-engine"],
        "kudzu-ui" => &["kudzu-app", "kudzu-core", "kudzu-appkit"],
        "kudzu" => LAYERS,
        // kudzu-core, kudzu-appkit, xtask; unknown crates are rejected before this.
        _ => &[],
    }
}

fn forbidden(owner: &str, dep: &str) -> bool {
    let rune = dep.starts_with("rune");
    let os = dep.starts_with("objc2")
        || matches!(
            dep,
            "winit" | "core-foundation" | "core-graphics" | "cocoa" | "windows" | "windows-sys"
        );
    match owner {
        "kudzu-core" => rune || os || matches!(dep, "tokio" | "async-std" | "smol"),
        "kudzu-app" => rune || dep.starts_with("objc2") || matches!(dep, "winit" | "tokio"),
        "kudzu-engine" => rune || dep == "kudzu-app",
        "kudzu-system" => rune,
        "kudzu-appkit" => rune || dep.starts_with("kudzu-"),
        "kudzu-ui" => matches!(dep, "kudzu-engine" | "kudzu-system"),
        _ => false,
    }
}

fn validate(root: &Path, metadata: &Value) -> Result<()> {
    let packages = metadata["packages"].as_array().ok_or("missing packages")?;
    let members: HashSet<_> = metadata["workspace_members"]
        .as_array()
        .ok_or("missing members")?
        .iter()
        .filter_map(Value::as_str)
        .collect();
    let by_id: HashMap<_, _> = packages
        .iter()
        .map(|p| (p["id"].as_str().unwrap(), p))
        .collect();
    let nodes: HashMap<_, _> = metadata["resolve"]["nodes"]
        .as_array()
        .ok_or("missing resolve graph")?
        .iter()
        .map(|n| (n["id"].as_str().unwrap(), n))
        .collect();
    let first_party = |package: &&Value| {
        members.contains(package["id"].as_str().unwrap()) || package["source"].is_null()
    };
    for package in packages.iter().filter(first_party) {
        let owner = package["name"].as_str().unwrap();
        let manifest = contained(root, Path::new(package["manifest_path"].as_str().unwrap()))?;
        let spike = manifest.starts_with(root.join("spikes"));
        if !spike && !KNOWN.contains(&owner) {
            return Err(format!(
                "unknown first-party crate {owner}: add its rules to check-deps"
            ));
        }
    }
    for package in packages.iter().filter(first_party) {
        let id = package["id"].as_str().unwrap();
        let manifest = contained(root, Path::new(package["manifest_path"].as_str().unwrap()))?;
        let dir = manifest.parent().unwrap();
        for dep in package["dependencies"]
            .as_array()
            .ok_or("missing dependencies")?
        {
            if let Some(path) = dep["path"].as_str() {
                contained(root, Path::new(path))?;
            }
        }
        if dir.join("build.rs").exists()
            || package["targets"].as_array().unwrap().iter().any(|t| {
                t["kind"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|k| k == "custom-build")
            })
        {
            return Err(format!("first-party build script: {}", dir.display()));
        }
        scan(root, dir)?;
        let owner = package["name"].as_str().unwrap();
        let spike = dir.starts_with(root.join("spikes"));
        if let Some(node) = nodes.get(id) {
            for dep in node["dependencies"].as_array().unwrap() {
                let name = by_id[dep.as_str().unwrap()]["name"].as_str().unwrap();
                if name.starts_with("kudzu-") && !allowed(owner, spike).contains(&name) {
                    return Err(format!("forbidden edge: {owner} -> {name}"));
                }
            }
        }
        let mut visited = HashSet::new();
        let mut pending = vec![id];
        while let Some(id) = pending.pop() {
            if !visited.insert(id) {
                continue;
            }
            if let Some(node) = nodes.get(id) {
                for dep in node["dependencies"].as_array().unwrap() {
                    let dep = dep.as_str().unwrap();
                    let name = by_id[dep]["name"].as_str().unwrap();
                    if !spike && forbidden(owner, name) {
                        return Err(format!("forbidden dependency tree: {owner} -> {name}"));
                    }
                    pending.push(dep);
                }
            }
        }
    }
    if root.join("scripts").exists() {
        scan(root, &root.join("scripts"))?;
    }
    Ok(())
}

fn scan(root: &Path, path: &Path) -> Result<()> {
    let path = contained(root, path)?;
    if path.is_dir() {
        for entry in fs::read_dir(&path).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            if matches!(
                entry.file_name().to_str(),
                Some("target" | ".git" | "__pycache__")
            ) {
                continue;
            }
            scan(root, &entry.path())?;
        }
        return Ok(());
    }
    let scripts = path.starts_with(root.join("scripts"));
    if !scripts
        && path.extension().and_then(|s| s.to_str()) != Some("rs")
        && path.file_name().and_then(|s| s.to_str()) != Some("Cargo.toml")
    {
        return Ok(());
    }
    let source = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    if mentions_frozen_app(&source) {
        return Err(format!("reference to frozen app: {}", path.display()));
    }
    if path.file_name().and_then(|s| s.to_str()) == Some("Cargo.toml") {
        let items = tokenize(&source, true)?;
        for group in items.windows(3) {
            if group[0].trim_matches('"') == "path" && group[1] == "=" {
                let value = group[2]
                    .strip_prefix('"')
                    .and_then(|s| s.strip_suffix('"'))
                    .ok_or("non-literal manifest path")?;
                contained(root, &path.parent().unwrap().join(value))?;
            }
        }
    }
    if path.extension().and_then(|s| s.to_str()) == Some("rs") {
        source_paths(root, &path, &source)?;
    }
    Ok(())
}

/// Defence in depth beside the structural checks: a literal mention of the frozen Tauri app.
/// `apps/desktop-v2` and `../desktop-v2` are this workspace, so a name character after the
/// match (`-`, `_`, alphanumeric) is not a mention.
fn mentions_frozen_app(source: &str) -> bool {
    // Split the spellings so the checker does not flag its own implementation.
    [concat!("apps/", "desktop"), concat!("../", "desktop")]
        .iter()
        .any(|needle| {
            source.match_indices(needle).any(|(at, _)| {
                !source[at + needle.len()..]
                    .chars()
                    .next()
                    .is_some_and(|c| c == '-' || c == '_' || c.is_alphanumeric())
            })
        })
}

/// Resolve literal paths; reject computed include paths rather than silently trusting them.
/// Rust comments are skipped; strings are consumed atomically (including raw strings).
fn source_paths(root: &Path, file: &Path, source: &str) -> Result<()> {
    let tokens = tokenize(source, false)?;
    for (i, token) in tokens.iter().enumerate() {
        let is_include = matches!(token.as_str(), "include_str" | "include_bytes" | "include")
            && tokens.get(i + 1).map(String::as_str) == Some("!");
        let is_path = token == "path"
            && tokens.get(i.wrapping_sub(1)).map(String::as_str) == Some("[")
            && tokens.get(i + 1).map(String::as_str) == Some("=");
        if !is_include && !is_path {
            continue;
        }
        let literal = tokens
            .get(i + if is_include { 3 } else { 2 })
            .ok_or("missing path literal")?;
        let value = literal
            .strip_prefix('"')
            .and_then(|s| s.strip_suffix('"'))
            .ok_or_else(|| format!("non-literal include/path in {}", file.display()))?;
        if value.contains('\\') {
            return Err(format!("escaped source path in {}", file.display()));
        }
        let target = contained(root, &file.parent().unwrap().join(value))?;
        // `#[path]` and `include!` pull in Rust that `scan` must also read, so they must name
        // a `.rs` file. `include_str!` and `include_bytes!` read data, not code.
        let code = is_path || token == "include";
        if code && target.extension().and_then(|s| s.to_str()) != Some("rs") {
            let what = if is_path { "#[path]" } else { "include!" };
            return Err(format!("{what} must name a .rs file in {}", file.display()));
        }
    }
    Ok(())
}

fn tokenize(source: &str, toml: bool) -> Result<Vec<String>> {
    let b = source.as_bytes();
    let mut i = 0;
    let mut out = Vec::new();
    while i < b.len() {
        // Character literals must not be mistaken for opening string quotes.
        if !toml && b[i] == b'\'' {
            let end = if b.get(i + 1) == Some(&b'\\') {
                i + 3
            } else {
                i + 2
            };
            if b.get(end) == Some(&b'\'') {
                i = end + 1;
                continue;
            }
        }
        if toml && b[i] == b'#' {
            while i < b.len() && b[i] != b'\n' {
                i += 1;
            }
            continue;
        }
        if toml && b[i] == b'\'' {
            i += 1;
            let start = i;
            while i < b.len() && b[i] != b'\'' {
                i += 1;
            }
            if i == b.len() {
                return Err("unterminated TOML literal".into());
            }
            out.push(format!("\"{}\"", &source[start..i]));
            i += 1;
            continue;
        }
        if b[i].is_ascii_whitespace() {
            i += 1;
            continue;
        }
        if b[i..].starts_with(b"//") {
            while i < b.len() && b[i] != b'\n' {
                i += 1;
            }
        } else if b[i..].starts_with(b"/*") {
            i += 2;
            let mut depth = 1;
            while i < b.len() && depth > 0 {
                if b[i..].starts_with(b"/*") {
                    depth += 1;
                    i += 2;
                } else if b[i..].starts_with(b"*/") {
                    depth -= 1;
                    i += 2;
                } else {
                    i += 1;
                }
            }
        } else if b[i] == b'"'
            || (b[i] == b'r' && b.get(i + 1).is_some_and(|v| *v == b'#' || *v == b'"'))
        {
            let start = i;
            let raw = b[i] == b'r';
            if raw {
                i += 1;
            }
            let hashes_start = i;
            if raw {
                while b.get(i) == Some(&b'#') {
                    i += 1;
                }
            }
            let hashes = i - hashes_start;
            if b.get(i) != Some(&b'"') {
                return Err("invalid raw string".into());
            }
            i += 1;
            let content = i;
            loop {
                if i >= b.len() {
                    return Err("unterminated string".into());
                }
                if b[i] == b'"'
                    && (!raw
                        || b.get(i + 1..i + 1 + hashes)
                            .is_some_and(|s| s.iter().all(|v| *v == b'#')))
                {
                    break;
                }
                if !raw && b[i] == b'\\' {
                    i += 1;
                }
                i += 1;
            }
            if raw {
                out.push(format!("\"{}\"", &source[content..i]));
            } else {
                out.push(source[start..=i].to_owned());
            }
            i += 1 + hashes;
        } else if b[i].is_ascii_alphabetic() || b[i] == b'_' {
            let start = i;
            while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_') {
                i += 1;
            }
            out.push(source[start..i].to_owned());
        } else {
            // Only ASCII punctuation is relevant for path syntax.
            out.push((b[i] as char).to_string());
            i += 1;
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests;
