//! Shared helpers for the interchange integration tests.
#![allow(dead_code)]

use std::path::{Path, PathBuf};

use kudzu_core::SnippetRecord;

/// A temporary directory removed on drop.
pub struct TempDir(pub PathBuf);

impl TempDir {
    pub fn new(label: &str) -> Self {
        let path = std::env::temp_dir().join(format!("kudzu-{label}-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    pub fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// The copied Swift Espanso fixtures (`tests/fixtures/espanso`).
pub fn espanso_fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/espanso")
}

/// Writes `files` (relative path → contents) under `root`.
pub fn write_files<'a>(root: &Path, files: impl IntoIterator<Item = (&'a String, &'a String)>) {
    for (relative, contents) in files {
        let path = root.join(relative);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, contents).unwrap();
    }
}

/// Replaces `{{snippet id="…"}}` with the target's primary trigger so contents compare across imports.
pub fn normalize_snippet_ids(content: &str, snippets: &[SnippetRecord]) -> String {
    let mut out = content.to_string();
    for s in snippets {
        if let Some(t) = s.triggers.first() {
            let id = s.id.hyphenated().to_string();
            for form in [id.to_uppercase(), id] {
                out = out.replace(
                    &format!("{{{{snippet id=\"{form}\"}}}}"),
                    &format!("{{{{snippet trigger=\"{t}\"}}}}"),
                );
            }
        }
    }
    out
}
